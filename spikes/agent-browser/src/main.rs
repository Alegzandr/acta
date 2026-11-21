//! Spike for ADR 0010: Claude Code drives an agent web view inside the
//! running app, in-process through WebView2's DevTools Protocol methods,
//! over an authenticated per-user named pipe. Windows only.
//!
//! `demo` runs everything: a loopback test server, the host (the app's
//! stand-in, a child process) and the client (`acta mcp`'s stand-in).

mod auth;
mod ops;
mod policy;
mod render;

#[cfg(windows)]
mod client;
#[cfg(windows)]
mod host;
#[cfg(windows)]
mod http;
#[cfg(windows)]
mod pipe;

#[cfg(windows)]
fn key_from_env() -> Result<[u8; 32], String> {
    // Stands for the database key read from the keychain (ADR 0003). It
    // travels in the environment, never on a command line.
    let secret = std::env::var("SPIKE_SECRET").map_err(|_| "SPIKE_SECRET missing")?;
    let bytes = auth::unhex(&secret).ok_or("SPIKE_SECRET is not hex")?;
    Ok(auth::derive(&bytes))
}

#[cfg(windows)]
fn main() {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "demo".into());
    let dir = std::env::var_os("SPIKE_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("acta-spike-agent-browser"));
    let _ = std::fs::create_dir_all(&dir);
    let result = match mode.as_str() {
        "host" => key_from_env()
            .and_then(|k| host::run(k, std::env::var("SPIKE_BASE").unwrap_or_default(), dir)),
        "demo" => demo(&dir),
        _ => Err("usage: acta-spike-agent-browser [demo|host]".into()),
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

#[cfg(windows)]
fn demo(dir: &std::path::Path) -> Result<(), String> {
    let base = http::start().map_err(|e| e.to_string())?;
    println!(
        "test server {base}; profiles and screenshots in {}",
        dir.display()
    );
    let secret = auth::hex(&auth::nonce());
    let mut child = std::process::Command::new(std::env::current_exe().map_err(|e| e.to_string())?)
        .arg("host")
        .env("SPIKE_SECRET", &secret)
        .env("SPIKE_BASE", &base)
        .env("SPIKE_DIR", dir)
        .spawn()
        .map_err(|e| e.to_string())?;
    let key = auth::derive(&auth::unhex(&secret).unwrap_or_default());
    let t = std::time::Instant::now();
    let r = client::run(key, &base, dir, child.id());
    println!("client finished in {:.1} s", t.elapsed().as_secs_f64());
    if r.is_err() {
        let _ = child.kill();
    }
    let _ = child.wait();
    r
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Windows only; `cargo test` runs the portable parts.");
}
