// Embeds the app icon on Windows. GPUI's own build script already embeds the
// application manifest (DPI awareness, common controls), so none is added here
// (ADR 0001).
fn main() {
    let icon = "../../assets/icons/acta.ico";
    println!("cargo:rerun-if-changed={icon}");
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && let Err(err) = winresource::WindowsResource::new().set_icon(icon).compile()
    {
        // The icon is cosmetic; a toolchain without a resource compiler must
        // still be able to build the app.
        println!("cargo:warning=could not embed the Windows icon: {err}");
    }
}
