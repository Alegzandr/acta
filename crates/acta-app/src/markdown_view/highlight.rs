//! Code block highlighting in the classic Monokai roles (docs/design/
//! components-editor.md 5, Code; the user's choice of 2026-11-25):
//! syntect's parser over two-face's syntax set, each scope mapped to a
//! Monokai role the way the Monokai theme does; `diff` and `patch` colour
//! whole lines. Pure logic: the view maps roles to `syntax.monokai.*`.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::sync::mpsc::{SyncSender, sync_channel};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use syntect::parsing::{ParseState, Scope, ScopeStack, SyntaxDefinition, SyntaxSet};

/// A highlighted run's role. Default text (`syntax.monokai.fg`) has no run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// Keywords, operators, storage, tags.
    Keyword,
    String,
    /// Numbers, language constants, characters, escapes.
    Constant,
    /// Names being defined (functions, classes, types), attribute names.
    Function,
    /// Storage types, support (built-in) types, functions and constants.
    Type,
    Parameter,
    Comment,
    /// A `diff` line starting `+`.
    Added,
    /// A `diff` line starting `-`.
    Removed,
    /// A `diff` hunk header `@@`.
    Hunk,
}

impl Role {
    /// Whether classic Monokai sets the role in italic.
    pub fn italic(self) -> bool {
        matches!(self, Role::Type | Role::Parameter)
    }
}

/// A block over either limit draws unhighlighted.
const MAX_LINES: usize = 2_000;
const MAX_BYTES: usize = 200_000;
/// A block with a line longer than this (minified code, a data blob)
/// draws plain: the parser's backtracking on one long line is unbounded.
const MAX_LINE: usize = 8 * 1024;
/// A block the parser has not finished within this time draws plain. A
/// debug build parses about ten times slower (the dependencies are not
/// optimised), so it waits longer.
const BUDGET: Duration = Duration::from_millis(if cfg!(debug_assertions) { 3_000 } else { 300 });
/// Blocks waiting for the worker; more are asked for again on a later frame.
const QUEUE: usize = 64;
/// What the cache of runs may hold before the least recently drawn go.
const CACHE_BYTES: usize = 8 * 1024 * 1024;

static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();

/// The runs of a block, highlighted once on the worker.
pub type Runs = Arc<Vec<(Range<usize>, Role)>>;

struct Job {
    key: u64,
    code: String,
    lang: String,
}

/// Results by block: `None` while queued, then the runs (empty when the
/// block is too large, the language unknown, the budget ran out or the
/// parser failed), with when each was last asked for.
#[derive(Default)]
struct Cache {
    entries: HashMap<u64, (Option<Runs>, u64)>,
    bytes: usize,
    tick: u64,
}

impl Cache {
    fn size(runs: &Runs) -> usize {
        64 + runs.len() * std::mem::size_of::<(Range<usize>, Role)>()
    }

    /// Stores `runs`, then drops the least recently asked-for results
    /// until the cache fits; blocks still queued are never dropped.
    fn insert(&mut self, key: u64, runs: Runs, limit: usize) {
        self.tick += 1;
        self.bytes += Self::size(&runs);
        if let Some((Some(old), _)) = self.entries.insert(key, (Some(runs), self.tick)) {
            self.bytes -= Self::size(&old);
        }
        while self.bytes > limit {
            let oldest = self
                .entries
                .iter()
                .filter(|(k, (runs, _))| runs.is_some() && **k != key)
                .min_by_key(|(_, (_, used))| *used)
                .map(|(k, _)| *k);
            let Some(oldest) = oldest else { break };
            if let Some((Some(runs), _)) = self.entries.remove(&oldest) {
                self.bytes -= Self::size(&runs);
            }
        }
    }

    /// The entry of `key`, marked as just used.
    fn touch(&mut self, key: u64) -> Option<Option<Runs>> {
        self.tick += 1;
        let tick = self.tick;
        self.entries.get_mut(&key).map(|(runs, used)| {
            *used = tick;
            runs.clone()
        })
    }
}

