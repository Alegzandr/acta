//! `acta setup` (ADR 0005, section 1): the generated local plugin, its
//! registration through the `claude` CLI, and the allow rules of its two
//! servers (ADR 0005; ADR 0010 added the agent browser's).
//!
//! Idempotent: every step checks before it writes. Reversible: `--remove`
//! undoes only what setup wrote, using a record it keeps next to the plugin.
//! It never rewrites a settings file it cannot parse.

use acta_core::folder_path;
use acta_core::paths::Os;
use serde_json::{Map, Value, json};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const MARKETPLACE: &str = "acta";
pub const PLUGIN: &str = "acta";
pub const PLUGIN_ID: &str = "acta@acta";
pub const SERVER: &str = "acta";
/// The agent browser's server (ADR 0010, decision 7).
pub const BROWSER_SERVER: &str = "browser";
pub const ALLOW_RULE: &str = "mcp__plugin_acta_acta__*";
pub const BROWSER_ALLOW_RULE: &str = "mcp__plugin_acta_browser__*";
/// Every rule setup adds, in order.
pub const ALLOW_RULES: [&str; 2] = [ALLOW_RULE, BROWSER_ALLOW_RULE];
/// How a failure to start the `claude` CLI begins: the check reports
/// Claude Code as not found rather than as a setup step to do.
const NOT_RUN: &str = "cannot run claude";
/// How long one call to the `claude` CLI may take.
const CLAUDE_LIMIT: std::time::Duration = std::time::Duration::from_secs(60);
/// The notifications that mean a session is blocked on the human.
const WAITING_NOTIFICATIONS: &str =
    "permission_prompt|elicitation_dialog|elicitation_url_dialog|agent_needs_input";
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
        cmd.args(args);
        if let Some(d) = &self.config_dir {
            cmd.env("CLAUDE_CONFIG_DIR", d);
        }
        // A `claude` that hangs is killed rather than waited on forever.
        let out = acta_core::process::output_within(&mut cmd, CLAUDE_LIMIT)
            .map_err(|e| format!("{NOT_RUN} ({}): is Claude Code installed?", e.kind()))?
            .ok_or_else(|| format!("claude did not answer within {} s", CLAUDE_LIMIT.as_secs()))?;
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
        // Session presence (agent::presence): answered with empty text, so
        // nothing reaches the model; short timeouts, never blocking. No
        // `SessionEnd`: Claude Code refuses `mcp_tool` hooks there ("no MCP
        // client context", run on 2.1.330); the server ends the session
        // when its stdin closes.
        let presence = |event: &str, detail: Option<&str>| {
            let mut input = json!({"event": event, "session": "${session_id}"});
            if let Some(d) = detail {
                input["detail"] = json!(d);
            }
            json!({"type": "mcp_tool", "server": server, "tool": "hook",
                   "input": input, "timeout": 5})
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
                pretty(json!({"mcpServers": {
                    SERVER: {"command": exe, "args": ["mcp"]},
                    BROWSER_SERVER: {"command": exe, "args": ["mcp", "browser"]}
                }})),
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
                    "SubagentStart": [{"hooks": [mcp_hook("subagent")]}],
                    "UserPromptSubmit": [{"hooks": [presence("prompt", None)]}],
                    "PostToolUse": [{"hooks": [presence("tool", Some("${tool_name}"))]}],
                    "Notification": [{"matcher": WAITING_NOTIFICATIONS,
                                      "hooks": [presence("waiting", None)]}],
                    "Stop": [{"hooks": [presence("stop", None)]}]
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

    /// Writes the plugin tree where it differs (`setup --files-only`, and
    /// the first step of setup).
    pub fn plugin_files(&self) -> Step {
        match self.write_files() {
            Ok(0) => Step::new("plugin files", true, "up to date"),
            Ok(n) => Step::new("plugin files", true, format!("{n} written")),
            Err(e) => Step::new("plugin files", false, format!("cannot write: {}", e.kind())),
        }
    }

    /// Where Claude Code's `acta` marketplace points, from
    /// `claude plugin marketplace list --json`
    /// (`[{"name", "source", "path", "installLocation"}]`).
    fn marketplace_state(&self) -> Result<Reg, String> {
        let out = self
            .claude
            .run(&["plugin", "marketplace", "list", "--json"])?;
        let items = json_items(&out)?;
        let Some(m) = items.iter().find(|m| m["name"] == MARKETPLACE) else {
            return Ok(Reg::Absent);
        };
        let path = m["path"]
            .as_str()
            .or_else(|| m["installLocation"].as_str())
            .unwrap_or_default();
        Ok(if same_dir(path, &self.plugin_root) {
            Reg::Ours
        } else {
            Reg::Elsewhere
        })
    }

    /// Whether `acta@acta` is installed at user scope and loads from this
    /// plugin directory, from `claude plugin list --json`
    /// (`[{"id", "scope", "readFromFolder", ...}]`).
    fn plugin_state(&self) -> Result<Reg, String> {
        let out = self.claude.run(&["plugin", "list", "--json"])?;
        let items = json_items(&out)?;
        let Some(p) = items
            .iter()
            .find(|p| p["id"] == PLUGIN_ID && p["scope"].as_str().is_none_or(|s| s == "user"))
        else {
            return Ok(Reg::Absent);
        };
        let from = p["readFromFolder"].as_str().unwrap_or_default();
        Ok(if same_dir(from, &self.plugin_dir()) {
            Reg::Ours
        } else {
            Reg::Elsewhere
        })
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

    /// Runs every step. Each one is skipped when already done; a
    /// registration pointing at another folder is replaced.
    pub fn install_with(&self, allow: bool, statusline: bool) -> Vec<Step> {
        let mut steps = Vec::new();
        let mut record = self.record();
        let files = self.plugin_files();
        let wrote = files.ok;
        steps.push(files);
        if !wrote {
            return steps;
        }
        let root = self.plugin_root.to_string_lossy().into_owned();
        let add = |detail: &str| match self.claude.run(&["plugin", "marketplace", "add", &root]) {
            Ok(_) => Step::new("marketplace", true, detail),
            Err(e) => Step::new("marketplace", false, one_line(&e)),
        };
        let market = match self.marketplace_state() {
            Ok(Reg::Ours) => Step::new("marketplace", true, "already added"),
            Ok(Reg::Elsewhere) => {
                match self
                    .claude
                    .run(&["plugin", "marketplace", "remove", MARKETPLACE])
                {
                    Ok(_) => add("re-added here (it pointed to another folder)"),
                    Err(e) => Step::new("marketplace", false, one_line(&e)),
                }
            }
            _ => add("added"),
        };
        if market.ok {
            record["marketplace_added"] = json!(true);
        }
        steps.push(market);
        let install = |detail: &str| match self
            .claude
            .run(&["plugin", "install", PLUGIN_ID, "--scope", "user"])
        {
            Ok(_) => Step::new("plugin", true, detail),
            Err(e) => Step::new("plugin", false, one_line(&e)),
        };
        let plugin = match self.plugin_state() {
            Ok(Reg::Ours) => Step::new("plugin", true, "already installed"),
            Ok(Reg::Elsewhere) => {
                match self
                    .claude
                    .run(&["plugin", "uninstall", PLUGIN_ID, "--scope", "user"])
                {
                    Ok(_) => install("reinstalled from here (it loaded from another folder)"),
                    Err(e) => Step::new("plugin", false, one_line(&e)),
                }
            }
            _ => install("installed (user scope)"),
        };
        if plugin.ok {
            record["plugin_installed"] = json!(true);
        }
        steps.push(plugin);
        if allow {
            match edit_allow_rule(&self.settings_path(), true, &record) {
                Ok(Edit::Added(made, rules)) => {
                    for (k, v) in made {
                        // What an earlier setup created stays recorded.
                        if v || record[k].is_null() {
                            record[k] = json!(v);
                        }
                    }
                    let mut added = recorded_rules(&record);
                    for r in &rules {
                        if !added.contains(r) {
                            added.push(r.clone());
                        }
                    }
                    record["allow_rules_added"] = json!(added);
                    steps.push(Step::new(
                        "allow rule",
                        true,
                        format!(
                            "added {} to settings.json (a backup was kept)",
                            rules.join(" and ")
                        ),
                    ));
                }
                Ok(_) => steps.push(Step::new("allow rule", true, "already present")),
                Err(e) => steps.push(Step::new("allow rule", false, e)),
            }
        } else {
            steps.push(Step::new("allow rule", true, "skipped (--no-allow)"));
        }
        if statusline {
            let command = statusline_command(&self.exe);
            steps.push(
                match edit_status_line(&self.settings_path(), true, &command, &mut record) {
                    Ok(detail) => Step::new("status line", true, detail),
                    Err(e) => Step::new("status line", false, e),
                },
            );
        } else {
            record["statusline_opt_out"] = json!(true);
            steps.push(Step::new("status line", true, "skipped (--no-statusline)"));
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
        let files_ok = self.files_current();
        let market = self.marketplace_state();
        let found = !matches!(&market, Err(e) if e.starts_with(NOT_RUN));
        let mut steps = vec![
            Step::new(
                "claude",
                found,
                if found {
                    "found"
                } else {
                    "not found: install Claude Code"
                },
            ),
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
                files_ok,
                if files_ok {
                    "up to date"
                } else {
                    "missing or outdated"
                },
            ),
        ];
        let state = |r: Result<Reg, String>, yes: &str, no: &str| match r {
            Ok(Reg::Ours) => (true, yes.to_owned()),
            Ok(Reg::Elsewhere) => (false, "points to another folder: run acta setup".to_owned()),
            Ok(Reg::Absent) => (false, no.to_owned()),
            Err(e) => (false, one_line(&e)),
        };
        let (ok, d) = state(market, "added", "not added");
        steps.push(Step::new("marketplace", ok, d));
        let (ok, d) = state(self.plugin_state(), "installed", "not installed");
        steps.push(Step::new("plugin", ok, d));
        let allow = read_settings(&self.settings_path())
            .ok()
            .flatten()
            .is_some_and(|v| has_allow_rules(&v));
        steps.push(Step::new(
            "allow rule",
            allow,
            if allow {
                "present"
            } else {
                "absent: every Acta tool call will prompt"
            },
        ));
        let ours = statusline_command(&self.exe);
        let current = read_settings(&self.settings_path())
            .ok()
            .flatten()
            .and_then(|v| {
                v.pointer("/statusLine/command")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            });
        let record = self.record();
        let wrapped = record["statusline_original"].is_object();
        let opted_out = record["statusline_opt_out"] == json!(true);
        steps.push(match current {
            Some(c) if c == ours && wrapped => {
                Step::new("status line", true, "Acta's, after your own status line")
            }
            Some(c) if c == ours => Step::new("status line", true, "Acta's"),
            _ if opted_out => Step::new("status line", true, "skipped (--no-statusline)"),
            Some(c) if crate::statusline::is_acta_statusline(&c) => Step::new(
                "status line",
                false,
                "Acta's from another install: run acta setup",
            ),
            Some(_) => Step::new(
                "status line",
                false,
                "another status line: run acta setup to add plan usage after it",
            ),
            None => Step::new("status line", false, "absent: plan usage is not recorded"),
        });
        steps
    }

    /// Undoes what setup did: the plugin, the marketplace, the allow rule
    /// and the settings it created, its backup, and the plugin directory.
    pub fn remove(&self) -> Vec<Step> {
        let record = self.record();
        let mut steps = Vec::new();
        match self.plugin_state() {
            Ok(Reg::Absent) => steps.push(Step::new("plugin", true, "not installed")),
            _ => match self
                .claude
                .run(&["plugin", "uninstall", PLUGIN_ID, "--scope", "user"])
            {
                Ok(_) => steps.push(Step::new("plugin", true, "uninstalled")),
                Err(e) => steps.push(Step::new("plugin", false, one_line(&e))),
            },
        }
        match self.marketplace_state() {
            Ok(Reg::Absent) => steps.push(Step::new("marketplace", true, "not added")),
            _ => match self
                .claude
                .run(&["plugin", "marketplace", "remove", MARKETPLACE])
            {
                Ok(_) => steps.push(Step::new("marketplace", true, "removed")),
                Err(e) => steps.push(Step::new("marketplace", false, one_line(&e))),
            },
        }
        let acta_line = read_settings(&self.settings_path())
            .ok()
            .flatten()
            .and_then(|v| {
                v.pointer("/statusLine/command")
                    .and_then(Value::as_str)
                    .map(crate::statusline::is_acta_statusline)
            })
            .unwrap_or(false);
        if record["statusline_added"] == json!(true) || acta_line {
            let command = statusline_command(&self.exe);
            let mut r = record.clone();
            steps.push(
                match edit_status_line(&self.settings_path(), false, &command, &mut r) {
                    Ok(detail) => Step::new("status line", true, detail),
                    Err(e) => Step::new("status line", false, e),
                },
            );
        }
        if !recorded_rules(&record).is_empty() {
            match edit_allow_rule(&self.settings_path(), false, &record) {
                Ok(Edit::Removed) => steps.push(Step::new("allow rule", true, "removed")),
                Ok(_) => steps.push(Step::new("allow rule", true, "already absent")),
                Err(e) => steps.push(Step::new("allow rule", false, e)),
            }
        } else {
            steps.push(Step::new(
                "allow rule",
                true,
                "left as is (not added by setup)",
            ));
        }
        if record["backup_created"] == json!(true) {
            match fs::remove_file(backup_path(&self.settings_path())) {
                Ok(()) => steps.push(Step::new("backup", true, "settings backup removed")),
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => steps.push(Step::new(
                    "backup",
                    false,
                    format!("cannot remove the settings backup: {}", e.kind()),
                )),
            }
        }
        if steps.iter().all(|s| s.ok) {
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

/// What Claude Code's registration of the marketplace or the plugin is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reg {
    Absent,
    /// Registered, from this plugin directory.
    Ours,
    /// Registered under Acta's name, from another directory (an old data
    /// directory, or a development build).
    Elsewhere,
}

/// Whether a path printed by Claude Code is this directory.
fn same_dir(printed: &str, dir: &Path) -> bool {
    if printed.is_empty() {
        return false;
    }
    let form = |p: &Path| {
        folder_path::canonical(p)
            .unwrap_or_else(|_| folder_path::normalize(Os::CURRENT, &p.to_string_lossy()))
    };
    folder_path::same(Os::CURRENT, &form(Path::new(printed)), &form(dir))
}

fn one_line(text: &str) -> String {
    let t = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.chars().count() > 160 {
        format!("{}...", t.chars().take(157).collect::<String>())
    } else {
        t
    }
}

/// The top-level array of a `claude ... --json` listing.
fn json_items(text: &str) -> Result<Vec<Value>, String> {
    match serde_json::from_str::<Value>(text.trim()) {
        Ok(Value::Array(a)) => Ok(a),
        _ => Err("unexpected output from the claude CLI; update Claude Code".into()),
    }
}

pub enum Edit {
    /// Added, with what setup created to hold them (record keys) and the
    /// rules added.
    Added(Vec<(&'static str, bool)>, Vec<String>),
    Removed,
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

fn has_rule(v: &Value, rule: &str) -> bool {
    v.pointer("/permissions/allow")
        .and_then(Value::as_array)
        .is_some_and(|a| a.iter().any(|x| x == rule))
}

fn has_allow_rules(v: &Value) -> bool {
    ALLOW_RULES.iter().all(|r| has_rule(v, r))
}

/// The rules setup added, from its record: `allow_rules_added`, or the one
/// rule of a record written before the agent browser.
fn recorded_rules(record: &Value) -> Vec<String> {
    let mut out: Vec<String> = record["allow_rules_added"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| r.as_str())
        .filter(|r| ALLOW_RULES.contains(r))
        .map(str::to_owned)
        .collect();
    if record["allow_rule_added"] == json!(true) && !out.iter().any(|r| r == ALLOW_RULE) {
        out.insert(0, ALLOW_RULE.to_owned());
    }
    out
}

fn backup_path(settings: &Path) -> PathBuf {
    settings.with_extension("json.acta-backup")
}

/// Adds the missing allow rules, or removes the ones setup added, keeping
/// everything else as it was (key order included). Adding keeps a backup of the previous file once and
/// reports what it created; removing takes out an `allow` list, a
/// `permissions` object or a whole file that setup created and that are
/// empty again (`record` says which).
pub fn edit_allow_rule(path: &Path, add: bool, record: &Value) -> Result<Edit, String> {
    let existing = read_settings(path)?;
    let mut v = existing.clone().unwrap_or_else(|| json!({}));
    let missing: Vec<&str> = ALLOW_RULES
        .into_iter()
        .filter(|r| !has_rule(&v, r))
        .collect();
    let ours = recorded_rules(record);
    if add && missing.is_empty() || !add && !ours.iter().any(|r| has_rule(&v, r)) {
        return Ok(Edit::Unchanged);
    }
    let obj = v.as_object_mut().expect("object");
    let mut made = Vec::new();
    if add {
        made.push(("created_settings", existing.is_none()));
        made.push(("created_permissions", !obj.contains_key("permissions")));
        let perms = obj
            .entry("permissions")
            .or_insert_with(|| Value::Object(Map::new()));
        let Some(perms) = perms.as_object_mut() else {
            return Err("settings.json has a permissions value that is not an object".into());
        };
        made.push(("created_allow", !perms.contains_key("allow")));
        let allow = perms.entry("allow").or_insert_with(|| json!([]));
        let Some(allow) = allow.as_array_mut() else {
            return Err("settings.json has a permissions.allow value that is not a list".into());
        };
        for r in &missing {
            allow.push(json!(r));
        }
    } else {
        let created = |k: &str| record[k] == json!(true);
        if let Some(perms) = obj.get_mut("permissions").and_then(Value::as_object_mut) {
            if let Some(allow) = perms.get_mut("allow").and_then(Value::as_array_mut) {
                allow.retain(|x| !ours.iter().any(|r| x == r));
            }
            if created("created_allow")
                && perms
                    .get("allow")
                    .and_then(Value::as_array)
                    .is_some_and(Vec::is_empty)
            {
                perms.remove("allow");
            }
        }
        if created("created_permissions")
            && obj
                .get("permissions")
                .and_then(Value::as_object)
                .is_some_and(Map::is_empty)
        {
            obj.remove("permissions");
        }
        if created("created_settings") && obj.is_empty() {
            fs::remove_file(path)
                .map_err(|e| format!("cannot remove settings.json: {}", e.kind()))?;
            return Ok(Edit::Removed);
        }
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)
            .map_err(|e| format!("cannot create the settings folder: {}", e.kind()))?;
    }
    if add && existing.is_some() {
        let backup = backup_path(path);
        if !backup.exists() {
            fs::copy(path, &backup)
                .map_err(|e| format!("cannot back up settings.json: {}", e.kind()))?;
            made.push(("backup_created", true));
        }
    }
    let text = serde_json::to_string_pretty(&v).expect("json") + "\n";
    write_atomic(path, &text).map_err(|e| format!("cannot write settings.json: {}", e.kind()))?;
    Ok(if add {
        Edit::Added(made, missing.iter().map(|r| r.to_string()).collect())
    } else {
        Edit::Removed
    })
}

/// The `statusLine.command` setup writes: the `acta` executable with forward
/// slashes (Claude Code runs it through Git Bash or PowerShell on Windows,
/// `sh` elsewhere; code.claude.com/docs/en/statusline), unquoted so both
/// shells run it. A path either shell would split or interpret falls back
/// to `acta` on `PATH`, where ADR 0004 exports it.
pub fn statusline_command(exe: &Path) -> String {
    let path = exe.to_string_lossy().replace('\\', "/");
    let plain = path
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | ':' | '.' | '_' | '-'));
    if plain {
        format!("{path} statusline")
    } else {
        "acta statusline".to_owned()
    }
}

/// Puts Acta's status line in `statusLine` (`add`) or takes it out. A status
/// line the user had is kept whole in the record (`statusline_original`) and
/// run first by `acta statusline`, then put back exactly by `--remove`. A
/// `statusLine` the user changed after setup is never touched by removal.
pub fn edit_status_line(
    path: &Path,
    add: bool,
    command: &str,
    record: &mut Value,
) -> Result<&'static str, String> {
    let existing = read_settings(path)?;
    let mut v = existing.clone().unwrap_or_else(|| json!({}));
    let obj = v.as_object_mut().expect("object");
    let current = obj.get("statusLine").cloned();
    let current_command = current
        .as_ref()
        .and_then(|c| c.get("command"))
        .and_then(Value::as_str);
    // Any Acta status line is ours, wherever its `acta` lives (an older
    // install, a moved data folder): it is rewritten, never kept as the
    // user's own.
    let is_ours = current_command.is_some_and(crate::statusline::is_acta_statusline);
    let detail = if add {
        record["statusline_opt_out"] = Value::Null;
        if current_command == Some(command) {
            record["statusline_added"] = json!(true);
            return Ok("already present");
        }
        let mut line = match &current {
            Some(Value::Object(o)) => Value::Object(o.clone()),
            _ => json!({}),
        };
        line["type"] = json!("command");
        line["command"] = json!(command);
        obj.insert("statusLine".into(), line);
        if !is_ours {
            // The user's own status line (or none): kept whole.
            record["statusline_original"] = current.clone().unwrap_or(Value::Null);
        }
        record["statusline_added"] = json!(true);
        if record["created_settings"].is_null() {
            record["created_settings"] = json!(existing.is_none());
        }
        if is_ours {
            "updated to this Acta"
        } else if current.is_some() {
            "added after your own status line, which still shows first"
        } else {
            "added: plan usage is recorded"
        }
    } else {
        if !is_ours {
            record["statusline_added"] = json!(false);
            return Ok("left as is (changed since setup)");
        }
        match record["statusline_original"].clone() {
            Value::Null => {
                obj.remove("statusLine");
            }
            original => {
                obj.insert("statusLine".into(), original);
            }
        }
        record["statusline_added"] = json!(false);
        if record["created_settings"] == json!(true) && obj.is_empty() {
            fs::remove_file(path)
                .map_err(|e| format!("cannot remove settings.json: {}", e.kind()))?;
            return Ok("removed");
        }
        if record["statusline_original"].is_null() {
            "removed"
        } else {
            "your own status line is back"
        }
    };
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)
            .map_err(|e| format!("cannot create the settings folder: {}", e.kind()))?;
    }
    if add && existing.is_some() {
        let backup = backup_path(path);
        if !backup.exists() {
            fs::copy(path, &backup)
                .map_err(|e| format!("cannot back up settings.json: {}", e.kind()))?;
            record["backup_created"] = json!(true);
        }
    }
    let text = serde_json::to_string_pretty(&v).expect("json") + "\n";
    write_atomic(path, &text).map_err(|e| format!("cannot write settings.json: {}", e.kind()))?;
    Ok(detail)
}

