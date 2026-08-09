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

    let settings = iced::Settings {
        // Safari now ships WebGPU, so wgpu auto-detects it on iOS — and
        // renders black. WebGL2 is the proven web path; force it on
        // wasm (Chrome included: consistency beats novelty here).
        #[cfg(target_arch = "wasm32")]
        backend: iced::Backend::Hardware(iced::backend::Api::OpenGL),
        ..iced::Settings::default()
    };

    iced::application(app::boot, app::update, app::view)
        .settings(settings)
        .subscription(app::subscription)
        .theme(app::theme)
        .font(theme::MONO_BYTES)
        .font(elevato::icon::FONT)
        .default_font(theme::MONO)
        .title("elevato.rs")
        .run()
}
