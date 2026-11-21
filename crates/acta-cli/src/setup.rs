//! `acta setup` (ADR 0005, section 1): the generated local plugin, its
//! registration through the `claude` CLI, and one allow rule.
//!
//! Idempotent: every step checks before it writes. Reversible: `--remove`
//! undoes only what setup wrote, using a record it keeps next to the plugin.
//! It never rewrites a settings file it cannot parse.

use serde_json::{Map, Value, json};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const MARKETPLACE: &str = "acta";
pub const PLUGIN: &str = "acta";
pub const PLUGIN_ID: &str = "acta@acta";
pub const SERVER: &str = "acta";
pub const ALLOW_RULE: &str = "mcp__plugin_acta_acta__*";
/// What setup did that `--remove` must undo; kept in the plugin directory.
const RECORD: &str = "setup-record.json";

const SKILLS: &[(&str, &str)] = &[
    (
        "using-acta",
        include_str!("../plugin/skills/using-acta/SKILL.md"),
    ),
    ("triage", include_str!("../plugin/skills/triage/SKILL.md")),
    ("wrap", include_str!("../plugin/skills/wrap/SKILL.md")),
];

/// Runs the `claude` CLI. Tests inject a fake; nothing in a test runs the
/// real one or touches the real `~/.claude`.
pub trait ClaudeCli {
    /// `Ok(stdout)` on exit status 0, `Err(output)` otherwise.
    fn run(&self, args: &[&str]) -> Result<String, String>;
}

pub struct RealClaude {
    pub config_dir: Option<PathBuf>,
}

impl ClaudeCli for RealClaude {
    fn run(&self, args: &[&str]) -> Result<String, String> {
        let mut cmd = std::process::Command::new(claude_program());
        cmd.args(args).stdin(std::process::Stdio::null());
        if let Some(d) = &self.config_dir {
            cmd.env("CLAUDE_CONFIG_DIR", d);
        }
        let out = cmd.output().map_err(|e| {
            format!(
                "cannot run claude ({}): is Claude Code installed?",
                e.kind()
            )
        })?;
        let text = String::from_utf8_lossy(&out.stdout).into_owned();
        if out.status.success() {
            Ok(text)
        } else {
            Err(format!("{text}{}", String::from_utf8_lossy(&out.stderr)))
        }
    }
}

/// `claude` is an npm shim (`claude.cmd`) or a native `claude.exe` on
/// Windows; `Command` does not resolve `.cmd`, so look for it on `PATH`.
fn claude_program() -> PathBuf {
    if cfg!(windows)
        && let Some(path) = std::env::var_os("PATH")
    {
        for dir in std::env::split_paths(&path) {
            for name in ["claude.exe", "claude.cmd"] {
                let p = dir.join(name);
                if p.is_file() {
                    return p;
                }
            }
        }
    }
    PathBuf::from("claude")
}

pub struct Setup<'a> {
    /// `<data dir>/claude`: the marketplace root.
    pub plugin_root: PathBuf,
    /// Claude Code's config directory (`CLAUDE_CONFIG_DIR` or `~/.claude`).
    pub claude_dir: PathBuf,
    /// Absolute path of the `acta` executable written into the plugin.
    pub exe: PathBuf,
    pub claude: &'a dyn ClaudeCli,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    pub name: &'static str,
    pub ok: bool,
    pub detail: String,
}

impl Step {
    fn new(name: &'static str, ok: bool, detail: impl Into<String>) -> Step {
        Step {
            name,
            ok,
            detail: detail.into(),
        }
    }
}

