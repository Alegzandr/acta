// Embeds the app icon on Windows. GPUI's own build script already embeds the
// application manifest (DPI awareness, common controls), so none is added here
// (ADR 0001).
fn main() {
    check_vad();
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

/// The Silero VAD shipped beside the executable (ADR 0014, 4). When the
/// packaging step has put it in `assets/voice/`, its size and SHA-256 must
/// be the pinned ones, or the build fails; without it, dictation runs
/// without the VAD (development builds). Same values as
/// `acta_core::voice::models::SILERO_VAD` (a test keeps them equal).
const VAD_FILE: &str = "ggml-silero-v6.2.0.bin";
const VAD_SIZE: u64 = 885_098;
const VAD_SHA256: &str = "2aa269b785eeb53a82983a20501ddf7c1d9c48e33ab63a41391ac6c9f7fb6987";

fn check_vad() {
    use sha2::Digest as _;
    let path = format!("../../assets/voice/{VAD_FILE}");
    println!("cargo:rerun-if-changed={path}");
    let Ok(bytes) = std::fs::read(&path) else {
        return;
    };
    let hash: String = sha2::Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if bytes.len() as u64 != VAD_SIZE || hash != VAD_SHA256 {
        panic!("{path} is not the pinned Silero VAD (size or SHA-256 differs)");
    }
}
