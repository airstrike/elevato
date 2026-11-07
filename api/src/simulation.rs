//! The host's side of the API: a program running against a challenge's
//! world, one frame at a time.
//!
//! A [`Simulation`] holds the program's state and calls its `update` and
//! `subscription` the way an iced runtime does, but it runs the
//! program's tasks itself, on its own thread, at fixed points of each
//! physics substep. That keeps a run deterministic - the same seed and
//! program replay the same run - and it delivers every message at the
//! moment the original game called its handlers:
//!
//! 1. physics steps the world;
//! 2. riders' presses and passing floors are delivered, and the tasks
//!    they lead to run until none can make progress;
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

use crate::Rider;
use crate::elevator::{self, Elevator};
use crate::floor::{self, Floor};
use crate::hub::{self, Command, Hub};

/// Hard cap on delivery rounds per dispatch point: a program whose
/// commands keep raising events past this many rounds has the rest
/// delivered at the next dispatch point.
const DISPATCH_ROUNDS: usize = 128;

/// A program running against a challenge's world.
///
/// Minted from a challenge, a seed and the program's `new`, `update` and
/// `subscription` (`transformation-method`): construction builds the
/// world, hands the program its elevators and floors, and runs the tasks
/// `new` returns, so a simulation that exists is under way.
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
type Update<P, M> = dyn Fn(&mut P, M) -> Task<M>;

/// The program's `subscription`.
type Subscribe<P, M> = dyn Fn(&P) -> Subscription<M>;

/// Everything a simulation runs, apart from its hub.
struct Run<P, M> {
    world: World,
    program: P,
    update: Box<Update<P, M>>,
    subscription: Box<Subscribe<P, M>>,
    tasks: Vec<Live<BoxStream<Action<M>>>>,
    tracker: Tracker,
    listeners: Vec<Live<BoxFuture<()>>>,
    sender: mpsc::UnboundedSender<M>,
    receiver: mpsc::UnboundedReceiver<M>,
    /// Each car's trips, in the order of its destination queue.
    queues: Vec<Vec<Stop>>,
    /// Cars that stopped this substep, waiting for riders to get off and
    /// on before their trips end.
    arrivals: Vec<Arrival>,
    failure: Option<Error>,
}

/// One entry of a car's destination queue, and the trips that end there.
struct Stop {
    floor: Floor,
    trips: Vec<Trip>,
}

/// A trip's task: where its messages go, and how to abort it.
struct Trip {
    sender: mpsc::UnboundedSender<elevator::Message>,
    handle: task::Handle,
}