pub fn render_steps(steps: &[Step], json_out: bool) -> String {
    if json_out {
        let items: Vec<Value> = steps
            .iter()
            .map(|s| json!({"step": s.name, "ok": s.ok, "detail": s.detail}))
            .collect();
        return json!({"ok": steps.iter().all(|s| s.ok), "steps": items}).to_string();
    }
    steps
        .iter()
        .map(|s| {
            format!(
                "{:<5} {:<13} {}",
                if s.ok { "ok" } else { "todo" },
                s.name,
                s.detail
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

impl Setup<'_> {
    fn plugin_dir(&self) -> PathBuf {
        self.plugin_root.join("plugins").join(PLUGIN)
    }

    fn settings_path(&self) -> PathBuf {
        self.claude_dir.join("settings.json")
    }

    /// Every file of the plugin tree with its content.
    pub fn files(&self) -> Vec<(PathBuf, String)> {
        let exe = self.exe.to_string_lossy().into_owned();
        let server = format!("plugin:{PLUGIN}:{SERVER}");
        let pretty = |v: Value| serde_json::to_string_pretty(&v).expect("json") + "\n";
        let mcp_hook = |event: &str| {
            json!({"type": "mcp_tool", "server": server, "tool": "hook",
                   "input": {"event": event, "session": "${session_id}"}})
        };
        let p = self.plugin_dir();
        let mut out = vec![
            (
                self.plugin_root
                    .join(".claude-plugin")
                    .join("marketplace.json"),
                pretty(json!({
                    "name": MARKETPLACE,
                    "owner": {"name": "Acta"},
                    "description": "Local marketplace generated by `acta setup`; holds the Acta plugin only.",
                    "plugins": [{
                        "name": PLUGIN,
                        "source": "./plugins/acta",
                        "description": "Acta: project tracker and scoped memory for Claude Code"
                    }]
                })),
            ),
            (
                p.join(".claude-plugin").join("plugin.json"),
                pretty(json!({
                    "name": PLUGIN,
                    "version": acta_core::VERSION,
                    "description": "Acta's MCP server, hooks and skills. Generated by `acta setup`; rewritten on every Acta update.",
                    "author": {"name": "Acta"}
                })),
            ),
            (
                p.join(".mcp.json"),
                pretty(json!({"mcpServers": {SERVER: {"command": exe, "args": ["mcp"]}}})),
            ),
            (
                p.join("hooks").join("hooks.json"),
                pretty(json!({"hooks": {
                    "SessionStart": [
                        {"matcher": "startup|resume", "hooks": [
                            {"type": "command", "command": exe, "args": ["hook", "session-start"], "timeout": 10}
                        ]},
                        {"matcher": "clear", "hooks": [mcp_hook("clear")]},
                        {"matcher": "compact", "hooks": [mcp_hook("compact")]}
                    ],
                    "PreCompact": [{"hooks": [
                        {"type": "command", "command": exe, "args": ["hook", "pre-compact"], "timeout": 10}
                    ]}],
                    "SubagentStart": [{"hooks": [mcp_hook("subagent")]}]
                }})),
            ),
        ];
        for (name, text) in SKILLS {
            out.push((
                p.join("skills").join(name).join("SKILL.md"),
                (*text).to_owned(),
            ));
        }
        out
    }

    fn files_current(&self) -> bool {
        self.files()
            .iter()
            .all(|(path, text)| fs::read_to_string(path).is_ok_and(|t| &t == text))
    }

    fn write_files(&self) -> io::Result<usize> {
        let mut written = 0;
        for (path, text) in self.files() {
            if fs::read_to_string(&path).is_ok_and(|t| t == text) {
                continue;
            }
            if let Some(dir) = path.parent() {
                fs::create_dir_all(dir)?;
            }
            write_atomic(&path, &text)?;
            written += 1;
        }
        Ok(written)
    }

    fn marketplace_added(&self) -> Result<bool, String> {
        let out = self
            .claude
            .run(&["plugin", "marketplace", "list", "--json"])?;
        Ok(json_names(&out).iter().any(|n| n == MARKETPLACE))
    }

    fn plugin_installed(&self) -> Result<bool, String> {
        let out = self.claude.run(&["plugin", "list", "--json"])?;
        Ok(json_names(&out)
            .iter()
            .any(|n| n == PLUGIN_ID || n == PLUGIN))
    }

    fn record(&self) -> Value {
        fs::read_to_string(self.plugin_root.join(RECORD))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_else(|| json!({}))
    }

    fn save_record(&self, record: &Value) -> io::Result<()> {
        fs::create_dir_all(&self.plugin_root)?;
        write_atomic(
            &self.plugin_root.join(RECORD),
            &(serde_json::to_string_pretty(record).expect("json") + "\n"),
        )
    }

    /// Runs every step. Each one is skipped when already done.
    pub fn install(&self, allow: bool) -> Vec<Step> {
        let mut steps = Vec::new();
        let mut record = self.record();
        match self.write_files() {
            Ok(0) => steps.push(Step::new("plugin files", true, "up to date")),
            Ok(n) => steps.push(Step::new("plugin files", true, format!("{n} written"))),
            Err(e) => {
                steps.push(Step::new(
                    "plugin files",
                    false,
                    format!("cannot write: {}", e.kind()),
                ));
                return steps;
            }
        }
        let root = self.plugin_root.to_string_lossy().into_owned();
        match self.marketplace_added() {
            Ok(true) => steps.push(Step::new("marketplace", true, "already added")),
            _ => match self.claude.run(&["plugin", "marketplace", "add", &root]) {
                Ok(_) => {
                    record["marketplace_added"] = json!(true);
                    steps.push(Step::new("marketplace", true, "added"));
                }
                Err(e) => steps.push(Step::new("marketplace", false, one_line(&e))),
            },
        }
        match self.plugin_installed() {
            Ok(true) => steps.push(Step::new("plugin", true, "already installed")),
            _ => match self
                .claude
                .run(&["plugin", "install", PLUGIN_ID, "--scope", "user"])
            {
                Ok(_) => {
                    record["plugin_installed"] = json!(true);
                    steps.push(Step::new("plugin", true, "installed (user scope)"));
                }
                Err(e) => steps.push(Step::new("plugin", false, one_line(&e))),
            },
        }
        if allow {
            match edit_allow_rule(&self.settings_path(), true) {
                Ok(Edit::Changed) => {
                    record["allow_rule_added"] = json!(true);
                    steps.push(Step::new(
                        "allow rule",
                        true,
                        format!("added {ALLOW_RULE} to settings.json (a backup was kept)"),
                    ));
                }
                Ok(Edit::Unchanged) => steps.push(Step::new("allow rule", true, "already present")),
                Err(e) => steps.push(Step::new("allow rule", false, e)),
            }
        } else {
            steps.push(Step::new("allow rule", true, "skipped (--no-allow)"));
        }
        if let Err(e) = self.save_record(&record) {
            steps.push(Step::new(
                "record",
                false,
                format!("cannot save: {}", e.kind()),
            ));
        }
        steps
    }

    pub fn check(&self) -> Vec<Step> {
        let exe_ok = self.exe.is_file();
        let mut steps = vec![
            Step::new(
                "executable",
                exe_ok,
                if exe_ok {
                    "found"
                } else {
                    "missing: run acta setup again"
                },
            ),
            Step::new(
                "plugin files",
                self.files_current(),
                if self.files_current() {
                    "up to date"
                } else {
                    "missing or outdated"
                },
            ),
        ];
        let state = |r: Result<bool, String>, yes: &str, no: &str| match r {
            Ok(true) => (true, yes.to_owned()),
            Ok(false) => (false, no.to_owned()),
            Err(e) => (false, one_line(&e)),
        };
        let (ok, d) = state(self.marketplace_added(), "added", "not added");
        steps.push(Step::new("marketplace", ok, d));
        let (ok, d) = state(self.plugin_installed(), "installed", "not installed");
        steps.push(Step::new("plugin", ok, d));
        let allow = read_settings(&self.settings_path())
            .ok()
            .flatten()
            .is_some_and(|v| has_allow_rule(&v));
        steps.push(Step::new(
            "allow rule",
            allow,
            if allow {
                "present"
            } else {
                "absent: every Acta tool call will prompt"
            },
        ));
        steps
    }

    /// Undoes what setup did: the plugin, the marketplace, the allow rule if
    /// setup added it, and the plugin directory.
    pub fn remove(&self) -> Vec<Step> {
        let record = self.record();
        let mut steps = Vec::new();
        match self.plugin_installed() {
            Ok(false) => steps.push(Step::new("plugin", true, "not installed")),
            _ => match self
                .claude
                .run(&["plugin", "uninstall", PLUGIN_ID, "--scope", "user"])
            {
                Ok(_) => steps.push(Step::new("plugin", true, "uninstalled")),
                Err(e) => steps.push(Step::new("plugin", false, one_line(&e))),
            },
        }
        match self.marketplace_added() {
            Ok(false) => steps.push(Step::new("marketplace", true, "not added")),
            _ => match self
                .claude
                .run(&["plugin", "marketplace", "remove", MARKETPLACE])
            {
                Ok(_) => steps.push(Step::new("marketplace", true, "removed")),
                Err(e) => steps.push(Step::new("marketplace", false, one_line(&e))),
            },
        }
        if record["allow_rule_added"] == json!(true) {
            match edit_allow_rule(&self.settings_path(), false) {
                Ok(Edit::Changed) => steps.push(Step::new("allow rule", true, "removed")),
                Ok(Edit::Unchanged) => steps.push(Step::new("allow rule", true, "already absent")),
                Err(e) => steps.push(Step::new("allow rule", false, e)),
            }
        } else {
            steps.push(Step::new(
                "allow rule",
                true,
                "left as is (not added by setup)",
            ));
        }
        let files_ok = steps.iter().all(|s| s.ok);
        if files_ok {
            match fs::remove_dir_all(&self.plugin_root) {
                Ok(()) => steps.push(Step::new("plugin files", true, "removed")),
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    steps.push(Step::new("plugin files", true, "already absent"))
                }
                Err(e) => steps.push(Step::new(
                    "plugin files",
                    false,
                    format!("cannot remove: {}", e.kind()),
                )),
            }
        } else {
            steps.push(Step::new(
                "plugin files",
                false,
                "kept until the steps above succeed",
            ));
        }
        steps
    }
}

fn one_line(text: &str) -> String {
    let t = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.chars().count() > 160 {
        format!("{}...", t.chars().take(157).collect::<String>())
    } else {
        t
    }
}

/// Names found in `claude plugin list --json` / `marketplace list --json`
/// output: every `name` or `id` string of the top-level items. Lenient on
/// purpose: the format is Claude Code's, not ours.
fn json_names(text: &str) -> Vec<String> {
    let Ok(v) = serde_json::from_str::<Value>(text.trim()) else {
        return Vec::new();
    };
    let items: Vec<&Value> = match &v {
        Value::Array(a) => a.iter().collect(),
        Value::Object(o) => o
            .values()
            .flat_map(|x| match x {
                Value::Array(a) => a.iter().collect::<Vec<_>>(),
                other => vec![other],
            })
            .collect(),
        _ => Vec::new(),
    };
    let mut out = Vec::new();
    for it in items {
        for key in ["name", "id", "plugin"] {
            if let Some(s) = it.get(key).and_then(Value::as_str) {
                out.push(s.to_owned());
            }
        }
    }
    if let Value::Object(o) = &v {
        out.extend(o.keys().cloned());
    }
    out
}

pub enum Edit {
    Changed,
    Unchanged,
}

/// `Ok(None)` when the file does not exist. A file that is not a JSON
/// object is an error: setup never rewrites what it cannot read.
pub fn read_settings(path: &Path) -> Result<Option<Value>, String> {
    match fs::read_to_string(path) {
        Ok(t) if t.trim().is_empty() => Ok(Some(json!({}))),
        Ok(t) => match serde_json::from_str::<Value>(&t) {
            Ok(v @ Value::Object(_)) => Ok(Some(v)),
            _ => Err(
                "settings.json is not a JSON object setup can edit; add the rule by hand".into(),
            ),
        },
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("cannot read settings.json: {}", e.kind())),
    }
}

fn has_allow_rule(v: &Value) -> bool {
    v.pointer("/permissions/allow")
        .and_then(Value::as_array)
        .is_some_and(|a| a.iter().any(|x| x == ALLOW_RULE))
}

/// Adds or removes the single allow rule, keeping everything else as it was
/// (key order included). A backup of the previous file is kept once.
pub fn edit_allow_rule(path: &Path, add: bool) -> Result<Edit, String> {
    let existing = read_settings(path)?;
    let mut v = existing.clone().unwrap_or_else(|| json!({}));
    if has_allow_rule(&v) == add {
        return Ok(Edit::Unchanged);
    }
    let obj = v.as_object_mut().expect("object");
    if add {
        let perms = obj
            .entry("permissions")
            .or_insert_with(|| Value::Object(Map::new()));
        let Some(perms) = perms.as_object_mut() else {
            return Err("settings.json has a permissions value that is not an object".into());
        };
        let allow = perms.entry("allow").or_insert_with(|| json!([]));
        let Some(allow) = allow.as_array_mut() else {
            return Err("settings.json has a permissions.allow value that is not a list".into());
        };
        allow.push(json!(ALLOW_RULE));
    } else if let Some(allow) = obj
        .get_mut("permissions")
        .and_then(|p| p.get_mut("allow"))
        .and_then(Value::as_array_mut)
    {
        allow.retain(|x| x != ALLOW_RULE);
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)
            .map_err(|e| format!("cannot create the settings folder: {}", e.kind()))?;
    }
    if existing.is_some() {
        let backup = path.with_extension("json.acta-backup");
        if !backup.exists() {
            fs::copy(path, &backup)
                .map_err(|e| format!("cannot back up settings.json: {}", e.kind()))?;
        }
    }
    let text = serde_json::to_string_pretty(&v).expect("json") + "\n";
    write_atomic(path, &text).map_err(|e| format!("cannot write settings.json: {}", e.kind()))?;
    Ok(Edit::Changed)
}

