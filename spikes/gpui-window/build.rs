// Embeds the app icon on Windows. GPUI's own build script already embeds the
// application manifest (DPI awareness, common controls), so we add no manifest here.
fn main() {
    println!("cargo:rerun-if-changed=assets/acta-placeholder.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("assets/acta-placeholder.ico")
            .compile()
            .expect("failed to embed Windows resources");
    }
}
