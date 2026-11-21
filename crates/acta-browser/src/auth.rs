//! The mutual challenge-response over the pipe (ADR 0010, decision 3). Both
//! sides prove they hold the same key without sending it. The key is derived
//! from the database key that the app and `acta mcp browser` already read
//! from the OS keychain (ADR 0003): nothing new is stored, shown, copied or
//! passed in arguments.

use hmac::{Hmac, Mac};
use sha2::Sha256;

type H = Hmac<Sha256>;

pub const PROTOCOL: &str = "acta-agent-browser/1";
/// The label of `Store::derive_secret` for the pipe key.
pub const KEY_LABEL: &[u8] = b"acta agent-browser ipc v1";

fn tag(key: &[u8; 32], role: &str, a: &[u8], b: &[u8]) -> H {
    let mut m = <H as Mac>::new_from_slice(key).expect("HMAC takes any key length");
    m.update(PROTOCOL.as_bytes());
    m.update(role.as_bytes());
    m.update(a);
    m.update(b);
    m
}

/// The client's proof over (server nonce, client nonce).
pub fn client_mac(key: &[u8; 32], server: &[u8], client: &[u8]) -> String {
    hex(&tag(key, "client", server, client).finalize().into_bytes())
}

/// The server's proof, nonces swapped and another role label, so a client
/// proof can never be replayed as a server proof.
pub fn server_mac(key: &[u8; 32], server: &[u8], client: &[u8]) -> String {
    hex(&tag(key, "server", client, server).finalize().into_bytes())
}

/// Constant-time check of a hex proof made by `role` over (server nonce,
/// client nonce).
pub fn verify(key: &[u8; 32], role: &str, server: &[u8], client: &[u8], proof_hex: &str) -> bool {
    let Some(proof) = unhex(proof_hex) else {
        return false;
    };
    let (a, b) = if role == "server" {
        (client, server)
    } else {
        (server, client)
    };
    tag(key, role, a, b).verify_slice(&proof).is_ok()
}

pub fn nonce() -> [u8; 32] {
    let mut b = [0u8; 32];
    getrandom::fill(&mut b).expect("the system random number generator");
    b
}

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub fn unhex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) || !s.is_ascii() {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proofs_check_and_do_not_cross() {
        let k = [1u8; 32];
        let (s, c) = (nonce(), nonce());
        let cm = client_mac(&k, &s, &c);
        let sm = server_mac(&k, &s, &c);
        assert!(verify(&k, "client", &s, &c, &cm));
        assert!(verify(&k, "server", &s, &c, &sm));
        assert!(!verify(&k, "server", &s, &c, &cm));
        assert!(!verify(&k, "client", &c, &s, &cm));
        assert!(!verify(&[2u8; 32], "client", &s, &c, &cm));
        assert!(!verify(&k, "client", &s, &c, "zz"));
        assert!(!verify(&k, "client", &s, &c, ""));
        assert_ne!(nonce(), nonce());
    }
}