/// Writes to a temporary file next to the real target, then renames it over
/// the target. A symlink is followed (its target is replaced and the link
/// stays) and the target's permissions are kept.
pub fn write_atomic(path: &Path, text: &str) -> io::Result<()> {
    let target = match fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_symlink() => fs::canonicalize(path)?,
        _ => path.to_path_buf(),
    };
    let permissions = fs::metadata(&target).ok().map(|m| m.permissions());
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = target.with_file_name(format!(".{name}.acta-tmp-{}", std::process::id()));
    let result = fs::write(&tmp, text)
        .and_then(|()| match permissions {
            Some(p) => fs::set_permissions(&tmp, p),
            None => Ok(()),
        })
        .and_then(|()| fs::rename(&tmp, &target));
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// A fake `claude` that keeps the marketplace and plugin state in memory
    /// and prints the listings in the shape the real CLI prints them
    /// (checked against Claude Code 2.1.330 with a temporary config dir).
    #[derive(Default)]
    struct FakeClaude {
        calls: RefCell<Vec<String>>,
        /// The marketplace's directory, when added.
        marketplace: RefCell<Option<PathBuf>>,
        /// The plugin's `readFromFolder`, when installed.
        plugin: RefCell<Option<PathBuf>>,
    }

    impl ClaudeCli for FakeClaude {
        fn run(&self, args: &[&str]) -> Result<String, String> {
            self.calls.borrow_mut().push(args.join(" "));
            match args {
                ["plugin", "marketplace", "list", "--json"] => {
                    let items: Vec<Value> = self
                        .marketplace
                        .borrow()
                        .iter()
                        .map(|p| {
                            json!({"name": MARKETPLACE, "source": "directory",
                                   "path": p, "installLocation": p})
                        })
                        .collect();
                    Ok(serde_json::to_string_pretty(&items).unwrap())
                }
                ["plugin", "list", "--json"] => {
                    let items: Vec<Value> = self
                        .plugin
                        .borrow()
                        .iter()
                        .map(|p| {
                            json!({"id": PLUGIN_ID, "version": "0.1.4", "scope": "user",
                                   "enabled": true, "readFromFolder": p})
                        })
                        .collect();
                    Ok(serde_json::to_string_pretty(&items).unwrap())
                }
                ["plugin", "marketplace", "add", path] => {
                    *self.marketplace.borrow_mut() = Some(PathBuf::from(path));
                    Ok(String::new())
                }
                ["plugin", "install", _, ..] => {
                    let m = self.marketplace.borrow().clone().ok_or("no marketplace")?;
                    *self.plugin.borrow_mut() = Some(m.join("plugins").join(PLUGIN));
                    Ok(String::new())
                }
                ["plugin", "uninstall", ..] => {
                    *self.plugin.borrow_mut() = None;
                    Ok(String::new())
                }
                ["plugin", "marketplace", "remove", _] => {
                    *self.marketplace.borrow_mut() = None;
                    Ok(String::new())
                }
                _ => Err(format!("unexpected: {}", args.join(" "))),
            }
        }
    }

    fn setup<'a>(dir: &Path, claude: &'a dyn ClaudeCli) -> Setup<'a> {
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

        let first = s.install_with(true, true);
        assert!(first.iter().all(|x| x.ok), "{first:?}");
        assert!(s.check().iter().all(|x| x.ok), "{:?}", s.check());
        let after = fs::read_to_string(s.settings_path()).unwrap();
        let v: Value = serde_json::from_str(&after).unwrap();
        assert_eq!(
            v["permissions"]["allow"],
            json!(["Bash(git status)", ALLOW_RULE, BROWSER_ALLOW_RULE])
        );
        // Key order and other keys are kept.
        let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["theme", "permissions", "model", "statusLine"]);
        let backup = backup_path(&s.settings_path());
        assert_eq!(fs::read_to_string(&backup).unwrap(), user);

        // Second run: nothing changes, no install command repeated.
        let calls_before = claude.calls.borrow().len();
        let second = s.install_with(true, true);
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
        assert!(!backup.exists(), "the backup setup made is removed");
        assert!(!s.plugin_root.exists());
        assert!(claude.plugin.borrow().is_none() && claude.marketplace.borrow().is_none());
    }

    #[test]
    fn a_registration_from_another_folder_is_replaced() {
        let tmp = tempfile::tempdir().unwrap();
        let claude = FakeClaude::default();
        let s = setup(tmp.path(), &claude);
        let old = tmp.path().join("old-data").join("claude");
        fs::create_dir_all(&old).unwrap();
        *claude.marketplace.borrow_mut() = Some(old.clone());
        *claude.plugin.borrow_mut() = Some(old.join("plugins").join(PLUGIN));
        let check = s.check();
        let market = check.iter().find(|x| x.name == "marketplace").unwrap();
        assert!(
            !market.ok && market.detail.contains("another folder"),
            "{market:?}"
        );

        let steps = s.install_with(false, true);
        assert!(steps.iter().all(|x| x.ok), "{steps:?}");
        assert!(steps.iter().any(|x| x.detail.starts_with("re-added here")));
        assert!(
            steps
                .iter()
                .any(|x| x.detail.starts_with("reinstalled from here"))
        );
        assert!(same_dir(
            &claude
                .marketplace
                .borrow()
                .clone()
                .unwrap()
                .to_string_lossy(),
            &s.plugin_root
        ));
        assert!(s.check().iter().all(|x| x.ok || x.name == "allow rule"));
    }

    #[test]
    fn a_rule_the_user_had_is_never_removed() {
        let tmp = tempfile::tempdir().unwrap();
        let claude = FakeClaude::default();
        let s = setup(tmp.path(), &claude);
        fs::create_dir_all(&s.claude_dir).unwrap();
        let user = format!("{{\"permissions\":{{\"allow\":[\"{ALLOW_RULE}\"]}}}}");
        fs::write(s.settings_path(), &user).unwrap();
        s.install_with(true, true);
        let v: Value =
            serde_json::from_str(&fs::read_to_string(s.settings_path()).unwrap()).unwrap();
        assert_eq!(v["permissions"]["allow"], json!(ALLOW_RULES));
        s.remove();
        let v: Value =
            serde_json::from_str(&fs::read_to_string(s.settings_path()).unwrap()).unwrap();
        assert_eq!(v, serde_json::from_str::<Value>(&user).unwrap());
    }

    #[test]
    fn a_setup_from_before_the_agent_browser_adds_its_rule_and_removes_both() {
        let tmp = tempfile::tempdir().unwrap();
        let claude = FakeClaude::default();
        let s = setup(tmp.path(), &claude);
        fs::create_dir_all(&s.claude_dir).unwrap();
        fs::write(
            s.settings_path(),
            format!("{{\"permissions\":{{\"allow\":[\"{ALLOW_RULE}\"]}}}}"),
        )
        .unwrap();
        fs::create_dir_all(&s.plugin_root).unwrap();
        fs::write(
            s.plugin_root.join(RECORD),
            r#"{"allow_rule_added": true, "created_settings": true, "created_permissions": true, "created_allow": true}"#,
        )
        .unwrap();
        let steps = s.install_with(true, true);
        let rule = steps.iter().find(|x| x.name == "allow rule").unwrap();
        assert!(
            rule.detail.contains(BROWSER_ALLOW_RULE) && !rule.detail.contains(ALLOW_RULE),
            "{rule:?}"
        );
        assert!(s.check().iter().all(|x| x.ok), "{:?}", s.check());
        let mcp: Value =
            serde_json::from_str(&fs::read_to_string(s.plugin_dir().join(".mcp.json")).unwrap())
                .unwrap();
        assert_eq!(
            mcp["mcpServers"]["browser"]["args"],
            json!(["mcp", "browser"])
        );
        assert!(s.remove().iter().all(|x| x.ok));
        assert!(
            !s.settings_path().exists(),
            "both rules and the file setup made are gone"
        );
    }

    #[test]
    fn what_setup_created_is_removed_and_unreadable_settings_are_left_alone() {
        let tmp = tempfile::tempdir().unwrap();
        let claude = FakeClaude::default();
        let s = setup(tmp.path(), &claude);
        fs::create_dir_all(&s.claude_dir).unwrap();
        fs::write(s.settings_path(), "{ // comment\n}").unwrap();
        let steps = s.install_with(true, true);
        let rule = steps.iter().find(|x| x.name == "allow rule").unwrap();
        assert!(!rule.ok);
        assert_eq!(
            fs::read_to_string(s.settings_path()).unwrap(),
            "{ // comment\n}"
        );

        // A settings file setup creates is removed again.
        fs::remove_file(s.settings_path()).unwrap();
        assert!(s.install_with(true, false).iter().all(|x| x.ok));
        let v: Value =
            serde_json::from_str(&fs::read_to_string(s.settings_path()).unwrap()).unwrap();
        assert_eq!(v, json!({"permissions": {"allow": ALLOW_RULES}}));
        assert!(s.remove().iter().all(|x| x.ok));
        assert!(!s.settings_path().exists());

        // An empty permissions object setup created is removed; the user's
        // other keys stay.
        fs::write(s.settings_path(), "{\"theme\": \"dark\"}").unwrap();
        assert!(s.install_with(true, false).iter().all(|x| x.ok));
        assert!(s.remove().iter().all(|x| x.ok));
        let v: Value =
            serde_json::from_str(&fs::read_to_string(s.settings_path()).unwrap()).unwrap();
        assert_eq!(v, json!({"theme": "dark"}));
    }

    #[test]
    fn no_allow_skips_the_settings_file() {
        let tmp = tempfile::tempdir().unwrap();
        let claude = FakeClaude::default();
        let s = setup(tmp.path(), &claude);
        assert!(s.install_with(false, false).iter().all(|x| x.ok));
        assert!(!s.settings_path().exists());
        let check = s.check();
        assert!(!check.iter().find(|x| x.name == "allow rule").unwrap().ok);
        // The status line opt-out is recorded: skipped, not missing.
        assert!(check.iter().find(|x| x.name == "status line").unwrap().ok);
        let json_out: Value = serde_json::from_str(&render_steps(&check, true)).unwrap();
        assert_eq!(json_out["ok"], false);
        assert_eq!(json_out["steps"].as_array().unwrap().len(), 7);
    }

    #[test]
    fn a_users_status_line_is_wrapped_and_restored_exactly() {
        let tmp = tempfile::tempdir().unwrap();
        let claude = FakeClaude::default();
        let s = setup(tmp.path(), &claude);
        let user = r#"{
  "statusLine": {
    "type": "command",
    "command": "~/.claude/alexandre-line.sh",
    "padding": 2,
    "refreshInterval": 10
  }
}
"#;
        fs::create_dir_all(&s.claude_dir).unwrap();
        fs::write(s.settings_path(), user).unwrap();
        let steps = s.install_with(false, true);
        assert!(steps.iter().all(|x| x.ok), "{steps:?}");
        let v: Value =
            serde_json::from_str(&fs::read_to_string(s.settings_path()).unwrap()).unwrap();
        let ours = statusline_command(&s.exe);
        // Acta's command in the user's object: padding and refresh kept.
        assert_eq!(v["statusLine"]["command"], json!(ours));
        assert_eq!(v["statusLine"]["padding"], json!(2));
        assert_eq!(v["statusLine"]["refreshInterval"], json!(10));
        // The original is in the record, for `acta statusline` to run first.
        let record = s.record();
        assert_eq!(
            record["statusline_original"]["command"],
            "~/.claude/alexandre-line.sh"
        );
        let wrapped = s.check();
        let line = wrapped.iter().find(|x| x.name == "status line").unwrap();
        assert!(
            line.ok && line.detail.contains("after your own"),
            "{line:?}"
        );
        // Idempotent.
        let again = s.install_with(false, true);
        assert_eq!(
            again
                .iter()
                .find(|x| x.name == "status line")
                .unwrap()
                .detail,
            "already present"
        );
        // Remove puts the user's object back, exactly.
        let removed = s.remove();
        assert!(removed.iter().all(|x| x.ok), "{removed:?}");
        let back: Value =
            serde_json::from_str(&fs::read_to_string(s.settings_path()).unwrap()).unwrap();
        assert_eq!(back, serde_json::from_str::<Value>(user).unwrap());
    }

    #[test]
    fn an_acta_status_line_from_another_path_is_rewritten_never_kept_as_the_users() {
        let tmp = tempfile::tempdir().unwrap();
        let claude = FakeClaude::default();
        let s = setup(tmp.path(), &claude);
        let user =
            json!({"statusLine": {"type": "command", "command": "~/.claude/alexandre-line.sh"}});
        fs::create_dir_all(&s.claude_dir).unwrap();
        fs::write(s.settings_path(), user.to_string()).unwrap();
        assert!(s.install_with(false, true).iter().all(|x| x.ok));
        // Another install's Acta command replaces ours in settings (an older
        // path), then setup runs again from here.
        let mut v: Value =
            serde_json::from_str(&fs::read_to_string(s.settings_path()).unwrap()).unwrap();
        v["statusLine"]["command"] = json!("C:/Old/Acta/bin/acta.exe statusline");
        fs::write(s.settings_path(), v.to_string()).unwrap();
        let line = s
            .check()
            .into_iter()
            .find(|x| x.name == "status line")
            .unwrap();
        assert!(
            !line.ok && line.detail.contains("another install"),
            "{line:?}"
        );
        let again = s.install_with(false, true);
        let line = again.iter().find(|x| x.name == "status line").unwrap();
        assert_eq!(line.detail, "updated to this Acta");
        // The user's original is still the recorded one, never an Acta command.
        assert_eq!(
            s.record()["statusline_original"]["command"],
            "~/.claude/alexandre-line.sh"
        );
        assert!(s.remove().iter().all(|x| x.ok));
        let back: Value =
            serde_json::from_str(&fs::read_to_string(s.settings_path()).unwrap()).unwrap();
        assert_eq!(back, user);
    }

    #[test]
    fn the_status_line_opt_out_is_recorded_and_checked_as_done() {
        let tmp = tempfile::tempdir().unwrap();
        let claude = FakeClaude::default();
        let s = setup(tmp.path(), &claude);
        assert!(s.install_with(true, false).iter().all(|x| x.ok));
        let line = s
            .check()
            .into_iter()
            .find(|x| x.name == "status line")
            .unwrap();
        assert!(line.ok && line.detail.contains("skipped"), "{line:?}");
        // Setup without the flag clears the opt-out and adds it.
        assert!(s.install_with(true, true).iter().all(|x| x.ok));
        assert_eq!(s.record()["statusline_opt_out"], Value::Null);
        let line = s
            .check()
            .into_iter()
            .find(|x| x.name == "status line")
            .unwrap();
        assert_eq!(line.detail, "Acta's");
    }

    #[test]
    fn a_status_line_changed_after_setup_is_left_alone_and_a_created_file_goes() {
        let tmp = tempfile::tempdir().unwrap();
        let claude = FakeClaude::default();
        let s = setup(tmp.path(), &claude);
        // No settings file: setup creates it for the status line alone.
        assert!(s.install_with(false, true).iter().all(|x| x.ok));
        assert!(s.settings_path().exists());
        assert!(s.remove().iter().all(|x| x.ok));
        assert!(
            !s.settings_path().exists(),
            "the file setup created is gone"
        );

        assert!(s.install_with(false, true).iter().all(|x| x.ok));
        let mine = json!({"statusLine": {"type": "command", "command": "my-line"}});
        fs::write(s.settings_path(), mine.to_string()).unwrap();
        let removed = s.remove();
        let line = removed.iter().find(|x| x.name == "status line").unwrap();
        assert!(line.ok && line.detail.contains("left as is"), "{line:?}");
        let v: Value =
            serde_json::from_str(&fs::read_to_string(s.settings_path()).unwrap()).unwrap();
        assert_eq!(v, mine);
    }

    #[test]
    fn the_status_line_command_runs_in_bash_and_powershell() {
        assert_eq!(
            statusline_command(Path::new(
                "C:\\Users\\alexandre\\AppData\\Local\\Acta\\bin\\acta.exe"
            )),
            "C:/Users/alexandre/AppData/Local/Acta/bin/acta.exe statusline"
        );
        assert_eq!(
            statusline_command(Path::new("C:\\Users\\Alexandre B\\acta.exe")),
            "acta statusline"
        );
        assert_eq!(
            statusline_command(Path::new("/home/alexandre/.local/bin/acta")),
            "/home/alexandre/.local/bin/acta statusline"
        );
    }

    #[test]
    fn a_missing_claude_command_is_reported_as_not_found() {
        struct NoClaude;
        impl ClaudeCli for NoClaude {
            fn run(&self, _: &[&str]) -> Result<String, String> {
                Err(format!("{NOT_RUN} (NotFound): is Claude Code installed?"))
            }
        }
        let tmp = tempfile::tempdir().unwrap();
        let s = setup(tmp.path(), &NoClaude);
        let check = s.check();
        assert_eq!(check[0].name, "claude");
        assert!(!check[0].ok);
        let fake = FakeClaude::default();
        assert!(setup(tmp.path(), &fake).check()[0].ok);
    }

    #[test]
    fn listings_must_be_arrays() {
        assert_eq!(json_items(r#"[{"name":"acta"}]"#).unwrap().len(), 1);
        assert!(json_items(r#"{"acta":{}}"#).is_err());
        assert!(json_items("not json").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn write_atomic_follows_symlinks_and_keeps_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let real = tmp.path().join("real.json");
        fs::write(&real, "{}").unwrap();
        fs::set_permissions(&real, fs::Permissions::from_mode(0o600)).unwrap();
        let link = tmp.path().join("settings.json");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        write_atomic(&link, "{\"a\":1}").unwrap();
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read_to_string(&real).unwrap(), "{\"a\":1}");
        assert_eq!(
            fs::metadata(&real).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[test]
    fn write_atomic_replaces_a_plain_file() {
        let tmp = tempfile::tempdir().unwrap();
        let f = tmp.path().join("settings.json");
        fs::write(&f, "old").unwrap();
        write_atomic(&f, "new").unwrap();
        assert_eq!(fs::read_to_string(&f).unwrap(), "new");
        assert_eq!(fs::read_dir(tmp.path()).unwrap().count(), 1);
    }
}
