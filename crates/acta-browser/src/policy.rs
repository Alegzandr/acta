//! What the agent browser never does (ADR 0010, decision 5). Pure functions:
//! the app applies [`Policy::check`] to every top-level navigation (WebView2
//! `NavigationStarting`, which also covers links the page or the agent
//! follows), and the operations apply it before `open` and the field rule
//! before `type`.

use url::Url;

/// Sites the agent browser never opens: the Claude account lives in the
/// chat panel's profile and is never automated (ADR 0009, ADR 0010).
pub const NEVER: &[&str] = &["claude.ai", "claude.com", "anthropic.com"];

/// The navigation policy. `blocked` is the user's blocked-sites list
/// (hosts; a host blocks its subdomains too).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Policy {
    pub blocked: Vec<String>,
}

fn under(host: &str, domain: &str) -> bool {
    host == domain || host.ends_with(&format!(".{domain}"))
}

/// Why a navigation is refused, as one line for the agent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    NotAUrl,
    Scheme(String),
    NoHost,
    Never(String),
    Credentials,
    Blocked(String),
}

impl Refusal {
    /// The line the agent reads. Hosts here came from the agent's own
    /// request or from a page; callers fence them when they came from a page.
    pub fn line(&self) -> String {
        match self {
            Refusal::NotAUrl => "refused: not a URL".into(),
            Refusal::Scheme(s) => format!("refused: {s}: URLs are not opened by the agent browser"),
            Refusal::NoHost => "refused: the URL has no host".into(),
            Refusal::Never(h) => format!(
                "refused: {h} is never automated (the Claude account stays in the chat panel)"
            ),
            Refusal::Credentials => "refused: credentials in the URL".into(),
            Refusal::Blocked(h) => format!("refused: {h} is on the user's blocked sites"),
        }
    }
}

impl Policy {
    /// `Ok` with the parsed URL, or why not.
    pub fn check(&self, raw: &str) -> Result<Url, Refusal> {
        if raw == "about:blank" {
            return Url::parse(raw).map_err(|_| Refusal::NotAUrl);
        }
        let url = Url::parse(raw.trim()).map_err(|_| Refusal::NotAUrl)?;
        match url.scheme() {
            "http" | "https" => {}
            s => return Err(Refusal::Scheme(s.into())),
        }
        let host = url.host_str().unwrap_or("").to_ascii_lowercase();
        let host = host.trim_end_matches('.').to_string();
        if host.is_empty() {
            return Err(Refusal::NoHost);
        }
        if NEVER.iter().any(|d| under(&host, d)) {
            return Err(Refusal::Never(host));
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(Refusal::Credentials);
        }
        if self
            .blocked
            .iter()
            .map(|b| b.trim().trim_end_matches('.').to_ascii_lowercase())
            .any(|b| !b.is_empty() && under(&host, &b))
        {
            return Err(Refusal::Blocked(host));
        }
        Ok(url)
    }
}

/// Scheme and host only (scheme alone when there is no host): what the log,
/// the activity line and `blocked:` lines name. Paths and queries can carry
/// tokens.
pub fn origin_of(raw: &str) -> String {
    match Url::parse(raw) {
        Ok(url) => match url.host_str() {
            Some(host) => match url.port() {
                Some(p) => format!("{}://{host}:{p}", url.scheme()),
                None => format!("{}://{host}", url.scheme()),
            },
            None => format!("{}:", url.scheme()),
        },
        Err(_) => "(not a URL)".into(),
    }
}

/// Whether an element is a field the agent never fills: passwords, one-time
/// codes, payment card data. `attrs` is CDP's flat `[name, value, ...]`.
pub fn secret_field(node_name: &str, attrs: &[String]) -> bool {
    if !node_name.eq_ignore_ascii_case("input") {
        return false;
    }
    let get = |k: &str| {
        attrs
            .chunks(2)
            .find(|p| p[0].eq_ignore_ascii_case(k))
            .map(|p| p.get(1).cloned().unwrap_or_default().to_ascii_lowercase())
    };
    if get("type").as_deref().map(str::trim) == Some("password") {
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

/// At most `n` characters, with "..." when cut.
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

    fn p() -> Policy {
        Policy::default()
    }

    #[test]
    fn only_http_and_https_are_opened() {
        for u in [
            "file:///C:/Windows/win.ini",
            "data:text/html,<p>x",
            "javascript:alert(1)",
            "edge://settings",
            "chrome://version",
            "ftp://example.com/",
            "view-source:https://example.com",
            "blob:https://example.com/x",
            "not a url",
        ] {
            assert!(p().check(u).is_err(), "{u}");
        }
        assert!(p().check("about:blank").is_ok());
        assert!(p().check("https://example.com/a?b").is_ok());
        assert!(p().check("http://127.0.0.1:5173/").is_ok());
    }

    #[test]
    fn the_claude_account_sites_are_never_opened() {
        for u in [
            "https://claude.ai/",
            "https://www.claude.ai/new",
            "https://CLAUDE.AI/",
            "https://claude.ai./",
            "https://console.anthropic.com/",
            "https://claude.com/",
            "http://claude.ai/",
        ] {
            assert!(matches!(p().check(u), Err(Refusal::Never(_))), "{u}");
        }
        assert!(p().check("https://notclaude.ai/").is_ok());
        assert!(p().check("https://claude.ai.example.com/").is_ok());
    }

    #[test]
    fn credentials_in_urls_are_refused() {
        assert_eq!(
            p().check("https://alexandre:pw@example.com/"),
            Err(Refusal::Credentials)
        );
        assert_eq!(
            p().check("https://alexandre@example.com/"),
            Err(Refusal::Credentials)
        );
    }

    #[test]
    fn blocked_sites_block_their_subdomains() {
        let policy = Policy {
            blocked: vec!["Bank.example".into(), " ".into()],
        };
        assert!(matches!(
            policy.check("https://www.bank.example/"),
            Err(Refusal::Blocked(_))
        ));
        assert!(policy.check("https://bank.example.org/").is_ok());
    }

    #[test]
    fn origins_drop_path_and_query() {
        assert_eq!(
            origin_of("https://example.com/a/b?token=x"),
            "https://example.com"
        );
        assert_eq!(
            origin_of("http://127.0.0.1:8080/x"),
            "http://127.0.0.1:8080"
        );
        assert_eq!(origin_of("about:blank"), "about:");
    }

    #[test]
    fn secret_fields_are_recognised() {
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(secret_field("INPUT", &a(&["type", "Password"])));
        assert!(secret_field(
            "input",
            &a(&["autocomplete", "one-time-code"])
        ));
        assert!(secret_field("input", &a(&["autocomplete", "new-password"])));
        assert!(secret_field(
            "input",
            &a(&["autocomplete", "billing cc-number"])
        ));
        assert!(!secret_field(
            "input",
            &a(&["type", "text", "name", "password_hint"])
        ));
        assert!(!secret_field("textarea", &a(&["type", "password"])));
    }
}
