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
    // Release links on MSVC warn LNK4099 (a dependency's static library
    // ships without its PDB). Harmless, and only seen there: silenced for
    // the release binary alone, so debug builds and their caches are
    // untouched.
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
        && std::env::var("PROFILE").as_deref() == Ok("release")
    {
        println!("cargo:rustc-link-arg-bins=/ignore:4099");
    }
}
