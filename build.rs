fn main() -> std::io::Result<()> {
    // granita's scan emits `-Wl,-rpath` link-arg-bins that rust-lld
    // rejects on wasm32, and its registry only feeds the native-only
    // preview macros — skip it entirely for web builds.
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        return Ok(());
    }
    granita_build::scan()
}
