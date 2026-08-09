/// One floor of the building: its number, its call-button state, and
/// the two call-button `Event`s via `on`.
///
/// All fields are read-only.
pub struct Floor {
    /// This floor's number, ground = 0. Also callable as a method —
    /// `floor.floor_num()` — for ported muscle memory.
    pub floor_num: i64,

    /// Alias of `floor_num`, under its original raw-property name.
    pub level: i64,

    /// True while the up call button is lit. Polling these two from
    /// `update` instead of listening to events is the "Twentyliner"
    /// strategy family.
    pub up_pressed: bool,

    /// True while the down call button is lit.
    pub down_pressed: bool,
}

impl Floor {
    /// Subscribes a handler to one `Event` by name — or several via
    /// a space-separated string, in which case the event's name is
    /// prepended as the handler's first argument. Handlers receive
    /// the floor handle.
    pub fn on(&self, events: &str, handler: impl FnMut(Floor));
}

/// Everything a `Floor` reports, by name:
/// `floor.on("up_button_pressed", |floor| …)`.
pub enum Event {
    /// `"up_button_pressed"` — the up call button went from unlit
    /// to lit. Re-fires when riders who could not fit press again
    /// after an arrival cleared it.
    UpButtonPressed { floor: Floor },

    /// `"down_button_pressed"` — same, for the down button.
    DownButtonPressed { floor: Floor },
}
