//! The rhai `Elevator` handle: the elevator half of the scripting API.

use std::cell::RefCell;
use std::rc::Rc;

use elevato_core::World;
use elevato_core::event::Direction;
use rhai::{Array, Dynamic, Engine, EvalAltResult, FnPtr, ImmutableString};

use crate::api::{self, Binding, Hook};

/// A cheap clonable script-side reference to one elevator: the shared
/// world, the shared handler registry, and an index. Every method
/// borrows the world only for the duration of that call, so script code
/// never runs under a live borrow (see [`crate::api`]).
#[derive(Debug, Clone)]
pub(crate) struct Handle {
    world: Rc<RefCell<World>>,
    registry: Rc<RefCell<api::Registry>>,
    index: usize,
}

impl Handle {
    /// A handle to elevator `index` of the runtime's world.
    pub(crate) fn new(
        world: Rc<RefCell<World>>,
        registry: Rc<RefCell<api::Registry>>,
        index: usize,
    ) -> Self {
        Self {
            world,
            registry,
            index,
        }
    }

    fn go_to_floor(&self, floor: f64, force: bool) {
        self.world
            .borrow_mut()
            .go_to_floor(self.index, floor, force);
    }

    fn stop(&self) {
        self.world.borrow_mut().stop(self.index);
    }

    fn check_destination_queue(&self) {
        self.world.borrow_mut().check_destination_queue(self.index);
    }

    fn current_floor(&self) -> i64 {
        self.world.borrow().elevators()[self.index].current_floor() as i64
    }

    fn max_passenger_count(&self) -> i64 {
        self.world.borrow().elevators()[self.index].capacity() as i64
    }

    fn load_factor(&self) -> f64 {
        self.world.borrow().elevators()[self.index].load_factor()
    }

    fn destination_direction(&self) -> ImmutableString {
        match self.world.borrow().elevators()[self.index].destination_direction() {
            Some(Direction::Up) => "up".into(),
            Some(Direction::Down) => "down".into(),
            None => "stopped".into(),
        }
    }

    fn pressed_floors(&self) -> Array {
        self.world.borrow().elevators()[self.index]
            .pressed_floors()
            .into_iter()
            .map(|level| Dynamic::from(level as i64))
            .collect()
    }

    /// The queue as a rhai array — a *value copy*: mutating it cannot
    /// affect the elevator. Integral levels come back as ints so queue
    /// entries compare cleanly against floor numbers.
    fn destination_queue(&self) -> Array {
        self.world.borrow().elevators()[self.index]
            .destination_queue()
            .iter()
            .map(|&level| {
                if level.fract() == 0.0 {
                    Dynamic::from(level as i64)
                } else {
                    Dynamic::from(level)
                }
            })
            .collect()
    }

    fn set_destination_queue(&self, queue: Array) -> Result<(), Box<EvalAltResult>> {
        let mut levels = Vec::with_capacity(queue.len());
        for item in queue {
            let level = if let Ok(int) = item.as_int() {
                int as f64
            } else if let Ok(float) = item.as_float() {
                float
            } else {
                return Err(api::runtime_error(format!(
                    "set_destination_queue expects numbers, found {}",
                    item.type_name()
                )));
            };
            levels.push(level);
        }
        self.world
            .borrow_mut()
            .elevator_mut(self.index)
            .set_destination_queue(levels);
        Ok(())
    }

    fn is_full(&self) -> bool {
        self.world.borrow().elevators()[self.index].is_full()
    }

    fn move_count(&self) -> i64 {
        self.world.borrow().elevators()[self.index].move_count() as i64
    }

    fn is_busy(&self) -> bool {
        self.world.borrow().elevators()[self.index].is_busy()
    }

    fn is_moving(&self) -> bool {
        self.world.borrow().elevators()[self.index].is_moving()
    }

    fn is_on_a_floor(&self) -> bool {
        self.world.borrow().elevators()[self.index].is_on_a_floor()
    }

    fn going_up_indicator(&self) -> bool {
        self.world.borrow().elevators()[self.index].going_up_indicator()
    }

    fn set_going_up_indicator(&self, on: bool) {
        self.world
            .borrow_mut()
            .elevator_mut(self.index)
            .set_going_up_indicator(on);
    }

    fn going_down_indicator(&self) -> bool {
        self.world.borrow().elevators()[self.index].going_down_indicator()
    }

