//! Code block highlighting in the classic Monokai roles (docs/design/
//! components-editor.md 5, Code; the user's choice of 2026-11-25):
//! syntect's parser over two-face's syntax set, each scope mapped to a
//! Monokai role the way the Monokai theme does; `diff` and `patch` colour
//! whole lines. Pure logic: the view maps roles to `syntax.monokai.*`.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::sync::{Arc, Mutex, OnceLock};

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

static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();
static WARMING: OnceLock<()> = OnceLock::new();

/// Loads the syntax set on a background thread, once, so the first code
/// block does not wait for it on the UI thread.
pub fn warm() {
    WARMING.get_or_init(|| {
        spawn("acta-syntaxes", || {
            SYNTAXES.get_or_init(load);
        });
    });
}

/// A worker for the parser: fancy-regex backtracks recursively, which can
/// outgrow a default stack (the UI thread's is 1 MB on Windows), so
/// highlighting never runs on the UI thread and gets a large stack here.
fn spawn(name: &str, f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .name(name.into())
        .stack_size(16 * 1024 * 1024)
        .spawn(f)
        .ok();
}

/// The runs of a block, highlighted once on a worker.
pub type Runs = Arc<Vec<(Range<usize>, Role)>>;

/// Highlighted runs per block, keyed by language and code; `None` while
/// the worker is on it.
static RUNS: Mutex<Option<HashMap<u64, Option<Runs>>>> = Mutex::new(None);

/// The runs of `code` in `lang`, or `None` while they are being worked
/// out off the UI thread (the caller draws plain text and asks again).
pub fn runs(code: &str, lang: &str) -> Option<Runs> {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (lang, code).hash(&mut hasher);
    let key = hasher.finish();
    let mut cache = RUNS.lock().unwrap_or_else(|e| e.into_inner());
    let cache = cache.get_or_insert_with(HashMap::new);
    if let Some(entry) = cache.get(&key) {
        return entry.clone();
    }
    if cache.len() > 512 {
        cache.retain(|_, entry| entry.is_none());
    }
    cache.insert(key, None);
    let (code, lang) = (code.to_string(), lang.to_string());
    spawn("acta-highlight", move || {
        SYNTAXES.get_or_init(load);
        let runs = Arc::new(highlight(&code, &lang));
        if let Some(cache) = RUNS.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            cache.insert(key, Some(runs));
        }
    });
    None
}

/// PowerShell, which two-face's set lacks: the SublimeText/PowerShell
/// package's syntax (MIT, assets/syntaxes/powershell).
const POWERSHELL: &str =
    include_str!("../../../../assets/syntaxes/powershell/PowerShell.sublime-syntax");

/// two-face's syntax set with PowerShell added. Linking the set again
/// takes a moment, so it runs on the warm-up thread.
fn load() -> SyntaxSet {
    let set = two_face::syntax::extra_newlines();
    let Ok(powershell) = SyntaxDefinition::load_from_str(POWERSHELL, true, None) else {
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
pub fn highlight(code: &str, lang: &str) -> Vec<(Range<usize>, Role)> {
    if code.len() > MAX_BYTES || code.lines().count() > MAX_LINES {
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
    }

    #[test]
    fn tabs_stop_every_four_columns() {
        assert_eq!(expand_tabs("\ta\n12\tb"), "    a\n12  b");
    }
}