static CACHE: Mutex<Option<Cache>> = Mutex::new(None);
static WORKER: OnceLock<SyncSender<Job>> = OnceLock::new();
static NOTIFY: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

/// What the worker calls when a block's runs land, so the window draws
/// them without waiting for input. Set once by the view.
pub fn set_notifier(notify: impl Fn() + Send + Sync + 'static) {
    let _ = NOTIFY.set(Box::new(notify));
}

fn with_cache<R>(f: impl FnOnce(&mut Cache) -> R) -> R {
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    f(cache.get_or_insert_with(Cache::default))
}

/// The one worker, with a bounded queue. fancy-regex backtracks
/// recursively, which can outgrow a default stack (the UI thread's is 1 MB
/// on Windows), so highlighting never runs on the UI thread and the worker
/// gets a large stack. A panic in the parser leaves an empty result.
fn worker() -> Option<&'static SyncSender<Job>> {
    if let Some(sender) = WORKER.get() {
        return Some(sender);
    }
    let (sender, jobs) = sync_channel::<Job>(QUEUE);
    std::thread::Builder::new()
        .name("acta-highlight".into())
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            while let Ok(job) = jobs.recv() {
                let runs = std::panic::catch_unwind(|| {
                    SYNTAXES.get_or_init(load);
                    highlight_within(&job.code, &job.lang, Some(Instant::now() + BUDGET))
                })
                .unwrap_or_default();
                with_cache(|cache| cache.insert(job.key, Arc::new(runs), CACHE_BYTES));
                if let Some(notify) = NOTIFY.get() {
                    notify();
                }
            }
        })
        .ok()?;
    let _ = WORKER.set(sender);
    WORKER.get()
}

/// The runs of `code` in `lang`, or `None` while the worker is on them
/// (the caller draws plain text; the notifier says when they land).
pub fn runs(code: &str, lang: &str) -> Option<Runs> {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (lang, code).hash(&mut hasher);
    let key = hasher.finish();
    if let Some(found) = with_cache(|cache| cache.touch(key)) {
        return found;
    }
    let job = Job {
        key,
        code: code.to_string(),
        lang: lang.to_string(),
    };
    // A full queue leaves the block plain until a later frame asks again.
    if worker().is_some_and(|w| w.try_send(job).is_ok()) {
        with_cache(|cache| {
            let tick = cache.tick;
            cache.entries.entry(key).or_insert((None, tick));
        });
    }
    None
}

/// PowerShell, which two-face's set lacks: the SublimeText/PowerShell
/// package's syntax (MIT, assets/syntaxes/powershell), precompiled into a
/// syntect dump by scripts/syntax-dump so no YAML loader ships.
const POWERSHELL: &[u8] =
    include_bytes!("../../../../assets/syntaxes/powershell/PowerShell.syntax.bin");

/// two-face's syntax set with PowerShell added. Linking the set again
/// takes a moment: it runs on the worker, before its first block.
fn load() -> SyntaxSet {
    let set = two_face::syntax::extra_newlines();
    let Ok(powershell) = syntect::dumps::from_reader::<SyntaxDefinition, _>(POWERSHELL) else {
        return set;
    };
    let mut builder = set.into_builder();
    builder.add(powershell);
    builder.build()
}

/// The language a fence's info string names: its first word, lower case.
/// `None` for no language and for the ones drawn plain on purpose.
pub fn language(info: Option<&str>) -> Option<String> {
    let word = info?.split_whitespace().next()?.to_ascii_lowercase();
    match word.as_str() {
        "" | "text" | "plaintext" | "plain" | "txt" | "mermaid" | "math" => None,
        _ => Some(word),
    }
}

/// The runs of `code` (already tab-expanded) for a block of language
/// `lang` (see [`language`]). Empty when the block is too large, the
/// language unknown, or the syntax set still loading.
#[cfg(test)]
pub fn highlight(code: &str, lang: &str) -> Vec<(Range<usize>, Role)> {
    highlight_within(code, lang, None)
}