    fn set_going_down_indicator(&self, on: bool) {
        self.world
            .borrow_mut()
            .elevator_mut(self.index)
            .set_going_down_indicator(on);
    }

    /// Binds a handler to one event name — or several, space-separated,
    /// in which case the event name is prepended as the handler's first
    /// argument (riot's multi-event semantics). Unknown names are a
    /// bind-time error rather than the original's silent never-firing
    /// listener (documented deviation).
    fn on(&self, events: &str, handler: FnPtr) -> Result<(), Box<EvalAltResult>> {
        if self.registry.borrow().tea {
            return Err(api::runtime_error(
                "this program defines `fn model`: events arrive through \
                 `fn update(message, elevators, floors)`, not `on(...)`"
                    .to_string(),
            ));
        }
        let names: Vec<&str> = events.split_whitespace().collect();
        let multi = names.len() > 1;
        for name in names {
            let hook = self
                .hook(name)
                .ok_or_else(|| api::runtime_error(format!("unknown elevator event: `{name}`")))?;
            self.registry.borrow_mut().bind(
                hook,
                Binding {
                    fn_ptr: handler.clone(),
                    prepended_name: multi.then(|| name.into()),
                },
            );
        }
        Ok(())
    }

    fn hook(&self, name: &str) -> Option<Hook> {
        let elevator = self.index;
        match name {
            "idle" => Some(Hook::Idle { elevator }),
            "floor_button_pressed" => Some(Hook::FloorButtonPressed { elevator }),
            "passing_floor" => Some(Hook::PassingFloor { elevator }),
            "stopped_at_floor" => Some(Hook::StoppedAtFloor { elevator }),
            _ => None,
        }
    }
}

/// Registers the `Elevator` type: commands, properties, and `on`.
/// `go_to_floor` accepts ints and floats (the original coerced with
/// `Number()`); the world clamps to the building either way.
pub(crate) fn register(engine: &mut Engine) {
    engine
        .register_type_with_name::<Handle>("Elevator")
        .register_fn("go_to_floor", |handle: &mut Handle, floor: i64| {
            handle.go_to_floor(floor as f64, false);
        })
        .register_fn("go_to_floor", |handle: &mut Handle, floor: f64| {
            handle.go_to_floor(floor, false);
        })
        .register_fn(
            "go_to_floor",
            |handle: &mut Handle, floor: i64, force: bool| {
                handle.go_to_floor(floor as f64, force);
            },
        )
        .register_fn(
            "go_to_floor",
            |handle: &mut Handle, floor: f64, force: bool| {
                handle.go_to_floor(floor, force);
            },
        )
        .register_fn("stop", |handle: &mut Handle| handle.stop())
        .register_fn("check_destination_queue", |handle: &mut Handle| {
            handle.check_destination_queue();
        })
        .register_fn(
            "set_destination_queue",
            |handle: &mut Handle, queue: Array| handle.set_destination_queue(queue),
        )
        .register_fn("on", |handle: &mut Handle, events: &str, handler: FnPtr| {
            handle.on(events, handler)
        })
        .register_get("current_floor", |handle: &mut Handle| {
            handle.current_floor()
        })
        .register_get("max_passenger_count", |handle: &mut Handle| {
            handle.max_passenger_count()
        })
        .register_get("load_factor", |handle: &mut Handle| handle.load_factor())
        .register_get("destination_direction", |handle: &mut Handle| {
            handle.destination_direction()
        })
        .register_get("is_full", |handle: &mut Handle| handle.is_full())
        .register_get("move_count", |handle: &mut Handle| handle.move_count())
        .register_get("is_busy", |handle: &mut Handle| handle.is_busy())
        .register_get("is_moving", |handle: &mut Handle| handle.is_moving())
        .register_get("is_on_a_floor", |handle: &mut Handle| {
            handle.is_on_a_floor()
        })
        .register_get("pressed_floors", |handle: &mut Handle| {
            handle.pressed_floors()
        })
        .register_get("destination_queue", |handle: &mut Handle| {
            handle.destination_queue()
        })
        .register_get_set(
            "going_up_indicator",
            |handle: &mut Handle| handle.going_up_indicator(),
            |handle: &mut Handle, on: bool| handle.set_going_up_indicator(on),
        )
        .register_get_set(
            "going_down_indicator",
            |handle: &mut Handle| handle.going_down_indicator(),
            |handle: &mut Handle, on: bool| handle.set_going_down_indicator(on),
        );
}
