//! The `pre-git` guard (ADR 0011, "The guard"): what a Bash command Claude
//! Code is about to run may not do in a folder with an automatic commit
//! mode. Pure: the command text, the folder's commit settings and its git
//! state in, a one-line deny reason out (`None` lets Claude Code's normal
//! flow decide). It never allows anything, so it can only make Claude Code
//! stricter.
//!
//! The command is split as a shell would split it, far enough for this:
//! `;`, `&&`, `||`, `|`, `&`, newlines and parentheses separate commands;
//! quotes, escapes, comments and redirections are honoured; heredoc bodies
//! are skipped (a commit message may say anything); commands inside `$()`
//! and backticks are checked too.

use crate::agent::git::GitInfo;
use crate::claude_settings::{CommitMode, CommitSettings};

/// The hook's answer for a deny: Claude Code blocks the call and shows the
/// model the reason.
pub fn deny_json(reason: &str) -> String {
    serde_json::json!({"hookSpecificOutput": {
        "hookEventName": "PreToolUse",
        "permissionDecision": "deny",
        "permissionDecisionReason": reason
    }})
    .to_string()
}

/// The deny reason, or `None`.
pub fn check(command: &str, folder: &CommitSettings, git: &GitInfo) -> Option<String> {
    if !folder.mode.automatic() {
        return None;
    }
    commands(command)
        .iter()
        .filter_map(|words| git_call(words))
        .find_map(|(sub, args, skips_hooks)| match sub {
            "commit" => check_commit(args, skips_hooks, folder, git),
            "push" => check_push(args, folder, git),
            _ => None,
        })
}

const NO_VERIFY: &str =
    "Acta: commits here run the git hooks (no --no-verify or -n). Fix what the hook reports.";
const FORCED: &str = "Acta: forced, deleting or mirror pushes are refused here. Ask the user.";
const EXACT_PUSH: &str = "Acta: run exactly `git push`, with no arguments.";
const DETACHED: &str = "Acta: HEAD is detached. Ask the user before committing or pushing.";

fn on_default(branch: &str) -> String {
    let b: String = branch.chars().take(30).collect();
    format!("Acta: no automatic commit or push on {b} here. Ask the user.")
}

/// The branch rule: detached, or the default branch without the opt-in.
fn branch_rule(folder: &CommitSettings, git: &GitInfo) -> Option<String> {
    match &git.branch {
        None => Some(DETACHED.to_owned()),
        Some(b) if git.on_default_branch() && !folder.on_default => Some(on_default(b)),
        Some(_) => None,
    }
}

/// Options of `git commit` that take the next word as their value.
const COMMIT_LONG_VALUE: &[&str] = &[
    "--message",
    "--file",
    "--reuse-message",
    "--reedit-message",
    "--template",
    "--author",
    "--date",
    "--cleanup",
    "--fixup",
    "--squash",
    "--trailer",
    "--pathspec-from-file",
];

fn check_commit(
    args: &[String],
    skips_hooks: bool,
    folder: &CommitSettings,
    git: &GitInfo,
) -> Option<String> {
    let mut no_verify = skips_hooks;
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        i += 1;
        if a == "--" {
            break;
        }
        if let Some(long) = a.strip_prefix("--") {
            let name = long.split('=').next().unwrap_or("");
            // git takes any unambiguous prefix of a long option.
            if name.len() >= 5 && "no-verify".starts_with(name) {
                no_verify = true;
            }
            if !a.contains('=') && COMMIT_LONG_VALUE.contains(&a) {
                i += 1;
            }
        } else if let Some(cluster) = a.strip_prefix('-')
            && !cluster.is_empty()
        {
            for (k, c) in cluster.char_indices() {
                match c {
                    'n' => no_verify = true,
                    // Value taken from the rest, or the next word.
                    'm' | 'F' | 'C' | 'c' | 't' => {
                        if k + 1 == cluster.len() {
                            i += 1;
                        }
                        break;
                    }
                    // Optional value, attached only.
                    'u' | 'S' => break,
                    _ => {}
                }
            }
        }
    }
    if no_verify {
        return Some(NO_VERIFY.to_owned());
    }
    branch_rule(folder, git)
}