/// [`highlight`], giving up (empty) once `deadline` passes.
fn highlight_within(
    code: &str,
    lang: &str,
    deadline: Option<Instant>,
) -> Vec<(Range<usize>, Role)> {
    if code.len() > MAX_BYTES
        || code.lines().count() > MAX_LINES
        || code.lines().any(|line| line.len() > MAX_LINE)
    {
        return Vec::new();
    }
    if lang == "diff" || lang == "patch" {
        return diff_lines(code);
    }
    let Some(syntaxes) = SYNTAXES.get() else {
        return Vec::new();
    };
    let by_name = || {
        syntaxes
            .syntaxes()
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(lang))
    };
    let Some(syntax) = syntaxes.find_syntax_by_token(lang).or_else(by_name) else {
        return Vec::new();
    };
    let roles = RoleScopes::get();
    let mut state = ParseState::new(syntax);
    let mut stack = ScopeStack::new();
    let mut runs: Vec<(Range<usize>, Role)> = Vec::new();
    let mut offset = 0;
    for line in code.split_inclusive('\n') {
        if deadline.is_some_and(|d| Instant::now() > d) {
            return Vec::new();
        }
        let Ok(ops) = state.parse_line(line, syntaxes) else {
            return Vec::new();
        };
        let mut at = 0;
        for (pos, op) in ops {
            push(&mut runs, offset + at..offset + pos, roles.role(&stack));
            at = pos;
            if stack.apply(&op).is_err() {
                return Vec::new();
            }
        }
        push(
            &mut runs,
            offset + at..offset + line.len(),
            roles.role(&stack),
        );
        offset += line.len();
    }
    runs
}

fn push(runs: &mut Vec<(Range<usize>, Role)>, range: Range<usize>, role: Option<Role>) {
    let Some(role) = role else { return };
    if range.is_empty() {
        return;
    }
    if let Some(last) = runs.last_mut()
        && last.1 == role
        && last.0.end == range.start
    {
        last.0.end = range.end;
        return;
    }
    runs.push((range, role));
}

/// `diff` and `patch`: whole lines, no tints.
fn diff_lines(code: &str) -> Vec<(Range<usize>, Role)> {
    let mut runs = Vec::new();
    let mut offset = 0;
    for line in code.split_inclusive('\n') {
        let text = line.trim_end_matches(['\n', '\r']);
        let role = if text.starts_with("@@") {
            Some(Role::Hunk)
        } else if text.starts_with("+++") || text.starts_with("---") {
            None
        } else if text.starts_with('+') {
            Some(Role::Added)
        } else if text.starts_with('-') {
            Some(Role::Removed)
        } else {
            None
        };
        push(&mut runs, offset..offset + text.len(), role);
        offset += line.len();
    }
    runs
}

/// The scope prefixes of the roles, the most specific first: the
/// innermost scope that matches one decides, as in the Monokai theme.
struct RoleScopes(Vec<(Scope, Role)>);

impl RoleScopes {
    fn get() -> &'static RoleScopes {
        static ROLES: OnceLock<RoleScopes> = OnceLock::new();
        ROLES.get_or_init(|| {
            let table = [
                ("comment", Role::Comment),
                ("punctuation.definition.comment", Role::Comment),
                ("constant.character.escape", Role::Constant),
                ("string", Role::String),
                ("constant", Role::Constant),
                ("variable.parameter", Role::Parameter),
                ("entity.name.tag", Role::Keyword),
                ("entity.name", Role::Function),
                ("entity.other.inherited-class", Role::Function),
                ("entity.other.attribute-name", Role::Function),
                ("storage.type", Role::Type),
                ("support", Role::Type),
                ("keyword", Role::Keyword),
                ("storage", Role::Keyword),
            ];
            RoleScopes(
                table
                    .into_iter()
                    .map(|(name, role)| (Scope::new(name).expect("a valid scope"), role))
                    .collect(),
            )
        })
    }

    fn role(&self, stack: &ScopeStack) -> Option<Role> {
        stack.as_slice().iter().rev().find_map(|scope| {
            self.0
                .iter()
                .find(|(prefix, _)| prefix.is_prefix_of(*scope))
                .map(|(_, role)| *role)
        })
    }
}

