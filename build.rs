fn main() -> std::io::Result<()> {
    // Generates `src/icon.rs` + the tree-shaken `assets/fonts/lucide.ttf` from
    // the icon list - both targets use the icons.
    println!("cargo::rerun-if-changed=assets/fonts/icons.toml");
    iced_lucide::build("assets/fonts/icons.toml").expect("build elevato icons");

    Ok(())
}
