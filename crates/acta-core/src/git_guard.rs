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
use std::path::{Component, Path, PathBuf};

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

/// What the guard knows about a directory a git call acts in.
#[derive(Clone, Debug)]
pub enum Place {
    /// Not inside any attached folder.
    Unlinked,
    /// Inside an attached folder: its display name, commit settings and git
    /// state (`None` when git's files cannot be read there).
    Folder {
        name: String,
        settings: CommitSettings,
        git: Option<Box<GitInfo>>,
    },
    /// Attached, but Acta cannot read its settings (store or keychain).
    Unreadable,
}

/// A git alias looked up in the repository's config.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Alias {
    None,
    Value(String),
    /// git could not answer in time: the call cannot be read.
    Failed,
}

/// The guard's view of the session.
pub struct Context<'a> {
    /// The hook's working directory: relative `cd` and `-C` start here.
    pub cwd: &'a Path,
    /// Where the session started (its settings file holds the allow rules).
    pub session: &'a Path,
    pub place: &'a dyn Fn(&Path) -> Place,
    pub alias: &'a dyn Fn(&Path, &str) -> Alias,
}

/// The deny reason, or `None` (Claude Code's normal flow decides).
///
/// Each commit or push is judged in the folder it acts in: the hook's
/// directory, then every `cd`/`pushd` before it in the command (all of
/// them, since a subshell's `cd` may or may not apply), then `-C`,
/// `--work-tree`, `--git-dir`, `env -C`. A directory that cannot be worked
/// out is refused when the session folder is automatic.
pub fn check(command: &str, cx: &Context) -> Option<String> {
    let mut a = Analysis::default();
    analyze(command, 0, &mut a);
    let session = (cx.place)(cx.session);
    let session_auto =
        matches!(&session, Place::Folder { settings, .. } if settings.mode.automatic());
    let unreadable = matches!(session, Place::Unreadable);
    let mentions = mentions_commit_or_push(command);
    if !session_auto
        && !unreadable
        && !a
            .calls
            .iter()
            .any(|c| c.sub == "commit" || c.sub == "push" || !is_own(&c.sub))
    {
        return None;
    }
    // An alias is looked up in the repository the call acts in: the last
    // directory that can be worked out, else the hook's.
    resolve_aliases(&mut a, &|c: &Call, name: &str| {
        let dir = c
            .candidates
            .iter()
            .rev()
            .flatten()
            .next()
            .map_or_else(|| cx.cwd.to_path_buf(), |steps| walk(cx.cwd, steps));
        (cx.alias)(&dir, name)
    });
    let mut any_auto = session_auto;
    for c in a
        .calls
        .iter()
        .filter(|c| c.sub == "commit" || c.sub == "push")
    {
        for cand in &c.candidates {
            let Some(steps) = cand else {
                if session_auto || unreadable {
                    return Some(DIR.to_owned());
                }
                continue;
            };
            let dir = walk(cx.cwd, steps);
            match (cx.place)(&dir) {
                Place::Unreadable => return Some(FAIL.to_owned()),
                Place::Unlinked => {
                    if unreadable {
                        return Some(FAIL.to_owned());
                    }
                    if let Place::Folder { name, .. } = &session
                        && session_auto
                    {
                        return Some(outside(name));
                    }
                }
                Place::Folder {
                    name,
                    settings,
                    git,
                } => {
                    if !settings.mode.automatic() {
                        if session_auto {
                            return Some(manual(&name));
                        }
                        continue;
                    }
                    any_auto = true;
                    if c.config
                        || a.env
                            .iter()
                            .chain(&c.env)
                            .any(|n| n.to_ascii_uppercase().starts_with("GIT_CONFIG"))
                    {
                        return Some(CONFIG.to_owned());
                    }
                    if a.env.iter().chain(&c.env).any(|n| !env_allowed(n)) {
                        return Some(ENV.to_owned());
                    }
                    if c.open_args {
                        return Some(DIRECT.to_owned());
                    }
                    let Some(git) = git else {
                        return Some(UNKNOWN.to_owned());
                    };
                    let reason = if c.sub == "commit" {
                        check_commit(&c.args, &dir, &settings, &git)
                    } else {
                        check_push(&c.args, &settings, &git)
                    };
                    if reason.is_some() {
                        return reason;
                    }
                }
            }
        }
    }
    if a.alias_failed && (session_auto || unreadable) {
        return Some(DIRECT.to_owned());
    }
    if session_auto {
        for c in &a.calls {
            if c.sub.contains('$') && mentions {
                return Some(DIRECT.to_owned());
            }
            if c.config && !is_own(&c.sub) {
                return Some(CONFIG.to_owned());
            }
        }
    }
    if a.opaque && mentions {
        if unreadable {
            return Some(FAIL.to_owned());
        }
        if any_auto {
            return Some(DIRECT.to_owned());
        }
    }
    None
}

/// The hook's answer when the folder is linked but Acta cannot read it.
pub fn fail_closed() -> String {
    deny_json(FAIL)
}

/// Whether the command may need the guard at all: a git commit or push, a
/// subcommand that is not one of git's own (an alias), or something that
/// could not be read. `false` lets the hook return before opening anything.
pub fn needs_check(command: &str) -> bool {
    let mut a = Analysis::default();
    analyze(command, 0, &mut a);
    a.calls
        .iter()
        .any(|c| c.sub == "commit" || c.sub == "push" || !is_own(&c.sub))
        || (a.opaque && mentions_commit_or_push(command))
}

/// Variables a commit or push may carry: locale, time zone, terminal.
fn env_allowed(name: &str) -> bool {
    matches!(name, "LANG" | "TZ" | "TERM") || name.starts_with("LC_")
}

/// The directory a list of `cd` steps leads to from `base`, worked out on
/// the text (`.` and `..` folded). On Windows, Git Bash's `/c/...` is `C:\...`.
fn walk(base: &Path, steps: &[String]) -> PathBuf {
    let mut p = base.to_path_buf();
    for s in steps {
        let s = msys(s);
        let step = Path::new(&s);
        if step.has_root() {
            p = PathBuf::new();
        }
        for c in step.components() {
            match c {
                Component::CurDir => {}
                Component::ParentDir => {
                    p.pop();
                }
                other => p.push(other.as_os_str()),
            }
        }
    }
    p
}

