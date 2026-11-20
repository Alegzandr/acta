//! Navigation allow-list for the chat panel: the only non-UI logic in this spike.
//! The page may stay in the panel only on the hosts below; everything else is handed
//! to the system browser. Which sign-in provider hosts are really needed can only be
//! confirmed by a real sign-in, which this spike deliberately does not perform.

use url::Url;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Decision {
    /// Load inside the panel.
    InPanel,
    /// Refuse in the panel and open in the system browser.
    SystemBrowser,
    /// Refuse and do nothing (non-web schemes, unparsable input).
    Block,
}

/// Hosts allowed in the panel: exact match or a subdomain of an entry.
/// Only claude.ai was seen in the spike (`/` redirects to `/login`); help and marketing
/// pages on claude.com and anthropic.com belong in the system browser.
const PANEL_HOSTS: &[&str] = &["claude.ai"];
/// Sign-in providers: exact hosts only.
const SIGN_IN_HOSTS: &[&str] = &["accounts.google.com", "appleid.apple.com"];

pub fn classify(raw: &str, extra_host: Option<&str>) -> Decision {
    // Chromium reports these for the initial empty document and for blob/data frames.
    if raw == "about:blank" || raw.starts_with("blob:https://claude.ai/") {
        return Decision::InPanel;
    }
    let Ok(url) = Url::parse(raw) else {
        return Decision::Block;
    };
    let Some(host) = url.host_str() else {
        return Decision::Block;
    };
    if extra_host == Some(host) && matches!(url.scheme(), "http" | "https") {
        return Decision::InPanel;
    }
    match url.scheme() {
        "https" => {}
        "http" => return Decision::SystemBrowser,
        _ => return Decision::Block,
    }
    let in_panel = PANEL_HOSTS
        .iter()
        .any(|allowed| host == *allowed || host.ends_with(&format!(".{allowed}")))
        || SIGN_IN_HOSTS.contains(&host);
    if in_panel {
        Decision::InPanel
    } else {
        Decision::SystemBrowser
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_and_subdomains_stay_in_the_panel() {
        assert_eq!(classify("https://claude.ai/new", None), Decision::InPanel);
        assert_eq!(
            classify("https://api.claude.ai/x?y=1", None),
            Decision::InPanel
        );
        assert_eq!(classify("about:blank", None), Decision::InPanel);
    }

    #[test]
    fn lookalike_hosts_leave_the_panel() {
        assert_eq!(
            classify("https://claude.ai.example.com/login", None),
            Decision::SystemBrowser
        );
        assert_eq!(
            classify("https://notclaude.ai/", None),
            Decision::SystemBrowser
        );
        assert_eq!(
            classify("https://user@claude.ai@example.com/", None),
            Decision::SystemBrowser
        );
    }

    #[test]
    fn plain_http_and_other_sites_go_to_the_system_browser() {
        assert_eq!(classify("http://claude.ai/", None), Decision::SystemBrowser);
        assert_eq!(
            classify("https://github.com/Alexandre/project", None),
            Decision::SystemBrowser
        );
    }

    #[test]
    fn non_web_schemes_are_blocked() {
        assert_eq!(
            classify("file:///C:/Users/Alexandre/x", None),
            Decision::Block
        );
        assert_eq!(classify("javascript:alert(1)", None), Decision::Block);
        assert_eq!(classify("not a url", None), Decision::Block);
    }

    #[test]
    fn sign_in_providers_match_exactly() {
        assert_eq!(
            classify("https://accounts.google.com/o/oauth2/v2/auth", None),
            Decision::InPanel
        );
        assert_eq!(
            classify("https://evil.accounts.google.com/", None),
            Decision::SystemBrowser
        );
    }

    #[test]
    fn the_spike_test_origin_is_allowed_only_when_named() {
        let url = "http://127.0.0.1:8777/persist.html";
        assert_eq!(classify(url, Some("127.0.0.1")), Decision::InPanel);
        assert_eq!(classify(url, None), Decision::SystemBrowser);
    }
}
