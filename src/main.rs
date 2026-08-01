use elevato::{app, theme};

// No `#[granita::app]`: the app state owns the rhai runtime
// (`Rc<RefCell<World>>`, engine, handler FnPtrs), which is `!Send` and
// cannot cross granita's reload boundary. Granita serves previews only;
// the app itself runs via plain `cargo run`.
pub fn main() -> iced::Result {
    #[cfg(target_arch = "wasm32")]
    {
        console_log::init().expect("initialize logger");
        std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    }

    iced::application(app::boot, app::update, app::view)
        .subscription(app::subscription)
        .font(theme::MONO_BYTES)
        .title("elevato")
        .run()
}
