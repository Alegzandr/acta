//! Mutual challenge-response over the pipe (ADR 0010, decision 3). Both
//! sides prove they hold the same key without sending it. In the product the
//! key is derived from the database key both the app and `acta mcp` already
//! read from the OS keychain (ADR 0003), so nothing new is stored, copied or
//! passed on a command line. The spike stands it in with `SPIKE_SECRET`.

use hmac::{Hmac, Mac};
use sha2::Sha256;

type H = Hmac<Sha256>;

pub const PROTOCOL: &str = "acta-agent-browser/1";

/// The pipe key: `HMAC(database key, label)`, so it opens nothing else.
pub fn derive(secret: &[u8]) -> [u8; 32] {
    let mut m = H::new_from_slice(secret).expect("any key length");
    m.update(b"acta agent-browser ipc v1");
    m.finalize().into_bytes().into()
}

fn tag(key: &[u8; 32], role: &str, a: &[u8], b: &[u8]) -> H {
    let mut m = H::new_from_slice(key).expect("32 bytes");
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

/// The server's proof, with the nonces swapped and another role label, so a
/// client proof can never be replayed as a server proof.
pub fn server_mac(key: &[u8; 32], server: &[u8], client: &[u8]) -> String {
    hex(&tag(key, "server", client, server).finalize().into_bytes())
}

/// Constant-time check of a hex proof.
pub fn verify(key: &[u8; 32], role: &str, a: &[u8], b: &[u8], proof_hex: &str) -> bool {
    let Some(proof) = unhex(proof_hex) else {
        return false;
    };
    let (a, b) = if role == "server" { (b, a) } else { (a, b) };
    tag(key, role, a, b).verify_slice(&proof).is_ok()
}

pub fn nonce() -> [u8; 32] {
    let mut b = [0u8; 32];
    getrandom::getrandom(&mut b).expect("system RNG");
    b
}

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub fn unhex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
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
        let k = derive(b"alexandre's database key");
        let (s, c) = (nonce(), nonce());
        let cm = client_mac(&k, &s, &c);
        let sm = server_mac(&k, &s, &c);
        assert!(verify(&k, "client", &s, &c, &cm));
        assert!(verify(&k, "server", &s, &c, &sm));
        assert!(!verify(&k, "server", &s, &c, &cm));
        assert!(!verify(&k, "client", &c, &s, &cm));
        let other = derive(b"another key");
        assert!(!verify(&other, "client", &s, &c, &cm));
        assert!(!verify(&k, "client", &s, &c, "zz"));
    }
}