fn check_push(args: &[String], folder: &CommitSettings, git: &GitInfo) -> Option<String> {
    let mut forced = false;
    let mut positional = 0;
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        i += 1;
        if let Some(long) = a.strip_prefix("--") {
            let name = long.split('=').next().unwrap_or("");
            if name.starts_with("force")
                || (name.len() >= 3 && "delete".starts_with(name))
                || (name.len() >= 2 && "mirror".starts_with(name))
            {
                forced = true;
            }
            if !a.contains('=')
                && ["--repo", "--receive-pack", "--exec", "--push-option"].contains(&a)
            {
                i += 1;
            }
        } else if let Some(cluster) = a.strip_prefix('-')
            && !cluster.is_empty()
        {
            for (k, c) in cluster.char_indices() {
                match c {
                    'f' | 'd' => forced = true,
                    'o' => {
                        if k + 1 == cluster.len() {
                            i += 1;
                        }
                        break;
                    }
                    _ => {}
                }
            }
        } else {
            positional += 1;
            // `+src:dst` forces, `:dst` deletes (the repository is the first).
            if positional > 1 && (a.starts_with('+') || a.starts_with(':')) {
                forced = true;
            }
        }
    }
    if forced {
        return Some(FORCED.to_owned());
    }
    if folder.mode != CommitMode::Push {
        // Commit automatically: a push is the user's call, asked as usual.
        return None;
    }
    if !args.is_empty() {
        return Some(EXACT_PUSH.to_owned());
    }
    branch_rule(folder, git)
}

/// A `git` call: its subcommand, the arguments after it, and whether a
/// `-c core.hooksPath=...` before it turns the hooks off.
fn git_call(words: &[String]) -> Option<(&str, &[String], bool)> {
    let mut i = 0;
    // Leading variable assignments: `GIT_X=1 git commit`.
    while i < words.len() && is_assignment(&words[i]) {
        i += 1;
    }
    let exe = words.get(i)?;
    let base = exe.rsplit(['/', '\\']).next().unwrap_or(exe);
    if !base.eq_ignore_ascii_case("git") && !base.eq_ignore_ascii_case("git.exe") {
        return None;
    }
    i += 1;
    let mut skips_hooks = false;
    while let Some(w) = words.get(i) {
        if !w.starts_with('-') {
            break;
        }
        i += 1;
        let value = match w.as_str() {
            "-C" | "-c" | "--git-dir" | "--work-tree" | "--namespace" | "--config-env" => {
                i += 1;
                words.get(i - 1).map(String::as_str)
            }
            _ => w.split_once('=').map(|(_, v)| v),
        };
        if (w == "-c" || w.starts_with("--config-env"))
            && value.is_some_and(|v| v.to_ascii_lowercase().starts_with("core.hookspath"))
        {
            skips_hooks = true;
        }
    }
    let sub = words.get(i)?;
    Some((sub.as_str(), &words[i + 1..], skips_hooks))
}

fn is_assignment(w: &str) -> bool {
    w.split_once('=').is_some_and(|(name, _)| {
        !name.is_empty()
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && !name.starts_with(|c: char| c.is_ascii_digit())
    })
}

/// Every simple command in `src`, top level and inside substitutions, as
/// its words with quotes removed.
pub fn commands(src: &str) -> Vec<Vec<String>> {
    let chars: Vec<char> = src.chars().collect();
    let mut p = Parser {
        s: &chars,
        i: 0,
        out: Vec::new(),
    };
    p.parse(false);
    p.out
}

/// The command being read: its words and the word in progress.
#[derive(Default)]
struct Acc {
    words: Vec<String>,
    cur: String,
    in_word: bool,
}

impl Acc {
    fn end_word(&mut self) {
        if self.in_word {
            self.words.push(std::mem::take(&mut self.cur));
            self.in_word = false;
        }
    }

    fn end_command(&mut self, out: &mut Vec<Vec<String>>) {
        self.end_word();
        if !self.words.is_empty() {
            out.push(std::mem::take(&mut self.words));
        }
    }
}

struct Parser<'a> {
    s: &'a [char],
    i: usize,
    out: Vec<Vec<String>>,
}

