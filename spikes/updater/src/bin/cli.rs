//! Stand-in for the Acta CLI / MCP stdio server: console subsystem.

use acta_spike::{log, VERSION};
use std::io::{BufRead, Write};
use std::time::Duration;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("mcp") => {
            // Long-lived stdio server: echo lines until stdin closes.
            log("mcp", &format!("started from {:?}", std::env::current_exe().ok()));
            std::thread::spawn(|| loop {
                std::thread::sleep(Duration::from_secs(2));
                log("mcp", "alive");
            });
            let stdin = std::io::stdin();
            let mut out = std::io::stdout();
            for line in stdin.lock().lines().map_while(Result::ok) {
                let _ = writeln!(out, "{{\"server\":\"{VERSION}\",\"echo\":\"{line}\"}}");
                let _ = out.flush();
            }
            log("mcp", "stdin closed, exiting");
        }
        _ => println!("acta {VERSION} ({})", std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default()),
    }
}
