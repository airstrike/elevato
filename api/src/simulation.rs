//! The host's side of the API: a program running against a challenge's
//! world, one frame at a time.
//!
//! A [`Simulation`] holds the program's state and the [`Building`], and
//! calls the program's `update` and `subscription` the way an iced
//! runtime does, but it runs the
//! program's tasks itself, on its own thread, at fixed points of each
//! physics substep. That keeps a run deterministic - the same seed and
//! program replay the same run - and it delivers every message at the
//! moment the original game called its handlers:
//!
//! 1. physics steps the world;
//! 2. riders' presses, passing floors and idle cars are delivered, and
//!    the tasks they lead to run until none can make progress;
//! 3. riders get off and on at the cars that arrived;
//! 4. each arrival ends its trip's task with
//!    [`StoppedAtFloor`](elevator::Message::StoppedAtFloor), and the
//!    program runs to rest again.
//!
//! Tasks that wait on anything but elevato's own commands never make
//! progress: there is no clock and no I/O, only the building.

use std::panic::{self, AssertUnwindSafe};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll, Wake, Waker};

use elevato_core::World;
use elevato_core::challenge::{Challenge, Outcome};
use elevato_core::event::Event;
use elevato_core::stats::Stats;
use elevato_core::world::DT_MAX;
use iced_futures::futures::channel::mpsc;
use iced_futures::subscription::{self, Tracker};
use iced_futures::{BoxFuture, BoxStream, MaybeSend, Subscription};
use iced_runtime::{Action, Task, task};

use crate::building::Building;
use crate::elevator::{self, Elevator};
use crate::floor;
use crate::hub::{self, Command, Hub};
use crate::message::Message;

/// Hard cap on delivery rounds per dispatch point: a program whose
/// commands keep raising events past this many rounds has the rest
/// delivered at the next dispatch point.
const DISPATCH_ROUNDS: usize = 128;

/// A program running against a challenge's world.
///
/// Minted from a challenge, a seed and the program's `new`, `update` and
/// `subscription` (`transformation-method`): construction builds the
/// building, tells the program how many cars its bank holds, and runs the
/// tasks `new` returns, so a simulation that exists is under way.
pub struct Simulation<P, M> {
    hub: Hub,
    run: Run<P, M>,
}

/// Why a program stopped running.
#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    /// The program panicked; the run it was in is over.
    #[error("your program panicked: {0}")]
    Panicked(String),
}

/// The program's `update`.
type Update<P, M> = dyn Fn(&mut P, M, &Building) -> Task<M>;

/// The program's `subscription`.
type Subscribe<P, M> = dyn Fn(&P) -> Subscription<M>;

/// Everything a simulation runs, apart from its hub.
struct Run<P, M> {
    building: Building,
    program: P,
    update: Box<Update<P, M>>,
    subscription: Box<Subscribe<P, M>>,
    tasks: Vec<Live<BoxStream<Action<M>>>>,
    tracker: Tracker,
    listeners: Vec<Live<BoxFuture<()>>>,
    sender: mpsc::UnboundedSender<M>,
    receiver: mpsc::UnboundedReceiver<M>,
    /// Each powered car's news and current trip, by its bank index.
    cars: Vec<Car>,
    /// Cars that stopped this substep, waiting for riders to get off and
    /// on before their trips end.
    arrivals: Vec<Arrival>,
    failure: Option<Error>,
}

/// A powered car: where its news goes, and the trip it is on.
#[derive(Default)]
struct Car {
    news: Option<mpsc::UnboundedSender<elevator::Message>>,
    trip: Option<Trip>,
}

/// A trip's task: the floor it goes to, where its messages go, and how
/// to abort it.
struct Trip {
    floor: usize,
    sender: mpsc::UnboundedSender<elevator::Message>,
    handle: task::Handle,
}

/// A car that stopped, before riders got off and on.
struct Arrival {
    elevator: Elevator,
    floor: usize,
    /// Riders aboard as it stopped.
    aboard: usize,
    /// Riders aboard who were going here.
    leaving: usize,
    /// The trip it ended, when it reached the program's floor.
    trip: Option<Trip>,
}