fn msys(s: &str) -> String {
    if cfg!(windows) {
        let b = s.as_bytes();
        if b.len() >= 2
            && b[0] == b'/'
            && b[1].is_ascii_alphabetic()
            && (b.len() == 2 || b[2] == b'/')
        {
            return format!(
                "{}:\\{}",
                (b[1] as char).to_ascii_uppercase(),
                s.get(3..).unwrap_or("")
            );
        }
    }
    s.to_owned()
}

const NO_VERIFY: &str =
    "Acta: commits here run the git hooks (no --no-verify or -n). Fix what the hook reports.";
const FORCED: &str =
    "Acta: forced, deleting, pruning or mirror pushes are refused here. Ask the user.";
const EXACT_PUSH: &str = "Acta: run exactly `git push`, with no arguments.";
const DETACHED: &str = "Acta: HEAD is detached. Ask the user before committing or pushing.";
const UNKNOWN: &str =
    "Acta: cannot read this repository's branches. Ask the user before committing.";
const CONFIG: &str =
    "Acta: no -c, --config-env or GIT_CONFIG_* on a commit or push here. Run git plainly.";
const DIRECT: &str = "Acta: run git commit or git push directly here, not through another command.";
const ENV: &str =
    "Acta: no environment variables on a commit or push here (LANG, LC_*, TZ, TERM only).";
const DIR: &str = "Acta: run git in this folder (no cd to a variable, ~ or -, no --git-dir).";
const FAIL: &str = "Acta cannot read this folder's commit mode. Ask the user before committing.";
const READS: &str =
    "Acta: -F, -t and pathspec files must be in this repository; -C/-c take a commit.";

fn short(name: &str) -> String {
    name.chars().take(30).collect()
}

fn on_default(branch: &str) -> String {
    format!(
        "Acta: no automatic commit or push on {} here. Ask the user.",
        short(branch)
    )
}

fn manual(name: &str) -> String {
    format!("Acta: commits in {} are manual. Ask the user.", short(name))
}

fn outside(name: &str) -> String {
    format!(
        "Acta: automatic commits only in {}. Ask the user.",
        short(name)
    )
}

/// The branch rule: HEAD or the default branch unknown, detached, or the
/// default branch without the opt-in.
fn branch_rule(folder: &CommitSettings, git: &GitInfo) -> Option<String> {
    if !git.head_known || git.default_branch.is_none() {
        return Some(UNKNOWN.to_owned());
    }
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

/// What a value of `git commit` is read as.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ValueKind {
    /// Read from a file (`-F`, `--pathspec-from-file`); `-` is stdin.
    File,
    /// A message template file (`-t`): no stdin.
    Template,
    /// A commit whose message is reused (`-C`, `-c`).
    Commit,
}

/// Long options whose value names a file or a commit, and how it is read.
const COMMIT_READ_VALUE: &[(&str, ValueKind)] = &[
    ("file", ValueKind::File),
    ("pathspec-from-file", ValueKind::File),
    ("template", ValueKind::Template),
    ("reuse-message", ValueKind::Commit),
    ("reedit-message", ValueKind::Commit),
];

/// The read option a long option name stands for. git takes any
/// unambiguous prefix; an ambiguous one is an error in git, so taking it
/// for the read option only refuses what would fail anyway.
fn read_option(name: &str) -> Option<ValueKind> {
    if name.is_empty() {
        return None;
    }
    COMMIT_READ_VALUE
        .iter()
        .find(|(o, _)| o.starts_with(name))
        .map(|&(_, k)| k)
}

/// Whether a value of a read option stays in the repository: a file inside
/// the worktree (`-` for stdin where git allows it), or a commit name that
/// is not a path. A link inside the worktree is followed when it exists.
fn value_allowed(kind: ValueKind, value: &str, dir: &Path, root: &Path) -> bool {
    let shellish = value.starts_with('~') || value.contains(['$', '`']);
    let path = walk(dir, &[value.to_owned()]);
    match kind {
        ValueKind::File if value == "-" => true,
        ValueKind::File | ValueKind::Template => {
            !shellish && !value.is_empty() && inside(&path, root)
        }
        ValueKind::Commit => {
            !shellish && !value.is_empty() && !value.contains(['/', '\\', ':']) && !path.exists()
        }
    }
}

/// `path` is inside `root`: on the text, and also once links are resolved
/// when both exist.
fn inside(path: &Path, root: &Path) -> bool {
    if !path.starts_with(root) {
        return false;
    }
    match (std::fs::canonicalize(path), std::fs::canonicalize(root)) {
        (Ok(p), Ok(r)) => p.starts_with(r),
        _ => true,
    }
}