/// Write to a sibling temporary file, then rename over the target.
pub fn write_atomic(path: &Path, text: &str) -> io::Result<()> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = path.with_file_name(format!(".{name}.acta-tmp-{}", std::process::id()));
    fs::write(&tmp, text)?;
    fs::rename(&tmp, path).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// A fake `claude` that keeps the marketplace and plugin state in memory.
    #[derive(Default)]
    struct FakeClaude {
        calls: RefCell<Vec<String>>,
        marketplaces: RefCell<Vec<String>>,
        plugins: RefCell<Vec<String>>,
    }

    impl ClaudeCli for FakeClaude {
        fn run(&self, args: &[&str]) -> Result<String, String> {
            self.calls.borrow_mut().push(args.join(" "));
            let list = |v: &RefCell<Vec<String>>| {
                serde_json::to_string(
                    &v.borrow()
                        .iter()
                        .map(|n| json!({"name": n}))
                        .collect::<Vec<_>>(),
                )
                .unwrap()
            };
            match args {
                ["plugin", "marketplace", "list", "--json"] => Ok(list(&self.marketplaces)),
                ["plugin", "list", "--json"] => Ok(list(&self.plugins)),
                ["plugin", "marketplace", "add", _] => {
                    self.marketplaces.borrow_mut().push(MARKETPLACE.into());
                    Ok(String::new())
                }
                ["plugin", "install", id, ..] => {
                    self.plugins.borrow_mut().push((*id).into());
                    Ok(String::new())
                }
                ["plugin", "uninstall", ..] => {
                    self.plugins.borrow_mut().clear();
                    Ok(String::new())
                }
                ["plugin", "marketplace", "remove", _] => {
                    self.marketplaces.borrow_mut().clear();
                    Ok(String::new())
                }
                _ => Err(format!("unexpected: {}", args.join(" "))),
            }
        }
    }

    fn setup<'a>(dir: &Path, claude: &'a FakeClaude) -> Setup<'a> {
        let exe = dir.join("bin").join("acta.exe");
        fs::create_dir_all(exe.parent().unwrap()).unwrap();
        fs::write(&exe, "").unwrap();
        Setup {
            plugin_root: dir.join("data").join("claude"),
            claude_dir: dir.join("home").join(".claude"),
            exe,
            claude,
        }
    }

    #[test]
    fn install_is_idempotent_and_remove_restores_user_settings() {
        let tmp = tempfile::tempdir().unwrap();
        let claude = FakeClaude::default();
        let s = setup(tmp.path(), &claude);
        let user = r#"{
  "theme": "dark",
  "permissions": {
    "allow": ["Bash(git status)"],
    "deny": []
  },
  "model": "opus"
}
"#;
        fs::create_dir_all(&s.claude_dir).unwrap();
        fs::write(s.settings_path(), user).unwrap();

        let first = s.install(true);
        assert!(first.iter().all(|x| x.ok), "{first:?}");
        assert!(s.check().iter().all(|x| x.ok), "{:?}", s.check());
        let after = fs::read_to_string(s.settings_path()).unwrap();
        let v: Value = serde_json::from_str(&after).unwrap();
        assert_eq!(
            v["permissions"]["allow"],
            json!(["Bash(git status)", ALLOW_RULE])
        );
        // Key order and other keys are kept.
        let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["theme", "permissions", "model"]);
        assert_eq!(
            fs::read_to_string(s.settings_path().with_extension("json.acta-backup")).unwrap(),
            user
        );

        // Second run: nothing changes, no install command repeated.
        let calls_before = claude.calls.borrow().len();
        let second = s.install(true);
        assert!(second.iter().all(|x| x.ok));
        assert_eq!(second[0].detail, "up to date");
        assert_eq!(fs::read_to_string(s.settings_path()).unwrap(), after);
        let new_calls: Vec<String> = claude.calls.borrow()[calls_before..].to_vec();
        assert!(
            new_calls.iter().all(|c| c.ends_with("--json")),
            "{new_calls:?}"
        );

        // The executable path is in the plugin, in exec form.
        let hooks = fs::read_to_string(s.plugin_dir().join("hooks").join("hooks.json")).unwrap();
        let h: Value = serde_json::from_str(&hooks).unwrap();
        assert_eq!(
            h["hooks"]["SessionStart"][0]["hooks"][0]["command"],
            json!(s.exe.to_string_lossy())
        );
        assert_eq!(
            h["hooks"]["SessionStart"][1]["hooks"][0]["server"],
            "plugin:acta:acta"
        );

        let removed = s.remove();
        assert!(removed.iter().all(|x| x.ok), "{removed:?}");
        let restored: Value =
            serde_json::from_str(&fs::read_to_string(s.settings_path()).unwrap()).unwrap();
        assert_eq!(restored, serde_json::from_str::<Value>(user).unwrap());
        assert!(!s.plugin_root.exists());
        assert!(claude.plugins.borrow().is_empty() && claude.marketplaces.borrow().is_empty());
    }

    #[test]
    fn a_rule_the_user_had_is_never_removed() {
        let tmp = tempfile::tempdir().unwrap();
        let claude = FakeClaude::default();
        let s = setup(tmp.path(), &claude);
        fs::create_dir_all(&s.claude_dir).unwrap();
        let user = format!("{{\"permissions\":{{\"allow\":[\"{ALLOW_RULE}\"]}}}}");
        fs::write(s.settings_path(), &user).unwrap();
        s.install(true);
        assert_eq!(fs::read_to_string(s.settings_path()).unwrap(), user);
        s.remove();
        assert_eq!(fs::read_to_string(s.settings_path()).unwrap(), user);
    }

    #[test]
    fn unreadable_settings_are_left_alone_and_missing_ones_are_created() {
        let tmp = tempfile::tempdir().unwrap();
        let claude = FakeClaude::default();
        let s = setup(tmp.path(), &claude);
        fs::create_dir_all(&s.claude_dir).unwrap();
        fs::write(s.settings_path(), "{ // comment\n}").unwrap();
        let steps = s.install(true);
        let rule = steps.iter().find(|x| x.name == "allow rule").unwrap();
        assert!(!rule.ok);
        assert_eq!(
            fs::read_to_string(s.settings_path()).unwrap(),
            "{ // comment\n}"
        );

        fs::remove_file(s.settings_path()).unwrap();
        assert!(s.install(true).iter().all(|x| x.ok));
        let v: Value =
            serde_json::from_str(&fs::read_to_string(s.settings_path()).unwrap()).unwrap();
        assert_eq!(v, json!({"permissions": {"allow": [ALLOW_RULE]}}));
    }

    #[test]
    fn no_allow_skips_the_settings_file() {
        let tmp = tempfile::tempdir().unwrap();
        let claude = FakeClaude::default();
        let s = setup(tmp.path(), &claude);
        assert!(s.install(false).iter().all(|x| x.ok));
        assert!(!s.settings_path().exists());
        let check = s.check();
        assert!(!check.iter().find(|x| x.name == "allow rule").unwrap().ok);
        let json_out: Value = serde_json::from_str(&render_steps(&check, true)).unwrap();
        assert_eq!(json_out["ok"], false);
        assert_eq!(json_out["steps"].as_array().unwrap().len(), 5);
    }

    #[test]
    fn json_names_reads_lists_and_maps() {
        assert_eq!(json_names(r#"[{"name":"acta"}]"#), ["acta"]);
        assert!(
            json_names(r#"[{"id":"acta@acta","scope":"user"}]"#).contains(&"acta@acta".to_string())
        );
        assert!(json_names(r#"{"acta":{"source":"x"}}"#).contains(&"acta".to_string()));
        assert!(json_names("not json").is_empty());
    }
}
