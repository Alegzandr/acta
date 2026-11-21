//! The real named pipe (Windows): the DACL read back from the handle, a
//! squatter making the server fail, and clients in another process proving
//! (or failing to prove) the key.
#![cfg(windows)]

use std::process::Command;
use std::thread;

use acta_browser::args::{ConsoleArgs, Reply, Request};
use acta_browser::client::{Client, ConnectError};
use acta_browser::pipe::{self, Server};
use acta_browser::wire::{Conn, Hello};

const CHILD_PIPE: &str = "ACTA_BROWSER_TEST_PIPE";
const CHILD_KEY: &str = "ACTA_BROWSER_TEST_KEY";

fn unique_name() -> String {
    let mut b = [0u8; 8];
    getrandom::fill(&mut b).unwrap();
    let tag: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!(r"\\.\pipe\acta-test-{tag}")
}

/// A server that authenticates with `key` and answers every request with
/// "ok"; returns the DACL of each accepted instance.
fn serve(name: &str, key: [u8; 32], clients: usize) -> thread::JoinHandle<Vec<String>> {
    let sid = pipe::user_sid().unwrap();
    let mut server = Server::new(name, &sid).unwrap();
    thread::spawn(move || {
        let mut dacls = Vec::new();
        for _ in 0..clients {
            let accepted = server.accept().unwrap();
            dacls.push(accepted.dacl);
            let writer = accepted.file.try_clone().unwrap();
            let mut conn = Conn::new(accepted.file, writer);
            if acta_browser::wire::server_handshake(&mut conn, &key).is_ok() {
                while let Ok(Some(_)) = conn.recv::<Request>(65536) {
                    conn.send(&Reply::ok("ok")).unwrap();
                }
            }
        }
        dacls
    })
}

/// Runs this test binary's `child_client` in another process.
fn child(name: &str, key: [u8; 32]) -> String {
    let out = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child_client", "--nocapture", "--test-threads=1"])
        .env(CHILD_PIPE, name)
        .env(CHILD_KEY, acta_browser::auth::hex(&key))
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).to_string()
}

#[test]
fn child_client() {
    // Only does something when started by `child` above.
    let (Ok(name), Ok(key)) = (std::env::var(CHILD_PIPE), std::env::var(CHILD_KEY)) else {
        return;
    };
    let key: [u8; 32] = acta_browser::auth::unhex(&key).unwrap().try_into().unwrap();
    match Client::connect(&name, &key, &Hello::default()) {
        Ok(mut c) => {
            let r = c.call(&Request::Console(ConsoleArgs::default())).unwrap();
            println!("CHILD connected: {}", r.text);
        }
        Err(ConnectError::NoApp) => println!("CHILD no app"),
        Err(ConnectError::Failed(e)) => println!("CHILD failed: {e}"),
    }
}

#[test]
fn only_this_user_is_admitted_and_network_logons_are_denied() {
    let name = unique_name();
    let key = [5u8; 32];
    let server = serve(&name, key, 1);
    let mut c = Client::connect(&name, &key, &Hello::default()).unwrap();
    assert_eq!(
        c.call(&Request::Console(ConsoleArgs::default()))
            .unwrap()
            .text,
        "ok"
    );
    drop(c);
    let dacl = server.join().unwrap().remove(0);
    let sid = pipe::user_sid().unwrap();
    // Read back from the handle: protected, network denied, the user
    // admitted, nobody else.
    assert!(dacl.starts_with("D:P"), "{dacl}");
    assert!(dacl.contains(";;;NU)") && dacl.contains("(D;"), "{dacl}");
    assert!(dacl.contains(&format!(";;;{sid})")), "{dacl}");
    assert_eq!(dacl.matches('(').count(), 2, "{dacl}");
}

#[test]
fn another_process_needs_the_key() {
    let name = unique_name();
    let key = [6u8; 32];
    let server = serve(&name, key, 2);
    let wrong = child(&name, [7u8; 32]);
    assert!(
        wrong.contains("CHILD failed: the app refused this client"),
        "{wrong}"
    );
    let right = child(&name, key);
    assert!(right.contains("CHILD connected: ok"), "{right}");
    server.join().unwrap();
}

#[test]
fn a_squatted_name_makes_the_server_fail() {
    let name = unique_name();
    let sid = pipe::user_sid().unwrap();
    let squatter = serve(&name, [1u8; 32], 1);
    // The squatter's instance exists: a first-instance creation fails.
    let mut ours = Server::new(&name, &sid).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert!(ours.accept().is_err());
    // Unblock the squatter's accept.
    let _ = Client::connect(&name, &[1u8; 32], &Hello::default());
    squatter.join().unwrap();
}

#[test]
fn nobody_serving_means_no_app() {
    assert!(matches!(
        Client::connect(&unique_name(), &[0; 32], &Hello::default()),
        Err(ConnectError::NoApp)
    ));
}
