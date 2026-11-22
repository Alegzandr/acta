//! Code block highlighting in five roles (docs/design/components-editor.md
//! 5, Code): syntect's parser over two-face's syntax set, each scope mapped
//! to Default, Keyword, String, Constant or Comment; `diff` and `patch`
//! colour whole lines. Pure logic: the view maps roles to tokens.

use std::ops::Range;
use std::sync::OnceLock;

use syntect::parsing::{ParseState, Scope, ScopeStack, SyntaxSet};

/// A highlighted run's role. Default text has no run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Keyword,
    String,
    Constant,
    Comment,
    /// A `diff` line starting `+`.
    Added,
    /// A `diff` line starting `-`.
    Removed,
    /// A `diff` hunk header `@@`.
    Hunk,
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
        std::thread::Builder::new()
            .name("acta-syntaxes".into())
            .spawn(|| {
                SYNTAXES.get_or_init(two_face::syntax::extra_newlines);
            })
            .ok();
    });
}

/// Whether [`highlight`] can colour code yet.
pub fn ready() -> bool {
    SYNTAXES.get().is_some()
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
    let Some(syntax) = syntaxes.find_syntax_by_token(lang) else {
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
        push(&mut runs, offset + at..offset + line.len(), roles.role(&stack));
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

/// The scope prefixes of the five roles.
struct RoleScopes {
    comment: Scope,
    string: Scope,
    escape: Scope,
    numeric: Scope,
    language: Scope,
    character: Scope,
    operator: Scope,
    keyword: Scope,
    storage: Scope,
}

impl RoleScopes {
    fn get() -> &'static RoleScopes {
        static ROLES: OnceLock<RoleScopes> = OnceLock::new();
        ROLES.get_or_init(|| {
            let s = |name: &str| Scope::new(name).expect("a valid scope");
            RoleScopes {
                comment: s("comment"),
                string: s("string"),
                escape: s("constant.character.escape"),
                numeric: s("constant.numeric"),
                language: s("constant.language"),
                character: s("constant.character"),
                operator: s("keyword.operator"),
                keyword: s("keyword"),
                storage: s("storage"),
            }
        })
    }

    /// The innermost scope that names a role decides; an operator is
    /// default text, an escape belongs to its string.
    fn role(&self, stack: &ScopeStack) -> Option<Role> {
        for scope in stack.as_slice().iter().rev() {
            let is = |prefix: Scope| prefix.is_prefix_of(*scope);
            if is(self.comment) {
                return Some(Role::Comment);
            }
            if is(self.string) || is(self.escape) {
                return Some(Role::String);
            }
            if is(self.numeric) || is(self.language) || is(self.character) {
                return Some(Role::Constant);
            }
            if is(self.operator) {
                return None;
            }
            if is(self.keyword) || is(self.storage) {
                return Some(Role::Keyword);
            }
        }
        None
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
        SYNTAXES.get_or_init(two_face::syntax::extra_newlines);
    }

    fn roles_of(code: &str, lang: &str) -> Vec<(&str, Role)> {
        highlight(code, lang)
            .into_iter()
            .map(|(r, role)| (&code[r], role))
            .collect()
    }

    #[test]
    fn rust_in_five_roles() {
        load();
        let code = "// Alexandre's store\npub fn open(n: u32) -> bool {\n    let s = \"a\\n\";\n    n > 42 && true\n}\n";
        let runs = roles_of(code, "rust");
        let has = |text: &str, role: Role| runs.iter().any(|(t, r)| t.contains(text) && *r == role);
        assert!(has("// Alexandre's store", Role::Comment), "{runs:?}");
        assert!(has("pub", Role::Keyword), "{runs:?}");
        assert!(has("fn", Role::Keyword), "{runs:?}");
        assert!(has("let", Role::Keyword), "{runs:?}");
        assert!(has("\"a\\n\"", Role::String), "{runs:?}");
        assert!(has("42", Role::Constant), "{runs:?}");
        assert!(has("true", Role::Constant), "{runs:?}");
        // Operators and identifiers stay default text.
        assert!(!runs.iter().any(|(t, _)| t.contains("&&") || t.contains("open")), "{runs:?}");
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
            ("powershell", "$a = \"b\""),
            ("html", "<a href=\"x\">y</a>"),
            ("go", "func main() { return }"),
            ("sql", "SELECT 1"),
        ] {
            assert!(!highlight(code, lang).is_empty(), "{lang}");
        }
        assert!(highlight("anything", "no-such-language").is_empty());
    }

    #[test]
    fn diff_colours_whole_lines() {
        let code = "--- a/x\n+++ b/x\n@@ -1 +1 @@\n-old\n+new\n same\n";
        assert_eq!(
            roles_of(code, "diff"),
            [("@@ -1 +1 @@", Role::Hunk), ("-old", Role::Removed), ("+new", Role::Added)]
        );
        assert_eq!(roles_of("+x\n", "patch"), [("+x", Role::Added)]);
    }

    #[test]
    fn plain_languages_and_large_blocks_stay_plain() {
        assert_eq!(language(Some("Rust ignore")), Some("rust".into()));
        for plain in [None, Some(""), Some("text"), Some("mermaid"), Some("math"), Some("plaintext")] {
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