/// `dir` is where the commit runs: relative files are read from there.
fn check_commit(
    args: &[String],
    dir: &Path,
    folder: &CommitSettings,
    git: &GitInfo,
) -> Option<String> {
    let mut no_verify = false;
    let mut reads = false;
    let mut check_value = |kind: ValueKind, value: Option<&String>| {
        // A missing value is an error in git.
        if let Some(v) = value
            && !value_allowed(kind, v, dir, &git.root)
        {
            reads = true;
        }
    };
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        i += 1;
        if a == "--" {
            break;
        }
        if let Some(long) = a.strip_prefix("--") {
            let (name, attached) = match long.split_once('=') {
                Some((n, v)) => (n, Some(v.to_owned())),
                None => (long, None),
            };
            // git takes any unambiguous prefix of a long option.
            if name.len() >= 5 && "no-verify".starts_with(name) {
                no_verify = true;
            }
            if let Some(kind) = read_option(name) {
                match attached {
                    Some(v) => check_value(kind, Some(&v)),
                    None => {
                        check_value(kind, args.get(i));
                        i += 1;
                    }
                }
            } else if attached.is_none() && COMMIT_LONG_VALUE.contains(&a) {
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
                        let rest = &cluster[k + 1..];
                        let value = if rest.is_empty() {
                            i += 1;
                            args.get(i - 1).cloned()
                        } else {
                            Some(rest.to_owned())
                        };
                        let kind = match c {
                            'F' => Some(ValueKind::File),
                            't' => Some(ValueKind::Template),
                            'C' | 'c' => Some(ValueKind::Commit),
                            _ => None,
                        };
                        if let Some(kind) = kind {
                            check_value(kind, value.as_ref());
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
    if reads {
        return Some(READS.to_owned());
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
                || (name.len() >= 3 && "prune".starts_with(name))
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
    if let Some(r) = branch_rule(folder, git) {
        return Some(r);
    }
    // A plain push goes to the upstream: not the default branch either.
    match (&git.upstream_branch, &git.default_branch) {
        (Some(u), Some(d)) if u == d && !folder.on_default => Some(on_default(d)),
        _ => None,
    }
}

/// A git call found in a command.
#[derive(Clone, Debug, Default)]
struct Call {
    sub: String,
    args: Vec<String>,
    /// `-c` or `--config-env` before the subcommand.
    config: bool,
    /// More arguments come from elsewhere (`xargs`).
    open_args: bool,
    /// Variables set for this call (prefix assignments, `env NAME=`).
    env: Vec<String>,
    /// The directories it may act in, as `cd` steps from the hook's
    /// directory; `None` for one that cannot be worked out.
    candidates: Vec<Option<Vec<String>>>,
}

#[derive(Debug, Default)]
struct Analysis {
    calls: Vec<Call>,
    /// Something that may run git could not be read.
    opaque: bool,
    /// Variables exported or assigned for the rest of the command.
    env: Vec<String>,
    /// The `cd` / `pushd` steps so far, in order; `None` when unknown.
    cds: Vec<Option<String>>,
    /// git could not say whether a subcommand is an alias.
    alias_failed: bool,
}

const MAX_DEPTH: usize = 6;

/// git's own subcommands other than commit and push: never aliases.
const BUILTIN: &[&str] = &[
    "add",
    "am",
    "annotate",
    "apply",
    "archive",
    "backfill",
    "bisect",
    "blame",
    "branch",
    "bugreport",
    "bundle",
    "cat-file",
    "check-attr",
    "check-ignore",
    "check-mailmap",
    "check-ref-format",
    "checkout",
    "checkout-index",
    "cherry",
    "cherry-pick",
    "citool",
    "clean",
    "clone",
    "column",
    "commit-graph",
    "commit-tree",
    "config",
    "count-objects",
    "credential",
    "describe",
    "diagnose",
    "diff",
    "diff-files",
    "diff-index",
    "diff-tree",
    "difftool",
    "fast-export",
    "fast-import",
    "fetch",
    "filter-branch",
    "fmt-merge-msg",
    "for-each-ref",
    "for-each-repo",
    "format-patch",
    "fsck",
    "gc",
    "get-tar-commit-id",
    "grep",
    "gui",
    "hash-object",
    "help",
    "hook",
    "imap-send",
    "index-pack",
    "init",
    "init-db",
    "instaweb",
    "interpret-trailers",
    "lfs",
    "log",
    "ls-files",
    "ls-remote",
    "ls-tree",
    "mailinfo",
    "mailsplit",
    "maintenance",
    "merge",
    "merge-base",
    "merge-file",
    "merge-tree",
    "mergetool",
    "mktag",
    "mktree",
    "multi-pack-index",
    "mv",
    "name-rev",
    "notes",
    "pack-objects",
    "pack-refs",
    "patch-id",
    "prune",
    "prune-packed",
    "pull",
    "range-diff",
    "read-tree",
    "rebase",
    "reflog",
    "refs",
    "remote",
    "repack",
    "replace",
    "request-pull",
    "rerere",
    "reset",
    "restore",
    "rev-list",
    "rev-parse",
    "revert",
    "rm",
    "scalar",
    "send-email",
    "shortlog",
    "show",
    "show-branch",
    "show-index",
    "show-ref",
    "sparse-checkout",
    "stage",
    "stash",
    "status",
    "stripspace",
    "submodule",
    "subtree",
    "svn",
    "switch",
    "symbolic-ref",
    "tag",
    "unpack-objects",
    "update-index",
    "update-ref",
    "update-server-info",
    "var",
    "verify-commit",
    "verify-pack",
    "verify-tag",
    "version",
    "whatchanged",
    "worktree",
    "write-tree",
];

/// Shell keywords a command may start with.
const KEYWORDS: &[&str] = &[
    "{", "}", "!", "if", "then", "else", "elif", "do", "while", "until", "time", "fi", "done",
];

/// Commands that print or search their arguments rather than run them.
const INERT: &[&str] = &[
    "echo", "printf", "grep", "egrep", "fgrep", "rg", "man", "cat", "which", "type", "whereis",
    "head", "tail", "wc", "less",
];

fn mentions_commit_or_push(text: &str) -> bool {
    let t = text.to_ascii_lowercase();
    t.contains("commit") || t.contains("push")
}

fn basename(w: &str) -> &str {
    w.rsplit(['/', '\\']).next().unwrap_or(w)
}

fn analyze(src: &str, depth: usize, out: &mut Analysis) {
    if depth > MAX_DEPTH {
        out.opaque = true;
        return;
    }
    let parsed = parse_all(src);
    if parsed.incomplete {
        out.opaque = true;
    }
    for words in &parsed.commands {
        simple(words, depth, out);
    }
}

/// Skips options: each word starting with `-`, the next one too for an
/// option in `with_value`; `--` ends them.
fn skip_opts<'a>(mut w: &'a [String], with_value: &[&str]) -> &'a [String] {
    while let Some(x) = w.first() {
        if x == "--" {
            return &w[1..];
        }
        if !x.starts_with('-') || x == "-" {
            break;
        }
        w = if with_value.contains(&x.as_str()) {
            w.get(2..).unwrap_or(&[])
        } else {
            &w[1..]
        };
    }
    w
}

fn assigned_name(w: &str) -> &str {
    w.split_once('=').map_or(w, |(n, _)| n)
}

/// A directory word `cd` can be told about: no variable, substitution,
/// home or previous directory.
fn plain_dir(w: &str) -> Option<String> {
    (!w.is_empty() && w != "-" && !w.starts_with('~') && !w.contains('$') && !w.contains('`'))
        .then(|| w.to_owned())
}

/// One simple command: keywords, assignments and wrappers peeled off,
/// nested shells and `eval` read again, `cd` followed, a git call recorded.
fn simple(words: &[String], depth: usize, out: &mut Analysis) {
    let mut w = words;
    let mut open_args = false;
    let mut env: Vec<String> = Vec::new();
    // `env -C`, `sudo -D`: a directory for this command only.
    let mut steps: Vec<Option<String>> = Vec::new();
    loop {
        while let Some(first) = w.first()
            && is_assignment(first)
        {
            env.push(assigned_name(first).to_owned());
            w = &w[1..];
        }
        let Some(first) = w.first() else {
            // Assignments alone: they change the shell's variables, which
            // an exported one carries to git.
            out.env.extend(env);
            return;
        };
        let lower = basename(first).to_ascii_lowercase();
        let base = lower.strip_suffix(".exe").unwrap_or(&lower);
        w = match base {
            k if KEYWORDS.contains(&k) => skip_opts(&w[1..], &[]),
            "command" | "builtin" | "noglob" | "nohup" | "setsid" => skip_opts(&w[1..], &[]),
            "exec" => skip_opts(&w[1..], &["-a"]),
            "nice" => skip_opts(&w[1..], &["-n"]),
            "stdbuf" => skip_opts(&w[1..], &["-i", "-o", "-e"]),
            "sudo" | "doas" => {
                let mut rest = &w[1..];
                while let Some(x) = rest.first() {
                    if x == "--" {
                        rest = &rest[1..];
                        break;
                    }
                    if !x.starts_with('-') {
                        break;
                    }
                    if x == "-D" || x == "--chdir" {
                        steps.push(rest.get(1).and_then(|d| plain_dir(d)));
                    } else if let Some(d) = x.strip_prefix("--chdir=") {
                        steps.push(plain_dir(d));
                    }
                    let two = [
                        "-u", "-g", "-C", "-D", "-h", "-p", "-r", "-t", "-U", "-T", "--chdir",
                    ];
                    rest = if two.contains(&x.as_str()) {
                        rest.get(2..).unwrap_or(&[])
                    } else {
                        &rest[1..]
                    };
                }
                rest
            }
            "timeout" => {
                let rest = skip_opts(&w[1..], &["-s", "-k"]);
                rest.get(1..).unwrap_or(&[])
            }
            "xargs" => {
                open_args = true;
                skip_opts(
                    &w[1..],
                    &["-I", "-n", "-L", "-P", "-d", "-E", "-s", "-a", "-l"],
                )
            }
            "env" => match env_rest(&w[1..], depth, out, &mut env, &mut steps) {
                Some(rest) => rest,
                None => return,
            },
            "export" | "declare" | "typeset" | "readonly" | "local" => {
                for x in w[1..].iter().filter(|x| !x.starts_with('-')) {
                    out.env.push(assigned_name(x).to_owned());
                }
                return;
            }
            "cd" | "pushd" | "chdir" => {
                let rest = skip_opts(&w[1..], &[]);
                out.cds.push(rest.first().and_then(|d| plain_dir(d)));
                return;
            }
            "popd" => {
                out.cds.push(None);
                return;
            }
            "bash" | "sh" | "zsh" | "dash" | "ksh" | "fish" => {
                shell(w, depth, out);
                return;
            }
            "eval" => {
                analyze(&w[1..].join(" "), depth + 1, out);
                return;
            }
            "git" => {
                if let Some(mut call) = git_call(w, &steps, &out.cds) {
                    call.open_args |= open_args;
                    call.env = env;
                    out.calls.push(call);
                }
                return;
            }
            _ => {
                if first.contains('$') || (!INERT.contains(&base) && has_git_then_sub(&w[1..])) {
                    out.opaque = true;
                }
                return;
            }
        };
    }
}

/// `env`'s options and assignments skipped (names kept, `-C` followed);
/// `-S` read as a command. `None` when nothing is left to look at.
fn env_rest<'a>(
    mut rest: &'a [String],
    depth: usize,
    out: &mut Analysis,
    env: &mut Vec<String>,
    steps: &mut Vec<Option<String>>,
) -> Option<&'a [String]> {
    loop {
        let x = rest.first()?;
        let split = if x == "-S" || x == "--split-string" {
            rest.get(1).map(|s| (s.clone(), 2))
        } else {
            x.strip_prefix("--split-string=")
                .or_else(|| x.strip_prefix("-S"))
                .map(|v| (v.to_owned(), 1))
        };
        if let Some((script, n)) = split {
            // The variables set so far stay with the split command.
            let tail = rest.get(n..).unwrap_or(&[]);
            out.env.append(env);
            analyze(&format!("{script} {}", quote_all(tail)), depth + 1, out);
            return None;
        }
        if x == "-C" || x == "--chdir" {
            steps.push(rest.get(1).and_then(|d| plain_dir(d)));
            rest = rest.get(2..).unwrap_or(&[]);
        } else if let Some(d) = x.strip_prefix("--chdir=") {
            steps.push(plain_dir(d));
            rest = &rest[1..];
        } else if x == "-u" || x == "--unset" {
            rest = rest.get(2..).unwrap_or(&[]);
        } else if x.starts_with('-') {
            rest = &rest[1..];
        } else if is_assignment(x) {
            env.push(assigned_name(x).to_owned());
            rest = &rest[1..];
        } else {
            return Some(rest);
        }
    }
}

