//! Navigation and input policy of the agent browser (ADR 0010, decision 5).
//! Pure functions: the host applies them to every top-level navigation
//! (WebView2 `NavigationStarting`, which also covers links the page or the
//! agent clicks) and before every `navigate` and `type`.

use url::Url;

/// Sites the agent browser never opens: the claude.ai account lives in the
/// chat panel's profile and must never be automated (ADR 0009, Consumer
/// Terms section 3).
const NEVER: &[&str] = &["claude.ai", "claude.com", "anthropic.com"];

#[derive(Clone, Copy, Debug)]
pub struct Policy {
    /// Spike only: loopback hosts and nothing else, so no test leaves the
    /// machine. The product allows every `http(s)` host but [`NEVER`].
    pub local_only: bool,
}

fn under(host: &str, domain: &str) -> bool {
    host == domain || host.ends_with(&format!(".{domain}"))
}

pub fn is_loopback(host: &str) -> bool {
    host == "localhost" || host == "127.0.0.1" || host == "[::1]" || host.ends_with(".localhost")
}

impl Policy {
    /// `Ok` with the parsed URL, or the one-line reason given to the agent.
    pub fn check(&self, raw: &str) -> Result<Url, String> {
        if raw == "about:blank" {
            return Url::parse(raw).map_err(|e| e.to_string());
        }
        let url = Url::parse(raw).map_err(|_| format!("refused: not a URL: {}", clip(raw, 80)))?;
        match url.scheme() {
            "http" | "https" => {}
            s => {
                return Err(format!(
                    "refused: {s}: URLs are not opened by the agent browser"
                ));
            }
        }
        let host = url.host_str().unwrap_or("").to_ascii_lowercase();
        if host.is_empty() {
            return Err("refused: no host".into());
        }
        if NEVER.iter().any(|d| under(&host, d)) {
            return Err(format!(
                "refused: {host} is never automated (the Claude account stays in the chat panel)"
            ));
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err("refused: credentials in the URL".into());
        }
        if self.local_only && !is_loopback(&host) {
            return Err(format!("refused: {host} (spike: loopback only)"));
        }
        Ok(url)
    }
}

/// Whether an element is a field the agent never fills: passwords, one-time
/// codes, payment card data. `attrs` is CDP's flat `[name, value, ...]`.
pub fn sensitive_field(node_name: &str, attrs: &[String]) -> bool {
    if !node_name.eq_ignore_ascii_case("input") {
        return false;
    }
    let get = |k: &str| {
        attrs
            .chunks(2)
            .find(|p| p[0].eq_ignore_ascii_case(k))
            .map(|p| p.get(1).cloned().unwrap_or_default().to_ascii_lowercase())
    };
    if get("type").as_deref() == Some("password") {
        return true;
    }
    let ac = get("autocomplete").unwrap_or_default();
    ac.split_whitespace().any(|t| {
        t == "current-password"
            || t == "new-password"
            || t == "one-time-code"
            || t.starts_with("cc-")
    })
}

pub fn clip(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(n.saturating_sub(3)).collect();
        out.push_str("...");
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRODUCT: Policy = Policy { local_only: false };
    const SPIKE: Policy = Policy { local_only: true };

    #[test]
    fn schemes_other_than_http_are_refused() {
        for u in [
            "file:///C:/Windows/win.ini",
            "data:text/html,<p>x",
            "javascript:alert(1)",
            "edge://settings",
            "chrome://version",
            "ftp://example.com/",
            "view-source:https://example.com",
        ] {
            assert!(PRODUCT.check(u).is_err(), "{u}");
        }
        assert!(PRODUCT.check("about:blank").is_ok());
        assert!(PRODUCT.check("https://example.com/a?b").is_ok());
    }

    #[test]
    fn the_claude_account_sites_are_never_opened() {
        for u in [
            "https://claude.ai/",
            "https://www.claude.ai/new",
            "https://CLAUDE.AI/",
            "https://console.anthropic.com/",
            "https://claude.com/",
        ] {
            assert!(
                PRODUCT.check(u).unwrap_err().contains("never automated"),
                "{u}"
            );
        }
        assert!(PRODUCT.check("https://notclaude.ai/").is_ok());
    }

    #[test]
    fn credentials_in_urls_are_refused() {
        assert!(PRODUCT.check("https://alexandre:pw@example.com/").is_err());
    }

    #[test]
    fn the_spike_stays_on_loopback() {
        assert!(SPIKE.check("http://127.0.0.1:8080/form.html").is_ok());
        assert!(SPIKE.check("http://localhost:3000/").is_ok());
        assert!(SPIKE.check("http://app.localhost/").is_ok());
        assert!(SPIKE.check("https://example.com/").is_err());
    }

    #[test]
    fn secret_fields_are_recognised() {
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(sensitive_field("INPUT", &a(&["type", "Password"])));
        assert!(sensitive_field(
            "input",
            &a(&["autocomplete", "one-time-code"])
        ));
        assert!(sensitive_field(
            "input",
            &a(&["autocomplete", "billing cc-number"])
        ));
        assert!(!sensitive_field(
            "input",
            &a(&["type", "text", "name", "password_hint"])
        ));
        assert!(!sensitive_field("textarea", &a(&["type", "password"])));
    }
}
