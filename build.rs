fn main() -> std::io::Result<()> {
    // Generates `src/icon.rs` + the tree-shaken `assets/fonts/lucide.ttf`
    // from the icon list - both targets use the icons.
    println!("cargo::rerun-if-changed=assets/fonts/icons.toml");
    iced_lucide::build("assets/fonts/icons.toml").expect("build elevato icons");

    // granita's scan emits `-Wl,-rpath` link-arg-bins that rust-lld
    // rejects on wasm32, and its registry only feeds the native-only
    // preview macros - skip it entirely for web builds.
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        return Ok(());
    }
    granita_build::scan()
}