/// `bash -c '...'` and its relatives: the script is read again; a script
/// file cannot be.
fn shell(w: &[String], depth: usize, out: &mut Analysis) {
    let mut i = 1;
    while let Some(x) = w.get(i) {
        if x == "-o" || x == "+o" {
            i += 2;
            continue;
        }
        if x.starts_with('-') && !x.starts_with("--") && x.contains('c') {
            match w.get(i + 1) {
                Some(script) => analyze(script, depth + 1, out),
                None => out.opaque = true,
            }
            return;
        }
        if !x.starts_with('-') && !x.starts_with('+') {
            break;
        }
        i += 1;
    }
    if has_git_then_sub(w) {
        out.opaque = true;
    }
}

/// Words joined back into shell text, each single-quoted.
fn quote_all(words: &[String]) -> String {
    words
        .iter()
        .map(|w| format!("'{}'", w.replace('\'', r"'\''")))
        .collect::<Vec<_>>()
        .join(" ")
}

/// A `git` word followed somewhere by `commit` or `push`.
fn has_git_then_sub(words: &[String]) -> bool {
    words.iter().enumerate().any(|(i, w)| {
        let b = basename(w).to_ascii_lowercase();
        (b == "git" || b == "git.exe")
            && words[i + 1..].iter().any(|x| x == "commit" || x == "push")
    })
}