impl Parser<'_> {
    fn peek(&self, k: usize) -> Option<char> {
        self.s.get(self.i + k).copied()
    }

    /// Parses commands until the end, or a closing `)` when `nested`.
    fn parse(&mut self, nested: bool) {
        let mut a = Acc::default();
        let mut heredocs: Vec<(String, bool)> = Vec::new();
        let mut depth = 0usize;
        while let Some(c) = self.peek(0) {
            match c {
                ' ' | '\t' | '\r' => {
                    self.i += 1;
                    a.end_word();
                }
                '\n' => {
                    self.i += 1;
                    a.end_command(&mut self.out);
                    for (delim, strip) in std::mem::take(&mut heredocs) {
                        self.skip_heredoc(&delim, strip);
                    }
                }
                ';' | '|' => {
                    self.i += 1;
                    a.end_command(&mut self.out);
                }
                '&' => {
                    if self.peek(1) == Some('>') {
                        self.i += 1;
                        a.end_word();
                        self.redirection();
                    } else {
                        self.i += 1;
                        a.end_command(&mut self.out);
                    }
                }
                '(' => {
                    self.i += 1;
                    depth += 1;
                    a.end_command(&mut self.out);
                }
                ')' => {
                    self.i += 1;
                    if depth == 0 && nested {
                        a.end_command(&mut self.out);
                        return;
                    }
                    depth = depth.saturating_sub(1);
                    a.end_command(&mut self.out);
                }
                '#' if !a.in_word => {
                    while self.peek(0).is_some_and(|c| c != '\n') {
                        self.i += 1;
                    }
                }
                '\\' => {
                    self.i += 1;
                    match self.peek(0) {
                        Some('\n') => self.i += 1,
                        Some(n) => {
                            a.cur.push(n);
                            a.in_word = true;
                            self.i += 1;
                        }
                        None => {}
                    }
                }
                '\'' => {
                    self.i += 1;
                    a.in_word = true;
                    while let Some(n) = self.peek(0) {
                        self.i += 1;
                        if n == '\'' {
                            break;
                        }
                        a.cur.push(n);
                    }
                }
                '"' => {
                    self.i += 1;
                    a.in_word = true;
                    self.double_quoted(&mut a.cur);
                }
                '`' => {
                    self.i += 1;
                    a.in_word = true;
                    self.backticks();
                    a.cur.push_str("$()");
                }
                '$' if self.peek(1) == Some('(') => {
                    self.i += 2;
                    a.in_word = true;
                    self.parse(true);
                    a.cur.push_str("$()");
                }
                '<' if self.peek(1) == Some('<') && self.peek(2) != Some('<') => {
                    self.i += 2;
                    a.end_word();
                    let strip = self.peek(0) == Some('-');
                    if strip {
                        self.i += 1;
                    }
                    while self.peek(0).is_some_and(|c| c == ' ' || c == '\t') {
                        self.i += 1;
                    }
                    heredocs.push((self.word(), strip));
                }
                '>' | '<' => {
                    if a.cur.chars().all(|c| c.is_ascii_digit()) && a.in_word {
                        a.cur.clear();
                        a.in_word = false;
                    }
                    a.end_word();
                    self.redirection();
                }
                _ => {
                    a.cur.push(c);
                    a.in_word = true;
                    self.i += 1;
                }
            }
        }
        a.end_command(&mut self.out);
    }

    /// Skips a redirection operator and its target.
    fn redirection(&mut self) {
        self.i += 1; // '>', '<' or '&'
        while self
            .peek(0)
            .is_some_and(|c| c == '>' || c == '&' || c == '<')
        {
            self.i += 1;
        }
        while self.peek(0).is_some_and(|c| c == ' ' || c == '\t') {
            self.i += 1;
        }
        let _ = self.word();
    }

    /// A plain word (a heredoc delimiter, a redirection target), quotes
    /// removed.
    fn word(&mut self) -> String {
        let mut w = String::new();
        while let Some(c) = self.peek(0) {
            match c {
                ' ' | '\t' | '\n' | ';' | '&' | '|' | '(' | ')' | '<' | '>' => break,
                '\'' | '"' => {
                    self.i += 1;
                    while let Some(n) = self.peek(0) {
                        self.i += 1;
                        if n == c {
                            break;
                        }
                        w.push(n);
                    }
                }
                '\\' => {
                    self.i += 1;
                    if let Some(n) = self.peek(0) {
                        w.push(n);
                        self.i += 1;
                    }
                }
                _ => {
                    w.push(c);
                    self.i += 1;
                }
            }
        }
        w
    }

    /// Skips heredoc lines up to the delimiter line.
    fn skip_heredoc(&mut self, delim: &str, strip_tabs: bool) {
        while self.i < self.s.len() {
            let start = self.i;
            while self.peek(0).is_some_and(|c| c != '\n') {
                self.i += 1;
            }
            let line: String = self.s[start..self.i].iter().collect();
            if self.peek(0) == Some('\n') {
                self.i += 1;
            }
            let line = line.trim_end_matches('\r');
            let line = if strip_tabs {
                line.trim_start_matches('\t')
            } else {
                line
            };
            if line == delim {
                return;
            }
        }
    }

    /// Inside double quotes: escapes and substitutions; the text goes to
    /// `cur`.
    fn double_quoted(&mut self, cur: &mut String) {
        while let Some(c) = self.peek(0) {
            self.i += 1;
            match c {
                '"' => return,
                '\\' => {
                    if let Some(n) = self.peek(0) {
                        cur.push(n);
                        self.i += 1;
                    }
                }
                '$' if self.peek(0) == Some('(') => {
                    self.i += 1;
                    self.parse(true);
                    cur.push_str("$()");
                }
                '`' => {
                    self.backticks();
                    cur.push_str("$()");
                }
                _ => cur.push(c),
            }
        }
    }

    /// The commands between backticks (no nesting).
    fn backticks(&mut self) {
        let start = self.i;
        while self.peek(0).is_some_and(|c| c != '`') {
            self.i += 1;
        }
        let inner: String = self.s[start..self.i].iter().collect();
        self.i += 1;
        self.out.extend(commands(&inner));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn git(branch: Option<&str>) -> GitInfo {
        GitInfo {
            root: PathBuf::from("r"),
            main_root: Some(PathBuf::from("r")),
            branch: branch.map(str::to_owned),
            common_dir: PathBuf::from("r/.git"),
            default_branch: Some("main".into()),
        }
    }

    fn folder(mode: CommitMode, on_default: bool) -> CommitSettings {
        CommitSettings {
            mode,
            as_user: false,
            on_default,
        }
    }

    const MANUAL: CommitMode = CommitMode::Manual;
    const COMMIT: CommitMode = CommitMode::Commit;
    const PUSH: CommitMode = CommitMode::Push;
    const OK: Option<&str> = None;

    /// (command, mode, on default branch opt-in, branch, expected reason start)
    type Row = (
        &'static str,
        CommitMode,
        bool,
        Option<&'static str>,
        Option<&'static str>,
    );
    const TABLE: &[Row] = &[
        // Manual: never anything.
        (
            "git commit --no-verify -m x",
            MANUAL,
            false,
            Some("main"),
            OK,
        ),
        ("git push --force", MANUAL, false, Some("main"), OK),
        // Hooks skipped.
        (
            "git commit --no-verify -m x",
            COMMIT,
            false,
            Some("t"),
            Some(NO_VERIFY),
        ),
        (
            "git commit -n -m x",
            COMMIT,
            false,
            Some("t"),
            Some(NO_VERIFY),
        ),
        (
            "git commit -anm x",
            COMMIT,
            false,
            Some("t"),
            Some(NO_VERIFY),
        ),
        (
            "git commit --no-veri -m x",
            PUSH,
            false,
            Some("t"),
            Some(NO_VERIFY),
        ),
        (
            "git -c core.hooksPath=/dev/null commit -m x",
            COMMIT,
            false,
            Some("t"),
            Some(NO_VERIFY),
        ),
        // A value is not an option.
        ("git commit -m -n", COMMIT, false, Some("t"), OK),
        (
            "git commit -m \"Explain --no-verify\"",
            COMMIT,
            false,
            Some("t"),
            OK,
        ),
        ("git commit -am 'x -n'", COMMIT, false, Some("t"), OK),
        (
            "git commit --author \"Alexandre -n\" -m x",
            COMMIT,
            false,
            Some("t"),
            OK,
        ),
        (
            "git add src/a.rs && git commit -m x",
            COMMIT,
            false,
            Some("t"),
            OK,
        ),
        // Default branch, with and without opt-in; detached head.
        (
            "git commit -m x",
            COMMIT,
            false,
            Some("main"),
            Some("Acta: no automatic commit or push on main"),
        ),
        ("git commit -m x", COMMIT, true, Some("main"), OK),
        ("git commit -m x", COMMIT, false, None, Some(DETACHED)),
        (
            "git push",
            PUSH,
            false,
            Some("main"),
            Some("Acta: no automatic commit or push on main"),
        ),
        ("git push", PUSH, true, Some("main"), OK),
        ("git push", PUSH, false, None, Some(DETACHED)),
        // Forced, rewriting, deleting.
        ("git push -f", COMMIT, false, Some("t"), Some(FORCED)),
        (
            "git push -uf origin t",
            PUSH,
            false,
            Some("t"),
            Some(FORCED),
        ),
        (
            "git push --force-with-lease",
            COMMIT,
            false,
            Some("t"),
            Some(FORCED),
        ),
        (
            "git push --force-if-includes",
            PUSH,
            false,
            Some("t"),
            Some(FORCED),
        ),
        (
            "git push origin +main",
            COMMIT,
            false,
            Some("t"),
            Some(FORCED),
        ),
        (
            "git push origin :old",
            COMMIT,
            false,
            Some("t"),
            Some(FORCED),
        ),
        (
            "git push --delete origin old",
            COMMIT,
            false,
            Some("t"),
            Some(FORCED),
        ),
        (
            "git push -d origin old",
            COMMIT,
            false,
            Some("t"),
            Some(FORCED),
        ),
        ("git push --mirror", COMMIT, false, Some("t"), Some(FORCED)),
        // Arguments: refused in push mode, the user's call in commit mode.
        (
            "git push origin x",
            PUSH,
            false,
            Some("t"),
            Some(EXACT_PUSH),
        ),
        (
            "git push -u origin t",
            PUSH,
            false,
            Some("t"),
            Some(EXACT_PUSH),
        ),
        ("git push origin x", COMMIT, false, Some("t"), OK),
        // Plain push in each mode.
        ("git push", PUSH, false, Some("t"), OK),
        ("git push", COMMIT, false, Some("t"), OK),
        ("git push", COMMIT, false, Some("main"), OK),
        // Not git, or another subcommand.
        ("echo git push --force", PUSH, false, Some("t"), OK),
        ("git status && git log -1", PUSH, false, Some("main"), OK),
    ];

    #[test]
    fn the_table() {
        for (cmd, mode, opt_in, branch, want) in TABLE {
            let got = check(cmd, &folder(*mode, *opt_in), &git(*branch));
            match want {
                None => assert_eq!(got, None, "{cmd} {mode:?}"),
                Some(w) => assert!(
                    got.as_deref().is_some_and(|g| g.starts_with(w)),
                    "{cmd} {mode:?}: {got:?}"
                ),
            }
        }
    }

    #[test]
    fn heredoc_messages_and_compound_commands() {
        let f = folder(COMMIT, false);
        let g = git(Some("t"));
        // Claude Code's own commit form: the body may say anything.
        let heredoc = "git add a.rs && git commit -m \"$(cat <<'EOF'\nSkip --no-verify (part \"1\"\n\nWhy: -n is bad ) here\nEOF\n)\"";
        assert_eq!(check(heredoc, &f, &g), None);
        let top = "git commit -F - <<EOF\n--no-verify\nEOF\ngit status";
        assert_eq!(check(top, &f, &g), None);
        // ... but the commands around and inside are checked.
        let after =
            "git commit -m \"$(cat <<'EOF'\nmsg\nEOF\n)\" && git commit --amend --no-verify";
        assert_eq!(check(after, &f, &g).as_deref(), Some(NO_VERIFY));
        assert_eq!(
            check("echo $(git push -f)", &f, &g).as_deref(),
            Some(FORCED)
        );
        assert_eq!(check("echo `git push -f`", &f, &g).as_deref(), Some(FORCED));
        assert_eq!(
            check("(cd x; git push --force)", &f, &g).as_deref(),
            Some(FORCED)
        );
        assert_eq!(
            check("GIT_TRACE=1 git commit -n", &f, &g).as_deref(),
            Some(NO_VERIFY)
        );
        assert_eq!(
            check("git -C sub commit --no-verify", &f, &g).as_deref(),
            Some(NO_VERIFY)
        );
        assert_eq!(check("git commit -m x 2>&1 | tail -3", &f, &g), None);
        assert_eq!(check("git commit -m x # --no-verify", &f, &g), None);
        assert_eq!(
            check("/usr/bin/git push --force", &f, &g).as_deref(),
            Some(FORCED)
        );
    }

    #[test]
    fn reasons_fit_the_hook_budget() {
        let long = on_default(&"b".repeat(300));
        for r in [NO_VERIFY, FORCED, EXACT_PUSH, DETACHED, long.as_str()] {
            let json = deny_json(r);
            assert!(
                json.chars().count() <= 200,
                "{} {json}",
                json.chars().count()
            );
        }
    }

    #[test]
    fn splitting() {
        let c = commands("A=1 git commit -m 'a b' ; echo \"x $(git push) y\"");
        assert_eq!(
            c,
            vec![
                vec![
                    "A=1".to_owned(),
                    "git".into(),
                    "commit".into(),
                    "-m".into(),
                    "a b".into()
                ],
                vec!["git".into(), "push".into()],
                vec!["echo".into(), "x $() y".into()],
            ]
        );
    }
}