/// A car that stopped, before riders got off and on.
struct Arrival {
    elevator: Elevator,
    floor: Floor,
    /// Riders aboard as it stopped.
    aboard: usize,
    /// Riders aboard who were going here.
    leaving: usize,
    /// The queue entry it reached, when it reached one of the program's.
    stop: Option<Stop>,
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
    /// Builds the world for `challenge` and boots the program on it:
    /// `new` gets the bank's elevators and the building's floors, and
    /// the tasks it returns run before this returns.
    pub fn new(
        challenge: &Challenge,
        seed: u64,
        new: impl FnOnce(Vec<Elevator>, Vec<Floor>) -> (P, Task<M>),
        update: impl Fn(&mut P, M) -> Task<M> + 'static,
        subscription: impl Fn(&P) -> Subscription<M> + 'static,
    ) -> Result<Self, Error> {
        let world = World::new(challenge, seed);
        let elevators = (0..world.elevators().len()).map(Elevator::new).collect();
        let floors = (0..world.floors().len()).map(Floor::from).collect();
        let queues = world.elevators().iter().map(|_| Vec::new()).collect();
        let mut hub = Hub::default();
        let (program, task) = hub::enter(&mut hub, || guard(|| new(elevators, floors)))?;
        let (sender, receiver) = mpsc::unbounded();
        let mut simulation = Self {
            hub,
            run: Run {
                world,
                program,
                update: Box::new(update),
                subscription: Box::new(subscription),
                tasks: Vec::new(),
                tracker: Tracker::new(),
                listeners: Vec::new(),
                sender,
                receiver,
                queues,
                arrivals: Vec::new(),
                failure: None,
            },
        };
        let run = &mut simulation.run;
        hub::enter(&mut simulation.hub, || {
            run.guarded(|run| {
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
                    if run.world.ended() {
                        break;
                    }
                    run.world.step(DT_MAX);
                    run.dispatch();
                    run.world.process_arrivals();
                    run.arrive();
                    run.dispatch();
                }
            })
        })
    }

    /// The live statistics.
    pub fn stats(&self) -> Stats {
        *self.run.world.stats()
    }

    /// How the challenge stands.
    pub fn outcome(&self) -> Outcome {
        self.run.world.outcome()
    }

    /// Whether the challenge has decided.
    pub fn ended(&self) -> bool {
        self.run.world.ended()
    }

    /// The world, for drawing.
    pub fn world(&self) -> &World {
        &self.run.world
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
            let events = self.world.drain_events();
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
            // The program owns its queues: an empty one tells it nothing.
            Event::Idle { .. } => {}
            Event::UpButtonPressed { floor } => {
                hub::broadcast(Rider::Floor(
                    Floor::from(floor),
                    floor::Message::UpButtonPressed,
                ));
            }
            Event::DownButtonPressed { floor } => {
                hub::broadcast(Rider::Floor(
                    Floor::from(floor),
                    floor::Message::DownButtonPressed,
                ));
            }
            Event::FloorButtonPressed { elevator, floor } => {
                hub::broadcast(Rider::Elevator(
                    Elevator::new(elevator),
                    elevator::Message::FloorButtonPressed(Floor::from(floor)),
                ));
            }
            Event::PassingFloor {
                elevator,
                floor,
                direction,
            } => {
                let heading = self.world.elevators()[elevator]
                    .destination_queue()
                    .first()
                    .map(|&level| Floor::from(level.round() as usize));
                if let Some(stop) = self.queues[elevator].first()
                    && Some(stop.floor) == heading
                {
                    for trip in &stop.trips {
                        let _ = trip.sender.unbounded_send(elevator::Message::PassingFloor(
                            Floor::from(floor),
                            direction,
                        ));
                    }
                }
            }
            Event::StoppedAtFloor { elevator, floor } => {
                let floor = Floor::from(floor);
                let queue = &mut self.queues[elevator];
                let stop =
                    (queue.first().map(|stop| stop.floor) == Some(floor)).then(|| queue.remove(0));
                let aboard = self.riders(elevator, |_| true);
                let leaving = self.riders(elevator, |destination| destination == floor.level());
                self.arrivals.push(Arrival {
                    elevator: Elevator::new(elevator),
                    floor,
                    aboard,
                    leaving,
                    stop,
                });
            }
        }
    }

    /// Ends the trips of the cars that arrived, now that riders have got
    /// off and on.
    fn arrive(&mut self) {
        for arrival in std::mem::take(&mut self.arrivals) {
            let index = arrival.elevator.index();
            let car = &self.world.elevators()[index];
            let message = elevator::Message::StoppedAtFloor {
                floor: arrival.floor,
                exited: arrival.leaving,
                boarded: self.riders(index, |_| true) + arrival.leaving - arrival.aboard,
                load_factor: car.load_factor(),
                full: car.is_full(),
            };
            for trip in arrival.stop.into_iter().flat_map(|stop| stop.trips) {
                let _ = trip.sender.unbounded_send(message);
            }
        }
    }

    /// How many riders aboard `elevator` are going where `going` accepts.
    fn riders(&self, elevator: usize, going: impl Fn(usize) -> bool) -> usize {
        self.world
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
        let task = (self.update)(&mut self.program, message);
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

    /// Applies the commands the program's tasks submitted, in order.
    fn apply(&mut self) {
        for command in hub::commands() {
            match command {
                Command::GoToFloor {
                    elevator,
                    floor,
                    first,
                    sender,
                    handle,
                } => {
                    let index = elevator.index();
                    let floor = floor.within(self.world.floors().len());
                    let before = self.world.elevators()[index].destination_queue().len();
                    self.world.go_to_floor(index, floor.level() as f64, first);
                    let queued = self.world.elevators()[index].destination_queue().len() > before;
                    let trip = Trip { sender, handle };
                    let queue = &mut self.queues[index];
                    let adjacent = if first {
                        queue.first_mut()
                    } else {
                        queue.last_mut()
                    };
                    match adjacent {
                        // The car already stops there next (or last): the
                        // trip ends at that same arrival.
                        Some(stop) if !queued && stop.floor == floor => stop.trips.push(trip),
                        _ => {
                            let stop = Stop {
                                floor,
                                trips: vec![trip],
                            };
                            if first {
                                queue.insert(0, stop);
                            } else {
                                queue.push(stop);
                            }
                        }
                    }
                }
                Command::Stop { elevator } => {
                    self.world.stop(elevator.index());
                    for stop in self.queues[elevator.index()].drain(..) {
                        for trip in stop.trips {
                            trip.handle.abort();
                        }
                    }
                }
                Command::GoingUpIndicator { elevator, on } => {
                    self.world
                        .elevator_mut(elevator.index())
                        .set_going_up_indicator(on);
                }
                Command::GoingDownIndicator { elevator, on } => {
                    self.world
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