/// A git call from its words (`git` first): global options read (`-C`,
/// `--work-tree`, `--git-dir` become directory steps), the subcommand and
/// its arguments, and the directories it may act in.
fn git_call(words: &[String], pre: &[Option<String>], cds: &[Option<String>]) -> Option<Call> {
    let mut i = 1;
    let mut config = false;
    let mut steps: Vec<Option<String>> = pre.to_vec();
    let mut work_tree: Option<Option<String>> = None;
    let mut git_dir: Option<Option<String>> = None;
    while let Some(w) = words.get(i) {
        if !w.starts_with('-') {
            break;
        }
        i += 1;
        if w.starts_with("-c") || w.starts_with("--config-env") {
            config = true;
        }
        let value = |inline: &str| -> Option<String> {
            match w.strip_prefix(inline).and_then(|r| r.strip_prefix('=')) {
                Some(v) => plain_dir(v),
                None => words.get(i).and_then(|v| plain_dir(v)),
            }
        };
        if w == "-C" {
            steps.push(value("-C"));
        } else if w == "--work-tree" || w.starts_with("--work-tree=") {
            work_tree = Some(value("--work-tree"));
        } else if w == "--git-dir" || w.starts_with("--git-dir=") {
            git_dir = Some(value("--git-dir"));
        }
        if [
            "-C",
            "-c",
            "--git-dir",
            "--work-tree",
            "--namespace",
            "--config-env",
        ]
        .contains(&w.as_str())
        {
            i += 1;
        }
    }
    match (work_tree, git_dir) {
        (Some(t), _) => steps.push(t),
        // `--git-dir=<worktree>/.git`: the worktree is its parent.
        (None, Some(d)) => steps.push(d.and_then(|d| {
            let t = d.trim_end_matches(['/', '\\']);
            (t == ".git" || t.ends_with("/.git") || t.ends_with("\\.git"))
                .then(|| format!("{t}/.."))
        })),
        (None, None) => {}
    }
    let sub = words.get(i)?;
    // The hook's directory and every point the `cd`s before this call lead
    // to (a subshell's `cd` may or may not apply: each prefix is judged).
    let mut candidates: Vec<Option<Vec<String>>> = Vec::new();
    for k in 0..=cds.len() {
        let path: Option<Vec<String>> = cds[..k].iter().chain(steps.iter()).cloned().collect();
        if !candidates.contains(&path) {
            candidates.push(path);
        }
    }
    Some(Call {
        sub: sub.clone(),
        args: words[i + 1..].to_vec(),
        config,
        open_args: false,
        env: Vec::new(),
        candidates,
    })
}

fn is_own(sub: &str) -> bool {
    sub == "commit" || sub == "push" || BUILTIN.contains(&sub)
}

/// Replaces calls to an alias by what it runs, a few levels deep. A lookup
/// git could not answer makes the call unreadable.
fn resolve_aliases(a: &mut Analysis, alias: &dyn Fn(&Call, &str) -> Alias) {
    for _ in 0..5 {
        let mut changed = false;
        for c in std::mem::take(&mut a.calls) {
            let found = if is_own(&c.sub) || c.sub.contains('$') {
                Alias::None
            } else {
                alias(&c, &c.sub)
            };
            let value = match found {
                Alias::None => {
                    a.calls.push(c);
                    continue;
                }
                Alias::Failed => {
                    a.alias_failed = true;
                    a.calls.push(c);
                    continue;
                }
                Alias::Value(v) => v,
            };
            changed = true;
            let text = match value.strip_prefix('!') {
                Some(shell) => format!("{shell} {}", quote_all(&c.args)),
                None => format!("git {value} {}", quote_all(&c.args)),
            };
            let mut inner = Analysis::default();
            analyze(&text, 1, &mut inner);
            a.opaque |= inner.opaque;
            a.env.extend(inner.env);
            for mut ic in inner.calls {
                ic.config |= c.config;
                ic.open_args |= c.open_args;
                ic.env.extend(c.env.iter().cloned());
                // The alias runs where the call was made.
                ic.candidates = c.candidates.clone();
                a.calls.push(ic);
            }
        }
        if !changed {
            return;
        }
    }
    // Still an alias after five levels: not readable.
    if a.calls.iter().any(|c| !is_own(&c.sub)) {
        a.opaque = true;
    }
}

fn is_assignment(w: &str) -> bool {
    w.split_once('=').is_some_and(|(name, _)| {
        !name.is_empty()
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && !name.starts_with(|c: char| c.is_ascii_digit())
    })
}

/// The commands of a text and whether it ended inside a quote, a
/// substitution or a heredoc.
struct Parsed {
    commands: Vec<Vec<String>>,
    incomplete: bool,
}

fn parse_all(src: &str) -> Parsed {
    let chars: Vec<char> = src.chars().collect();
    let mut p = Parser {
        s: &chars,
        i: 0,
        out: Vec::new(),
        incomplete: false,
    };
    p.parse(false);
    Parsed {
        commands: p.out,
        incomplete: p.incomplete,
    }
}