/// Tabs to spaces at 4-column stops (components-editor.md 5, Code).
pub fn expand_tabs(code: &str) -> String {
    if !code.contains('\t') {
        return code.to_string();
    }
    let mut out = String::with_capacity(code.len() + 16);
    let mut column = 0;
    for c in code.chars() {
        match c {
            '\t' => {
                let n = 4 - column % 4;
                out.extend(std::iter::repeat_n(' ', n));
                column += n;
            }
            '\n' => {
                out.push(c);
                column = 0;
            }
            _ => {
                out.push(c);
                column += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load() {
        SYNTAXES.get_or_init(super::load);
    }

    /// The licences of two-face's syntaxes ship with Acta: the notice in
    /// scripts/licenses/third-party is what two-face lists for them
    /// (`ACTA_BLESS=1` writes it again).
    #[test]
    fn the_syntax_licences_ship() {
        let mut want = String::from(
            "# Syntax definitions\n\nThe syntax definitions Acta highlights code with come from \
             two-face's syntax set (two-face 0.4.5), which lists their licences as follows. \
             Generated by `markdown_view::highlight` tests; PowerShell's own licence is in \
             assets/syntaxes/powershell.\n\n",
        );
        for licence in two_face::acknowledgement::listing().for_syntaxes() {
            if licence.needs_acknowledgement() {
                licence.write_md(&mut want);
            }
        }
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../scripts/licenses/third-party/two-face-syntaxes.md");
        if std::env::var_os("ACTA_BLESS").is_some() {
            std::fs::write(&path, &want).expect("written");
        }
        let have = std::fs::read_to_string(&path)
            .unwrap_or_default()
            .replace("\r\n", "\n");
        assert!(
            have == want,
            "{} is stale (ACTA_BLESS=1 rewrites it)",
            path.display()
        );
    }

    #[test]
    fn a_past_deadline_draws_plain() {
        load();
        let code = "let a = 1;\n";
        assert!(!highlight_within(code, "rust", None).is_empty());
        let past = Instant::now() - Duration::from_secs(1);
        assert!(highlight_within(code, "rust", Some(past)).is_empty());
    }

    #[test]
    fn the_cache_drops_the_least_recently_drawn() {
        let runs = |n: usize| Arc::new(vec![(0..1, Role::Keyword); n]);
        let mut cache = Cache::default();
        let one = Cache::size(&runs(10));
        cache.insert(1, runs(10), one * 2);
        cache.insert(2, runs(10), one * 2);
        cache.touch(1);
        cache.insert(3, runs(10), one * 2);
        assert!(cache.entries.contains_key(&1), "recently drawn, kept");
        assert!(!cache.entries.contains_key(&2), "least recent, dropped");
        assert!(cache.entries.contains_key(&3));
        assert_eq!(cache.bytes, one * 2);
        // A queued block is never dropped.
        cache.entries.insert(4, (None, 0));
        cache.insert(5, runs(10), one);
        assert!(cache.entries.contains_key(&4));
    }

    #[test]
    fn the_worker_highlights_off_the_calling_thread() {
        let code = "fn worker_test() {}\n";
        let mut got = None;
        for _ in 0..200 {
            if let Some(runs) = runs(code, "rust") {
                got = Some(runs);
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(got.is_some_and(|r| !r.is_empty()));
    }

    fn roles_of<'a>(code: &'a str, lang: &str) -> Vec<(&'a str, Role)> {
        highlight(code, lang)
            .into_iter()
            .map(|(r, role)| (&code[r], role))
            .collect()
    }

    #[test]
    fn rust_in_monokai_roles() {
        load();
        let code = "// Alexandre's store\npub fn open(n: u32) -> bool {\n    let s = \"a\\n\";\n    n > 42 && true\n}\n";
        let runs = roles_of(code, "rust");
        let has = |text: &str, role: Role| runs.iter().any(|(t, r)| t.contains(text) && *r == role);
        assert!(has("// Alexandre's store", Role::Comment), "{runs:?}");
        assert!(has("pub", Role::Keyword), "{runs:?}");
        // `fn` and `let` are storage types in the Rust syntax: cyan and
        // italic, as Monokai draws them.
        assert!(has("let", Role::Type), "{runs:?}");
        assert!(has("open", Role::Function), "{runs:?}");
        assert!(runs.contains(&("n", Role::Parameter)), "{runs:?}");
        assert!(has("u32", Role::Type), "{runs:?}");
        assert!(has("42", Role::Constant), "{runs:?}");
        assert!(has("true", Role::Constant), "{runs:?}");
        assert!(has("\"a", Role::String), "{runs:?}");
        // Operators are keywords in Monokai.
        assert!(has("&&", Role::Keyword), "{runs:?}");
        assert!(Role::Type.italic() && Role::Parameter.italic() && !Role::Keyword.italic());
    }

    #[test]
    fn common_languages_are_known() {
        load();
        for (lang, code) in [
            ("ts", "const x: number = 1;"),
            ("json", "{\"a\": 1}"),
            ("toml", "a = \"b\""),
            ("yaml", "a: 'b'"),
            ("python", "def f(): return None"),
            ("sh", "echo \"hi\""),
            ("html", "<a href=\"x\">y</a>"),
            ("go", "func main() { return }"),
            ("sql", "SELECT 1"),
            (
                "powershell",
                "$name = \"Alexandre\"; Write-Host $name # greet",
            ),
            (
                "ps1",
                "Get-ChildItem -Path . | Where-Object { $_.Length -gt 0 }",
            ),
        ] {
            assert!(!highlight(code, lang).is_empty(), "{lang}");
        }
        assert!(highlight("anything", "no-such-language").is_empty());
        let code = "$name = \"Alexandre\" # greet\n";
        let runs = roles_of(code, "powershell");
        assert!(runs.contains(&("\"Alexandre\"", Role::String)), "{runs:?}");
        assert!(
            runs.iter()
                .any(|(t, r)| t.starts_with("# greet") && *r == Role::Comment),
            "{runs:?}"
        );
    }

    #[test]
    fn diff_colours_whole_lines() {
        let code = "--- a/x\n+++ b/x\n@@ -1 +1 @@\n-old\n+new\n same\n";
        assert_eq!(
            roles_of(code, "diff"),
            [
                ("@@ -1 +1 @@", Role::Hunk),
                ("-old", Role::Removed),
                ("+new", Role::Added)
            ]
        );
        assert_eq!(roles_of("+x\n", "patch"), [("+x", Role::Added)]);
    }

    #[test]
    fn plain_languages_and_large_blocks_stay_plain() {
        assert_eq!(language(Some("Rust ignore")), Some("rust".into()));
        for plain in [
            None,
            Some(""),
            Some("text"),
            Some("mermaid"),
            Some("math"),
            Some("plaintext"),
        ] {
            assert_eq!(language(plain), None, "{plain:?}");
        }
        load();
        let big = "let a = 1;\n".repeat(MAX_LINES + 1);
        assert!(highlight(&big, "rust").is_empty());
        let long = format!("let a = \"{}\";\n", "x".repeat(MAX_LINE));
        assert!(highlight(&long, "rust").is_empty());
    }

    #[test]
    fn tabs_stop_every_four_columns() {
        assert_eq!(expand_tabs("\ta\n12\tb"), "    a\n12  b");
    }
}
