/// One floor of the building. Fields are read-only.
pub struct Floor {
    /// The floor's number; ground is 0. Also callable, `floor_num()`.
    pub floor_num: i64,

    /// Alias of `floor_num`.
    pub level: i64,

    /// Whether the up call button is lit.
    pub up_pressed: bool,

    /// Whether the down call button is lit.
    pub down_pressed: bool,
}

impl Floor {
    /// Binds a handler to an `Event` by name, or to several via a
    /// space-separated string — the event's name is then prepended as
    /// the handler's first argument. Handlers receive the floor.
    pub fn on(&self, events: &str, handler: impl FnMut(Floor));
}

/// Dispatched by name: `floor.on("up_button_pressed", |floor| …)`.
pub enum Event {
    /// `"up_button_pressed"` — the up call button went from unlit to
    /// lit. Re-fires when riders who could not board press again
    /// after an arrival cleared it.
    UpButtonPressed { floor: Floor },

    /// `"down_button_pressed"` — likewise, for down.
    DownButtonPressed { floor: Floor },
}