/// Every simple command in `src`, top level and inside substitutions, as
/// its words with quotes removed.
pub fn commands(src: &str) -> Vec<Vec<String>> {
    parse_all(src).commands
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
    /// The text ended inside a quote, a substitution or a heredoc.
    incomplete: bool,
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
                    let mut closed = false;
                    while let Some(n) = self.peek(0) {
                        self.i += 1;
                        if n == '\'' {
                            closed = true;
                            break;
                        }
                        a.cur.push(n);
                    }
                    self.incomplete |= !closed;
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
        // The end of the text inside `$(`, or a heredoc never read.
        self.incomplete |= nested || !heredocs.is_empty();
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
        self.incomplete = true;
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
        self.incomplete = true;
    }

    /// The commands between backticks (no nesting).
    fn backticks(&mut self) {
        let start = self.i;
        while self.peek(0).is_some_and(|c| c != '`') {
            self.i += 1;
        }
        let inner: String = self.s[start..self.i].iter().collect();
        if self.i >= self.s.len() {
            self.incomplete = true;
        }
        self.i += 1;
        let parsed = parse_all(&inner);
        self.incomplete |= parsed.incomplete;
        self.out.extend(parsed.commands);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git(branch: Option<&str>) -> GitInfo {
        GitInfo {
            root: PathBuf::from("/w/acta"),
            main_root: Some(PathBuf::from("/w/acta")),
            branch: branch.map(str::to_owned),
            head_known: true,
            common_dir: PathBuf::from("r/.git"),
            default_branch: Some("main".into()),
            upstream_branch: None,
        }
    }

    /// The old shape: every directory is one attached folder named acta.
    fn check(cmd: &str, f: &CommitSettings, g: &GitInfo) -> Option<String> {
        check_with(cmd, f, g, &|_| None)
    }

    fn check_with(
        cmd: &str,
        f: &CommitSettings,
        g: &GitInfo,
        alias: &dyn Fn(&str) -> Option<String>,
    ) -> Option<String> {
        let place = |_: &Path| Place::Folder {
            name: "acta".into(),
            settings: *f,
            git: Some(Box::new(g.clone())),
        };
        let alias = |_: &Path, n: &str| alias(n).map_or(Alias::None, Alias::Value);
        let cwd = PathBuf::from("/w/acta");
        super::check(
            cmd,
            &Context {
                cwd: &cwd,
                session: &cwd,
                place: &place,
                alias: &alias,
            },
        )
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
            Some(CONFIG),
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
            Some(ENV)
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

    /// Every way of hiding a commit or push from a plain reading: each must
    /// still be denied in an automatic mode (here: `-n` or a force).
    const BYPASSES: &[(&str, &str)] = &[
        // Shell keywords and grouping.
        ("{ git commit -n -m x; }", NO_VERIFY),
        ("! git commit -n -m x", NO_VERIFY),
        ("if true; then git commit -n -m x; fi", NO_VERIFY),
        ("while false; do git push -f; done", FORCED),
        ("until true; do git push -f; done", FORCED),
        ("true && { git push --force; }", FORCED),
        ("time git commit -n -m x", NO_VERIFY),
        ("time -p git commit -n -m x", NO_VERIFY),
        // Wrappers.
        ("env git commit -n -m x", NO_VERIFY),
        ("env -i git commit -n -m x", NO_VERIFY),
        ("env -u HOME -C . git push -f", FORCED),
        ("env -S 'git commit -n -m x'", NO_VERIFY),
        ("env --split-string='git push -f'", FORCED),
        ("command git commit -n -m x", NO_VERIFY),
        ("builtin command -p git push -f", FORCED),
        ("exec git commit -n -m x", NO_VERIFY),
        ("exec -a g git push -f", FORCED),
        ("nohup git push -f", FORCED),
        ("sudo -u alexandre git commit -n -m x", NO_VERIFY),
        ("sudo -E -- git push -f", FORCED),
        ("doas git push -f", FORCED),
        ("nice -n 5 git commit -n -m x", NO_VERIFY),
        ("timeout 30 git commit -n -m x", NO_VERIFY),
        ("timeout -s KILL 30 git push -f", FORCED),
        ("stdbuf -o L git push -f", FORCED),
        ("noglob git push -f", FORCED),
        ("setsid git push -f", FORCED),
        ("LANG=C env LC_ALL=C nohup git commit -n -m x", NO_VERIFY),
        // Nested shells and eval.
        ("bash -c 'git commit -n -m x'", NO_VERIFY),
        ("sh -lc \"git push --force\"", FORCED),
        (
            "zsh -o pipefail -c 'git commit --no-verify -m x'",
            NO_VERIFY,
        ),
        ("/bin/bash -ec 'cd x && git push -f'", FORCED),
        ("bash -c \"bash -c 'git push -f'\"", FORCED),
        ("eval git commit -n -m x", NO_VERIFY),
        ("eval \"git push --force\"", FORCED),
        // Arguments from elsewhere.
        ("echo x | xargs git commit -m", DIRECT),
        ("xargs -n1 git push < remotes", DIRECT),
        // Unreadable commands.
        ("$GIT commit -n -m x", DIRECT),
        ("\"$(which git)\" push -f", DIRECT),
        ("git $(echo commit) -n", DIRECT),
        ("watch git push -f", DIRECT),
        ("find . -exec git commit -n -m x \\;", DIRECT),
        ("git commit -m 'unterminated && git push", DIRECT),
        ("bash ./commit-all.sh git commit", DIRECT),
        // Config overrides.
        ("git -c user.name=x commit -m x", CONFIG),
        ("git -c alias.ci='commit -n' ci -m x", CONFIG),
        ("git --config-env=core.hooksPath=H commit -m x", CONFIG),
        (
            "GIT_CONFIG_PARAMETERS=\"'core.hookspath'='/x'\" git commit -m x",
            CONFIG,
        ),
        ("export GIT_CONFIG_COUNT=1; git commit -m x", CONFIG),
        (
            "env GIT_CONFIG_KEY_0=core.hooksPath GIT_CONFIG_VALUE_0=/x git push",
            CONFIG,
        ),
        // Pruning is deleting.
        ("git push --prune origin", FORCED),
        ("git push --prun", FORCED),
    ];

    #[test]
    fn every_bypass_form_is_denied() {
        let g = git(Some("t"));
        for mode in [COMMIT, PUSH] {
            for (cmd, want) in BYPASSES {
                let got = check(cmd, &folder(mode, false), &g);
                assert_eq!(got.as_deref(), Some(*want), "{cmd} {mode:?}");
            }
        }
        for (cmd, _) in BYPASSES {
            assert_eq!(check(cmd, &folder(MANUAL, false), &g), None, "{cmd}");
            assert!(needs_check(cmd), "{cmd}");
        }
        // Read-only and inert commands stay quiet and cheap.
        for cmd in [
            "git status",
            "git log --oneline -5 && git diff",
            "echo git push --force",
            "grep -r 'git commit -n' docs",
            "ls -la",
            "git commit -m \"fix: bash -c is fine; time and env too\"",
        ] {
            assert_eq!(check(cmd, &folder(PUSH, false), &g), None, "{cmd}");
        }
        assert!(!needs_check("git status && git log -1"));
        assert!(!needs_check("cargo test"));
        assert!(needs_check("git ci -m x"), "an alias is looked up");
    }

    #[test]
    fn commit_reads_only_inside_the_repository() {
        let g = git(Some("t"));
        for mode in [COMMIT, PUSH] {
            let f = folder(mode, false);
            for cmd in [
                "git commit --allow-empty -F ~/.ssh/id_ed25519",
                "git commit -F /home/alexandre/.ssh/id_ed25519",
                "git commit -F ../other/secret",
                "git commit -F sub/../../secret",
                "git commit -F$HOME/.netrc",
                "git commit -aF /etc/passwd",
                "git commit --file /etc/passwd",
                "git commit --file=/etc/passwd",
                "git commit --fil /etc/passwd",
                "git commit --pathspec-from-file=/etc/passwd",
                "git commit --pathspec-f /etc/passwd",
                "git commit -t /etc/passwd",
                "git commit --template=../x",
                "git commit --temp /etc/passwd",
                "git commit -C /etc/passwd",
                "git commit -c ../x",
                "git commit --reuse-message=HEAD:secret",
                "git commit --reedit-message ~/x",
                "git commit -C$HOME",
                "git -C sub commit -F ../../x",
                "cd sub && git commit -F ../../x",
            ] {
                assert_eq!(check(cmd, &f, &g).as_deref(), Some(READS), "{cmd}");
            }
            for cmd in [
                "git commit -F msg.txt",
                "git commit -F -",
                "git commit -F- <<'EOF'\nMessage\nEOF",
                "git commit --file=./notes/msg",
                "git commit -F sub/../msg",
                "git commit --pathspec-from-file=-",
                "git commit -t .gitmessage",
                "git commit -C HEAD",
                "git commit --reuse-message=HEAD~1",
                "git commit -c abc1234",
                "git commit -m \"-F /etc/passwd\"",
                "git commit -m x -- -F",
                "git -C sub commit -F ../msg",
            ] {
                assert_eq!(check(cmd, &f, &g), None, "{cmd}");
            }
        }
        // Manual folders are not guarded at all.
        assert_eq!(
            check("git commit -F /etc/passwd", &folder(MANUAL, false), &g),
            None
        );
    }

    #[test]
    fn aliases_are_resolved() {
        let g = git(Some("t"));
        let f = folder(PUSH, false);
        let alias = |name: &str| -> Option<String> {
            match name {
                "ci" => Some("commit -n".into()),
                "p" => Some("push --force".into()),
                "c2" => Some("ci".into()),
                "sh1" => Some("!git commit --no-verify".into()),
                "st" => Some("status".into()),
                "loop" => Some("loop".into()),
                _ => None,
            }
        };
        let run = |cmd: &str| check_with(cmd, &f, &g, &alias);
        assert_eq!(run("git ci -m x").as_deref(), Some(NO_VERIFY));
        assert_eq!(run("git c2 -m x").as_deref(), Some(NO_VERIFY));
        assert_eq!(run("git p").as_deref(), Some(FORCED));
        assert_eq!(run("git sh1").as_deref(), Some(NO_VERIFY));
        assert_eq!(run("git st"), None);
        assert_eq!(run("git loop commit").as_deref(), Some(DIRECT));
        assert_eq!(run("git unknown-tool"), None);
    }

    #[test]
    fn unknown_branches_and_the_upstream() {
        let f = folder(PUSH, false);
        let mut g = git(Some("t"));
        g.head_known = false;
        g.branch = None;
        assert_eq!(check("git commit -m x", &f, &g).as_deref(), Some(UNKNOWN));
        let mut g = git(Some("t"));
        g.default_branch = None;
        assert_eq!(check("git push", &f, &g).as_deref(), Some(UNKNOWN));
        // A feature branch whose upstream is the default branch.
        let mut g = git(Some("t"));
        g.upstream_branch = Some("main".into());
        assert!(check("git push", &f, &g).unwrap().contains("on main"));
        assert_eq!(check("git commit -m x", &f, &g), None);
        assert_eq!(check("git push", &folder(PUSH, true), &g), None);
        g.upstream_branch = Some("t".into());
        assert_eq!(check("git push", &f, &g), None);
    }

    /// A small world: an automatic folder, a manual one, one Acta cannot
    /// read, and the rest unlinked.
    fn world(cmd: &str, cwd: &str, alias: Alias) -> Option<String> {
        let auto = folder(COMMIT, false);
        let place = |dir: &Path| -> Place {
            if dir.starts_with("/ws/auto") {
                Place::Folder {
                    name: "auto".into(),
                    settings: auto,
                    git: Some(Box::new(git(Some("t")))),
                }
            } else if dir.starts_with("/ws/manual") {
                Place::Folder {
                    name: "manual".into(),
                    settings: folder(MANUAL, false),
                    git: Some(Box::new(git(Some("t")))),
                }
            } else if dir.starts_with("/ws/locked") {
                Place::Unreadable
            } else {
                Place::Unlinked
            }
        };
        let alias = |_: &Path, _: &str| alias.clone();
        let cwd = PathBuf::from(cwd);
        super::check(
            cmd,
            &Context {
                cwd: &cwd,
                session: &cwd,
                place: &place,
                alias: &alias,
            },
        )
    }

    #[test]
    fn each_call_is_judged_where_it_acts() {
        let m = manual("manual");
        let out = outside("auto");
        let rows: &[(&str, Option<&str>)] = &[
            ("git commit -m x", None),
            ("cd sub && git commit -m x", None),
            ("cd /ws/auto/sub; git commit -m x", None),
            ("cd ../manual && git commit -m x", Some(&m)),
            ("cd .. && cd manual && git commit -m x", Some(&out)),
            ("cd ../manual; ( cd ../auto ); git commit -m x", Some(&m)),
            (
                "cd ../auto/sub; (cd ../../manual); git commit -m x",
                Some(&m),
            ),
            ("pushd /ws/manual; git commit -m x", Some(&m)),
            ("(cd ../manual); git commit -m x", Some(&m)),
            ("git -C ../manual commit -m x", Some(&m)),
            ("git -C .. -C manual push", Some(&m)),
            ("git --work-tree=../manual commit -m x", Some(&m)),
            ("git --git-dir ../manual/.git commit -m x", Some(&m)),
            ("env -C ../manual git commit -m x", Some(&m)),
            ("sudo -D ../manual git commit -m x", Some(&m)),
            ("bash -c 'cd ../manual && git commit -m x'", Some(&m)),
            ("cd ../other && git commit -m x", Some(&out)),
            ("git -C /tmp/elsewhere push", Some(&out)),
            ("cd $HOME/x && git commit -m x", Some(DIR)),
            ("cd - && git commit -m x", Some(DIR)),
            ("cd ~/x && git commit -m x", Some(DIR)),
            ("cd && git commit -m x", Some(DIR)),
            (
                "cd \"$(git rev-parse --show-toplevel)\" && git commit -m x",
                Some(DIR),
            ),
            ("popd; git commit -m x", Some(DIR)),
            ("git --git-dir=/srv/repo.git commit -m x", Some(DIR)),
            ("git -C \"$D\" commit -m x", Some(DIR)),
            ("cd ../locked && git commit -m x", Some(FAIL)),
            // Not a commit or push: nothing to judge.
            ("cd ../manual && git status", None),
        ];
        for (cmd, want) in rows {
            assert_eq!(
                world(cmd, "/ws/auto", Alias::None).as_deref(),
                *want,
                "{cmd}"
            );
        }
        // From a manual session, an automatic folder's rules still apply.
        assert_eq!(
            world(
                "cd ../auto && git commit -n -m x",
                "/ws/manual",
                Alias::None
            )
            .as_deref(),
            Some(NO_VERIFY)
        );
        assert_eq!(world("git commit -n -m x", "/ws/manual", Alias::None), None);
        assert_eq!(
            world("cd ../other && git commit -n", "/ws/manual", Alias::None),
            None
        );
        assert_eq!(
            world("cd $X && git commit -n", "/ws/manual", Alias::None),
            None
        );
        // Fail closed where Acta cannot read the folder.
        assert_eq!(
            world("git commit -m x", "/ws/locked", Alias::None).as_deref(),
            Some(FAIL)
        );
        assert_eq!(world("git status", "/ws/locked", Alias::None), None);
        // An alias git could not answer for.
        assert_eq!(
            world("git ci -m x", "/ws/auto", Alias::Failed).as_deref(),
            Some(DIRECT)
        );
        assert_eq!(world("git ci -m x", "/ws/auto", Alias::None), None);
    }

    #[test]
    fn variables_on_a_commit_or_push_are_refused() {
        let rows: &[(&str, Option<&str>)] = &[
            ("HUSKY=0 git commit -m x", Some(ENV)),
            ("HUSKY_SKIP_HOOKS=1 git commit -m x", Some(ENV)),
            ("SKIP=lint git commit -m x", Some(ENV)),
            ("LEFTHOOK=0 git push", Some(ENV)),
            ("LEFTHOOK_EXCLUDE=lint git commit -m x", Some(ENV)),
            (
                "env PRE_COMMIT_ALLOW_NO_CONFIG=1 git commit -m x",
                Some(ENV),
            ),
            ("export SKIP=lint; git commit -m x", Some(ENV)),
            ("export HUSKY; git commit -m x", Some(ENV)),
            ("declare -x HUSKY=0; git commit -m x", Some(ENV)),
            ("LEFTHOOK=0; git commit -m x", Some(ENV)),
            ("GIT_DIR=../manual/.git git commit -m x", Some(ENV)),
            ("GIT_CONFIG_COUNT=1 git commit -m x", Some(CONFIG)),
            ("LANG=C LC_ALL=C TZ=UTC TERM=dumb git commit -m x", None),
            ("HUSKY=0 npm test && git commit -m x", None),
            ("git commit -m \"HUSKY=0 is gone\"", None),
        ];
        for (cmd, want) in rows {
            assert_eq!(
                world(cmd, "/ws/auto", Alias::None).as_deref(),
                *want,
                "{cmd}"
            );
            assert_eq!(world(cmd, "/ws/other", Alias::None), None, "{cmd}");
        }
    }

    #[cfg(windows)]
    #[test]
    fn git_bash_paths_are_windows_paths() {
        assert_eq!(
            walk(Path::new("C:\\w"), &["/d/acta".into()]),
            PathBuf::from("D:\\acta")
        );
        assert_eq!(
            walk(Path::new("C:\\w\\a"), &["../b".into(), "./c".into()]),
            PathBuf::from("C:\\w\\b\\c")
        );
    }

    #[test]
    fn reasons_fit_the_hook_budget() {
        let long = on_default(&"b".repeat(300));
        let (m, o) = (manual(&"n".repeat(300)), outside(&"n".repeat(300)));
        for r in [
            NO_VERIFY,
            FORCED,
            EXACT_PUSH,
            DETACHED,
            UNKNOWN,
            CONFIG,
            DIRECT,
            ENV,
            DIR,
            FAIL,
            READS,
            long.as_str(),
            m.as_str(),
            o.as_str(),
        ] {
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