/// A stream or future the simulation polls, and whether it has been
/// woken since.
struct Live<T> {
    work: T,
    woken: Arc<Woken>,
    waker: Waker,
}

struct Woken(AtomicBool);

impl Wake for Woken {
    fn wake(self: Arc<Self>) {
        self.0.store(true, Ordering::Relaxed);
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.store(true, Ordering::Relaxed);
    }
}

impl<T> Live<T> {
    fn new(work: T) -> Self {
        let woken = Arc::new(Woken(AtomicBool::new(true)));
        Self {
            work,
            waker: Waker::from(woken.clone()),
            woken,
        }
    }

    /// Whether it was woken, clearing the mark.
    fn take_woken(&self) -> bool {
        self.woken.0.swap(false, Ordering::Relaxed)
    }

    fn mark_woken(&self) {
        self.woken.0.store(true, Ordering::Relaxed);
    }
}

impl<P, M> Simulation<P, M>
where
    M: MaybeSend + 'static,
{
    /// Builds the building for `challenge` and boots the program on it:
    /// `new` learns how many cars the bank holds and powers on the ones it
    /// wants, and its tasks run (each powered car reporting
    /// [`Idle`](elevator::Message::Idle)), all before this returns.
    pub fn new(
        challenge: &Challenge,
        seed: u64,
        new: impl FnOnce(usize) -> (P, Task<M>),
        update: impl Fn(&mut P, M, &Building) -> Task<M> + 'static,
        subscription: impl Fn(&P) -> Subscription<M> + 'static,
    ) -> Result<Self, Error> {
        let world = World::new(challenge, seed);
        let bank = world.bank_size();
        let mut hub = Hub::default();
        let (program, task) = hub::enter(&mut hub, || guard(|| new(bank)))?;
        let (sender, receiver) = mpsc::unbounded();
        let mut simulation = Self {
            hub,
            run: Run {
                building: Building::new(world),
                program,
                update: Box::new(update),
                subscription: Box::new(subscription),
                tasks: Vec::new(),
                tracker: Tracker::new(),
                listeners: Vec::new(),
                sender,
                receiver,
                cars: Vec::new(),
                arrivals: Vec::new(),
                failure: None,
            },
        };
        let run = &mut simulation.run;
        hub::enter(&mut simulation.hub, || {
            run.guarded(|run| {
                run.apply();
                run.spawn(task);
                run.subscribe();
                run.dispatch();
            })
        })?;
        Ok(simulation)
    }

    /// Advances one frame of `substeps` × [`DT_MAX`] simulated seconds,
    /// stopping the moment the challenge decides. A panic in the program
    /// ends the run: this frame and every later one return the error.
    pub fn frame(&mut self, substeps: usize) -> Result<(), Error> {
        let run = &mut self.run;
        hub::enter(&mut self.hub, || {
            run.guarded(|run| {
                for _ in 0..substeps {
                    if run.building.world().ended() {
                        break;
                    }
                    run.building.world_mut().step(DT_MAX);
                    run.dispatch();
                    run.building.world_mut().process_arrivals();
                    run.arrive();
                    run.dispatch();
                }
            })
        })
    }

    /// Hands the program a message from outside its tasks (its view's,
    /// say); it arrives at the next dispatch point.
    pub fn send(&self, message: M) {
        let _ = self.run.sender.unbounded_send(message);
    }

    /// The live statistics.
    pub fn stats(&self) -> Stats {
        *self.run.building.world().stats()
    }

    /// How the challenge stands.
    pub fn outcome(&self) -> Outcome {
        self.run.building.world().outcome()
    }

    /// Whether the challenge has decided.
    pub fn ended(&self) -> bool {
        self.run.building.world().ended()
    }

    /// The building as it stands.
    pub fn building(&self) -> &Building {
        &self.run.building
    }

    /// The program's state.
    pub fn program(&self) -> &P {
        &self.run.program
    }
}

