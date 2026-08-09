//! The rhai `Floor` handle: the floor half of the scripting API.

use std::cell::RefCell;
use std::rc::Rc;

use elevato_core::World;
use rhai::{Engine, EvalAltResult, FnPtr};

use crate::api::{self, Binding, Hook};

/// A cheap clonable script-side reference to one floor; same borrow
/// discipline as [`crate::api::elevator::Handle`]. Floor handles are
/// also what `up_button_pressed` / `down_button_pressed` handlers
/// receive as their argument, like the original passed the floor object.
#[derive(Debug, Clone)]
pub(crate) struct Handle {
    world: Rc<RefCell<World>>,
    registry: Rc<RefCell<api::Registry>>,
    index: usize,
}

impl Handle {
    /// A handle to floor `index` of the runtime's world.
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

    fn floor_num(&self) -> i64 {
        self.index as i64
    }

    fn up_pressed(&self) -> bool {
        self.world.borrow().floors()[self.index].up_pressed()
    }

    fn down_pressed(&self) -> bool {
        self.world.borrow().floors()[self.index].down_pressed()
    }

    /// Same bind semantics as the elevator's `on` (multi-event prepends
    /// the name; unknown names error at bind time).
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
                .ok_or_else(|| api::runtime_error(format!("unknown floor event: `{name}`")))?;
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
        let floor = self.index;
        match name {
            "up_button_pressed" => Some(Hook::UpButtonPressed { floor }),
            "down_button_pressed" => Some(Hook::DownButtonPressed { floor }),
            _ => None,
        }
    }
}

/// Registers the `Floor` type. `floor_num` is exposed both as a property
/// and as a method — the original's `floor.floorNum()` was a call, and
/// JS muscle memory deserves the parentheses to keep working. `level` is
/// the original's raw property of the same value.
pub(crate) fn register(engine: &mut Engine) {
    engine
        .register_type_with_name::<Handle>("Floor")
        .register_fn("floor_num", |handle: &mut Handle| handle.floor_num())
        .register_get("floor_num", |handle: &mut Handle| handle.floor_num())
        .register_get("level", |handle: &mut Handle| handle.floor_num())
        .register_get("up_pressed", |handle: &mut Handle| handle.up_pressed())
        .register_get("down_pressed", |handle: &mut Handle| handle.down_pressed())
        .register_fn("on", |handle: &mut Handle, events: &str, handler: FnPtr| {
            handle.on(events, handler)
        });
}