impl<P, M> Run<P, M>
where
    M: MaybeSend + 'static,
{
    /// Runs `f` unless an earlier panic ended the run, and ends the run
    /// if `f` panics.
    fn guarded(&mut self, f: impl FnOnce(&mut Self)) -> Result<(), Error> {
        if let Some(failure) = &self.failure {
            return Err(failure.clone());
        }
        guard(|| f(self)).inspect_err(|error| self.failure = Some(error.clone()))
    }

    /// Delivers the world's events and runs the program until it rests,
    /// round after round while its commands raise more.
    fn dispatch(&mut self) {
        self.settle();
        for _ in 0..DISPATCH_ROUNDS {
            let events = self.building.world_mut().drain_events();
            if events.is_empty() {
                return;
            }
            for event in events {
                self.route(event);
            }
            self.settle();
        }
    }

    /// Sends one world event where it belongs.
    fn route(&mut self, event: Event) {
        match event {
            Event::Idle { elevator } => {
                if let Some(news) = &self.cars[elevator].news {
                    let _ = news.unbounded_send(elevator::Message::Idle);
                }
            }
            Event::UpButtonPressed { floor } => {
                hub::broadcast(Message::Floor(floor, floor::Message::UpButtonPressed));
            }
            Event::DownButtonPressed { floor } => {
                hub::broadcast(Message::Floor(floor, floor::Message::DownButtonPressed));
            }
            Event::FloorButtonPressed { elevator, floor } => {
                hub::broadcast(Message::Elevator(
                    Elevator::at(elevator),
                    elevator::Message::FloorButtonPressed(floor),
                ));
            }
            Event::PassingFloor {
                elevator,
                floor,
                direction,
            } => {
                let heading = self.building.world().elevators()[elevator]
                    .destination()
                    .map(|level| level.round() as usize);
                if let Some(trip) = &self.cars[elevator].trip
                    && Some(trip.floor) == heading
                {
                    let _ = trip
                        .sender
                        .unbounded_send(elevator::Message::PassingFloor(floor, direction));
                }
            }
            Event::StoppedAtFloor { elevator, floor } => {
                let car = &mut self.cars[elevator];
                let trip = car.trip.take_if(|trip| trip.floor == floor);
                let aboard = self.riders(elevator, |_| true);
                let leaving = self.riders(elevator, |destination| destination == floor);
                self.arrivals.push(Arrival {
                    elevator: Elevator::at(elevator),
                    floor,
                    aboard,
                    leaving,
                    trip,
                });
            }
        }
    }

    /// Ends the trips of the cars that arrived, now that riders have got
    /// off and on.
    fn arrive(&mut self) {
        for arrival in std::mem::take(&mut self.arrivals) {
            let index = arrival.elevator.index();
            let car = &self.building.world().elevators()[index];
            let message = elevator::Message::StoppedAtFloor {
                floor: arrival.floor,
                exited: arrival.leaving,
                boarded: self.riders(index, |_| true) + arrival.leaving - arrival.aboard,
                load_factor: car.load_factor(),
                full: car.is_full(),
            };
            if let Some(trip) = arrival.trip {
                let _ = trip.sender.unbounded_send(message);
            }
        }
    }

    /// How many riders aboard `elevator` are going where `going` accepts.
    fn riders(&self, elevator: usize, going: impl Fn(usize) -> bool) -> usize {
        self.building
            .world()
            .passengers()
            .iter()
            .filter(|passenger| {
                passenger.aboard() == Some(elevator) && going(passenger.destination_floor())
            })
            .count()
    }

    /// Polls every woken task and subscription, delivering what they
    /// produce, until nothing is left to wake.
    fn settle(&mut self) {
        loop {
            let mut progressed = false;

            let mut index = 0;
            while index < self.tasks.len() {
                let live = &mut self.tasks[index];
                if !live.take_woken() {
                    index += 1;
                    continue;
                }
                progressed = true;
                let poll = live
                    .work
                    .as_mut()
                    .poll_next(&mut Context::from_waker(&live.waker));
                match poll {
                    Poll::Ready(Some(action)) => {
                        live.mark_woken();
                        self.apply();
                        if let Action::Output(message) = action {
                            self.deliver(message);
                        }
                        index += 1;
                    }
                    Poll::Ready(None) => {
                        let _ = self.tasks.remove(index);
                        self.apply();
                    }
                    Poll::Pending => {
                        self.apply();
                        index += 1;
                    }
                }
            }

            let mut index = 0;
            while index < self.listeners.len() {
                let live = &mut self.listeners[index];
                if !live.take_woken() {
                    index += 1;
                    continue;
                }
                progressed = true;
                let poll = live
                    .work
                    .as_mut()
                    .poll(&mut Context::from_waker(&live.waker));
                if poll.is_ready() {
                    let _ = self.listeners.remove(index);
                } else {
                    index += 1;
                }
            }

            while let Ok(message) = self.receiver.try_recv() {
                progressed = true;
                self.deliver(message);
            }

            if !progressed {
                return;
            }
        }
    }

    /// Folds one message into the program and starts what it answers.
    fn deliver(&mut self, message: M) {
        let task = (self.update)(&mut self.program, message, &self.building);
        self.apply();
        self.spawn(task);
        self.subscribe();
    }

    fn spawn(&mut self, task: Task<M>) {
        if let Some(stream) = task::into_stream(task) {
            self.tasks.push(Live::new(stream));
        }
    }

    /// Starts the subscriptions the program now asks for and stops the
    /// ones it no longer does.
    fn subscribe(&mut self) {
        let recipes = subscription::into_recipes((self.subscription)(&self.program));
        let started = self
            .tracker
            .update(recipes.into_iter(), self.sender.clone());
        self.listeners.extend(started.into_iter().map(Live::new));
    }

    /// Applies the commands the program submitted, in order. A car past
    /// the bank never enters the building, so its commands do nothing.
    fn apply(&mut self) {
        for command in hub::commands() {
            if command.elevator().index() >= self.cars.len()
                && !matches!(command, Command::PowerOn { .. })
            {
                continue;
            }
            match command {
                Command::PowerOn { news, .. } => {
                    if self.building.world_mut().add_elevator().is_some() {
                        self.cars.push(Car { news, trip: None });
                    }
                }
                Command::GoToFloor {
                    elevator,
                    floor,
                    sender,
                    handle,
                } => {
                    let index = elevator.index();
                    let world = self.building.world_mut();
                    // A floor past the top is the top floor, as in the original.
                    let floor = floor.min(world.floors().len().saturating_sub(1));
                    world.go_to_floor(index, floor as f64);
                    let trip = Trip {
                        floor,
                        sender,
                        handle,
                    };
                    if let Some(replaced) = self.cars[index].trip.replace(trip) {
                        replaced.handle.abort();
                    }
                }
                Command::Stop { elevator } => {
                    self.building.world_mut().stop(elevator.index());
                    if let Some(trip) = self.cars[elevator.index()].trip.take() {
                        trip.handle.abort();
                    }
                }
                Command::GoingUpIndicator { elevator, on } => {
                    self.building
                        .world_mut()
                        .elevator_mut(elevator.index())
                        .set_going_up_indicator(on);
                }
                Command::GoingDownIndicator { elevator, on } => {
                    self.building
                        .world_mut()
                        .elevator_mut(elevator.index())
                        .set_going_down_indicator(on);
                }
            }
        }
    }
}

/// Runs `f`, turning a panic into an [`Error`].
fn guard<T>(f: impl FnOnce() -> T) -> Result<T, Error> {
    panic::catch_unwind(AssertUnwindSafe(f)).map_err(|payload| {
        let message = payload
            .downcast_ref::<&str>()
            .map(|message| (*message).to_owned())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "a panic with no message".to_owned());
        Error::Panicked(message)
    })
}
