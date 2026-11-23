//! The memory quality checker (ADR 0015, 2 and 3): pure functions shared
//! by the write-time checks, the scan and the flags. Detection is lexical
//! and local; the agent judges what a signal means.
//!
//! - [`normalize`]: Unicode NFKD with combining marks removed, lower case,
//!   English and French stop words dropped, a light suffix strip; code
//!   spans, flags, paths and numbers kept as whole tokens.
//! - [`compare`]: the title trigram score `t`, the content token score `w`,
//!   `s = max(t, w, (t + w) / 2)`, the conflict signals (polarity, an
//!   antonym, a value) and the [`Verdict`].
//! - [`kb_flags`]: the flags the store can compute (age, a closed topic,
//!   an open pair, a removed folder), never stored.
//!
//! The thresholds are constants fixed by the labelled fixture in
//! `fixture.rs`; any change here re-runs it.

use crate::date::Date;
use crate::model::{KbId, KbKind, KbPairKind, Status};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::OnceLock;
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

#[cfg(test)]
mod fixture;

/// `s` at or above: a duplicate (when no conflict signal).
pub const DUPLICATE: f64 = 0.45;
/// `s` at or above: related (the ack names it; nothing is refused).
pub const RELATED: f64 = 0.30;
/// `s` at or above, with a conflict signal: a possible conflict.
pub const CONFLICT_OVERLAP: f64 = 0.30;
/// Most terms the full-text prefilter ORs together.
pub const MAX_FTS_TERMS: usize = 32;

/// The text of a memory as written: title, one-line summary, body.
#[derive(Clone, Copy, Debug)]
pub struct Text<'a> {
    pub title: &'a str,
    pub summary: &'a str,
    pub body: &'a str,
}

/// What kind of token: a word (stemmed), or something kept whole.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Word,
    /// A number or a version: `8080`, `0.43.0`, `v18`, `30s`.
    Value,
    /// A code span, a `--flag` or a path.
    Whole,
    /// The end of a clause (`.`, `;`, `:`, `!`, `?`): a negation's reach
    /// and a value's anchor stop there.
    Break,
}

#[derive(Clone, Debug)]
struct Tok {
    text: String,
    kind: Kind,
    /// A stop word (kept in the sequence for polarity and anchors).
    stop: bool,
}

/// A memory's normalised form, computed once and compared many times.
#[derive(Clone, Debug)]
pub struct Profile {
    /// Content tokens of each field, in order (for "same").
    fields: [Vec<String>; 3],
    /// Character trigrams of the normalised title.
    grams: HashSet<[char; 3]>,
    /// Content tokens of title, summary and body.
    words: HashSet<String>,
    /// Stems of every word, stop words included (for antonyms).
    stems: HashSet<String>,
    /// A negation or a prohibition somewhere.
    negated: bool,
    /// The content tokens a negation governs.
    negated_words: HashSet<String>,
    /// Values by anchor (the content word or flag before them).
    values: BTreeMap<String, BTreeSet<String>>,
}

/// Why a pair may contradict itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Signal {
    /// A negation or a prohibition on one side only.
    Polarity,
    /// One side says a word, the other its opposite.
    Antonym(&'static str, &'static str),
    /// The same anchor with different values (`timeout 30` / `timeout 60`).
    Value(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Verdict {
    /// Normalised title, summary and body all equal.
    Same,
    Conflict,
    Duplicate,
    Related,
    Unrelated,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Comparison {
    /// Jaccard of the titles' character trigrams.
    pub t: f64,
    /// Jaccard of the content tokens.
    pub w: f64,
    pub s: f64,
    pub signal: Option<Signal>,
    pub verdict: Verdict,
}

/// The content tokens of a text: what duplicates and conflicts compare.
pub fn normalize(text: &str) -> Vec<String> {
    tokens(text)
        .into_iter()
        .filter(|t| !t.stop)
        .map(|t| t.text)
        .collect()
}

/// Builds the profile of one memory.
pub fn profile(text: &Text<'_>) -> Profile {
    let parts = [tokens(text.title), tokens(text.summary), tokens(text.body)];
    let content = |toks: &[Tok]| -> Vec<String> {
        toks.iter()
            .filter(|t| !t.stop)
            .map(|t| t.text.clone())
            .collect()
    };
    let fields = [content(&parts[0]), content(&parts[1]), content(&parts[2])];
    let title = format!(" {} ", fields[0].join(" "));
    let chars: Vec<char> = title.chars().collect();
    let grams = chars.windows(3).map(|w| [w[0], w[1], w[2]]).collect();
    let words = fields.iter().flatten().cloned().collect();
    let mut stems = HashSet::new();
    let mut negated = false;
    let mut negated_words = HashSet::new();
    let mut values: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for toks in &parts {
        let (n, scope) = negation(toks);
        negated |= n;
        negated_words.extend(scope);
        for t in toks.iter().filter(|t| t.kind == Kind::Word) {
            stems.insert(if t.stop {
                stem(&t.text)
            } else {
                t.text.clone()
            });
        }
        for (anchor, value) in anchored_values(toks) {
            values.entry(anchor).or_default().insert(value);
        }
    }
    Profile {
        fields,
        grams,
        words,
        stems,
        negated,
        negated_words,
        values,
    }
}

/// Scores a pair and gives its verdict (ADR 0015, 3's table).
pub fn compare(a: &Profile, b: &Profile) -> Comparison {
    compare_at(a, b, Thresholds::DEFAULT)
}

/// The three thresholds; provisional (ADR 0015, Amendment): to be
/// recalibrated from the quality log after dogfooding.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Thresholds {
    pub duplicate: f64,
    pub related: f64,
    pub conflict: f64,
}

impl Thresholds {
    pub const DEFAULT: Thresholds = Thresholds {
        duplicate: DUPLICATE,
        related: RELATED,
        conflict: CONFLICT_OVERLAP,
    };
}

/// [`compare`] with other thresholds (the fixture's hold-out tuning).
pub fn compare_at(a: &Profile, b: &Profile, th: Thresholds) -> Comparison {
    let t = jaccard(&a.grams, &b.grams);
    let w = jaccard(&a.words, &b.words);
    let s = t.max(w).max(0.5 * t + 0.5 * w);
    let signal = conflict_signal(a, b);
    let verdict = if a.fields == b.fields && signal.is_none() {
        Verdict::Same
    } else if signal.is_some() && s >= th.conflict {
        Verdict::Conflict
    } else if s >= th.duplicate {
        Verdict::Duplicate
    } else if s >= th.related {
        Verdict::Related
    } else {
        Verdict::Unrelated
    };
    Comparison {
        t,
        w,
        s,
        signal,
        verdict,
    }
}

/// The full-text prefilter's query: the text's content words as quoted
/// prefixes, OR-ed, longest first, at most [`MAX_FTS_TERMS`]. `None` when
/// there is nothing to look for.
pub fn fts_terms(text: &Text<'_>) -> Option<String> {
    let mut seen = BTreeSet::new();
    for field in [text.title, text.summary, text.body] {
        for t in tokens(field) {
            // Words only: FTS5 splits paths and versions its own way.
            // 3 to 40 chars: a pasted blob is not a term.
            if t.kind == Kind::Word && !t.stop && (3..=40).contains(&t.text.chars().count()) {
                seen.insert(t.text);
            }
        }
    }
    let mut terms: Vec<String> = seen.into_iter().collect();
    terms.sort_by(|x, y| y.len().cmp(&x.len()).then(x.cmp(y)));
    terms.truncate(MAX_FTS_TERMS);
    if terms.is_empty() {
        return None;
    }
    Some(
        terms
            .iter()
            .map(|t| format!("\"{t}\"*"))
            .collect::<Vec<_>>()
            .join(" OR "),
    )
}

fn jaccard<T: Eq + std::hash::Hash>(a: &HashSet<T>, b: &HashSet<T>) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 0.0;
    }
    let inter = a.intersection(b).count();
    let union = a.len() + b.len() - inter;
    inter as f64 / union as f64
}

fn conflict_signal(a: &Profile, b: &Profile) -> Option<Signal> {
    for (x, y, sx, sy) in antonyms() {
        let one = a.stems.contains(sx) && !a.stems.contains(sy);
        let other = b.stems.contains(sy) && !b.stems.contains(sx);
        let rev_one = a.stems.contains(sy) && !a.stems.contains(sx);
        let rev_other = b.stems.contains(sx) && !b.stems.contains(sy);
        if (one && other) || (rev_one && rev_other) {
            return Some(Signal::Antonym(x, y));
        }
    }
    for (anchor, va) in &a.values {
        if let Some(vb) = b.values.get(anchor)
            && va.is_disjoint(vb)
        {
            return Some(Signal::Value(anchor.clone()));
        }
    }
    // A negation on one side only, governing a word the other side says.
    let flips = |x: &Profile, y: &Profile| {
        x.negated && !y.negated && x.negated_words.iter().any(|w| y.words.contains(w))
    };
    if flips(a, b) || flips(b, a) {
        return Some(Signal::Polarity);
    }
    None
}

/// Case folded, accents removed (NFKD, combining marks dropped), curly
/// apostrophes made straight.
pub(crate) fn fold(text: &str) -> String {
    text.nfkd()
        .filter(|c| !is_combining_mark(*c))
        .collect::<String>()
        .to_lowercase()
        .replace(['\u{2019}', '\u{2018}'], "'")
}

/// The token sequence of a text.
fn tokens(text: &str) -> Vec<Tok> {
    let folded = fold(text);
    let mut out = Vec::new();
    // Code spans first: they may hold spaces.
    let mut rest = String::with_capacity(folded.len());
    let mut parts = folded.split('`');
    if let Some(first) = parts.next() {
        rest.push_str(first);
    }
    let mut inside = true;
    let mut spans: Vec<String> = Vec::new();
    for part in parts {
        if inside {
            let code = part.split_whitespace().collect::<Vec<_>>().join(" ");
            if !code.is_empty() {
                // A marker word keeps the span's place in the sequence.
                rest.push_str(&format!(" \u{1}{} ", spans.len()));
                spans.push(code);
            }
        } else {
            rest.push_str(part);
        }
        inside = !inside;
    }
    for chunk in rest.split_whitespace() {
        if let Some(n) = chunk.strip_prefix('\u{1}') {
            if let Some(code) = n.parse::<usize>().ok().and_then(|i| spans.get(i)) {
                out.push(Tok {
                    text: format!("`{code}`"),
                    kind: Kind::Whole,
                    stop: false,
                });
            }
            continue;
        }
        let breaks = chunk.ends_with(['.', ';', ':', '!', '?']);
        let chunk = chunk.trim_matches(|c: char| {
            matches!(
                c,
                '(' | ')'
                    | '['
                    | ']'
                    | '{'
                    | '}'
                    | '<'
                    | '>'
                    | ','
                    | ';'
                    | ':'
                    | '!'
                    | '?'
                    | '"'
                    | '\''
                    | '.'
                    | '*'
                    | '_'
            )
        });
        if chunk.is_empty() {
            if breaks {
                out.push(BREAK);
            }
            continue;
        }
        if chunk.starts_with("--") && chunk.len() > 2 {
            out.push(Tok {
                text: chunk.to_owned(),
                kind: Kind::Whole,
                stop: false,
            });
        } else if is_value(chunk) {
            out.push(Tok {
                text: chunk.trim_start_matches('v').replace(',', "."),
                kind: Kind::Value,
                stop: false,
            });
        } else if is_path(chunk) {
            out.push(Tok {
                text: chunk.replace('\\', "/"),
                kind: Kind::Whole,
                stop: false,
            });
        } else {
            // "don't", "can't", "isn't": a stop word and a negation.
            if let Some(base) = chunk.strip_suffix("n't") {
                out.push(Tok {
                    text: base.to_owned(),
                    kind: Kind::Word,
                    stop: true,
                });
                out.push(Tok {
                    text: "not".into(),
                    kind: Kind::Word,
                    stop: true,
                });
            } else {
                for word in chunk.split(|c: char| !c.is_alphanumeric()) {
                    if word.is_empty() {
                        continue;
                    }
                    let stop = is_stop(word) || word.chars().count() < 2;
                    out.push(Tok {
                        // Stop words stay as written (negations are read off them).
                        text: if stop { word.to_owned() } else { stem(word) },
                        kind: Kind::Word,
                        stop,
                    });
                }
            }
        }
        if breaks {
            out.push(BREAK);
        }
    }
    out
}

const BREAK: Tok = Tok {
    text: String::new(),
    kind: Kind::Break,
    stop: true,
};

/// `8080`, `0.43.0`, `v18`, `1,5`, `30s`, `16mb`, `90%`.
fn is_value(chunk: &str) -> bool {
    let s = chunk.strip_prefix('v').unwrap_or(chunk);
    let digits = s
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == ',');
    let head: String = digits.collect();
    if head.is_empty() || !head.starts_with(|c: char| c.is_ascii_digit()) {
        return false;
    }
    let unit = &s[head.len()..];
    unit.len() <= 3 && unit.chars().all(|c| c.is_ascii_alphabetic() || c == '%')
}

/// Something with a separator (`src/store/kb.rs`, `C:\x`), or a file
/// name with an extension (`Cargo.toml`, `kb.rs`).
fn is_path(chunk: &str) -> bool {
    if chunk.contains('/') || chunk.contains('\\') {
        return chunk.chars().any(|c| c.is_alphanumeric());
    }
    let Some((name, ext)) = chunk.rsplit_once('.') else {
        return false;
    };
    name.chars().count() >= 2
        && name.chars().any(|c| c.is_alphabetic())
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.'))
        && (1..=5).contains(&ext.len())
        && ext.chars().all(|c| c.is_ascii_alphanumeric())
        && ext.chars().any(|c| c.is_ascii_alphabetic())
}

/// The light suffix strip of ADR 0015, 3 for English and French: plurals,
/// `-ing`, `-ed`, `-ment`, `-tion` (and `-ation`), French infinitives,
/// then a final `e`, so `file`, `files`; `configure`, `configured`,
/// `configuration`, `configurée` meet. Words with a digit stay whole.
pub fn stem(word: &str) -> String {
    let n = word.chars().count();
    if n < 4 || word.chars().any(|c| c.is_ascii_digit()) {
        return word.to_owned();
    }
    const SUFFIXES: &[&str] = &[
        "ations", "ation", "tions", "tion", "ements", "ement", "ments", "ment", "ings", "ing",
        "ees", "ee", "ers", "er", "ez", "ed", "es", "s", "x",
    ];
    let mut s = word.to_owned();
    for suf in SUFFIXES {
        if s.ends_with(suf) && n - suf.len() >= 3 {
            if *suf == "s" && (s.ends_with("ss") || s.ends_with("us") || s.ends_with("is")) {
                continue;
            }
            // Short `-er` words are English nouns (`user`, `order`).
            if matches!(*suf, "er" | "ers") && n - suf.len() < 4 {
                continue;
            }
            s.truncate(s.len() - suf.len());
            break;
        }
    }
    if s.ends_with('e') && s.chars().count() > 3 {
        s.pop();
    }
    s
}

/// Whether a negation or a prohibition is there, and the content
/// tokens it governs, within its clause: four after it, and for French
/// markers and prohibitions also two before it ("ne lancez pas les
/// tests", "the flag is deprecated"). Plain markers are read off stop words
/// only, so a content word that stems like one (`notes`) is not one.
fn negation(toks: &[Tok]) -> (bool, HashSet<String>) {
    let mut found = false;
    let mut scope = HashSet::new();
    let word = |i: usize| -> Option<&Tok> { toks.get(i).filter(|t| t.kind == Kind::Word) };
    for (i, t) in toks.iter().enumerate() {
        let before_is = |k: usize, set: &[&str]| -> bool {
            (i.saturating_sub(k)..i)
                .any(|j| word(j).is_some_and(|p| set.contains(&p.text.as_str())))
        };
        // French puts the verb before `pas` and `jamais`; English after.
        let (marker, after_only) = match (t.kind, t.stop, t.text.as_str()) {
            (Kind::Word, true, "not" | "never" | "cannot" | "no" | "nor") => (true, true),
            (Kind::Word, true, "jamais" | "pas" | "aucun" | "aucune") => (true, false),
            (Kind::Word, true, "instead") => (true, true),
            (Kind::Word, true, "lieu") if before_is(1, &["au"]) => (true, true),
            // "ne ... plus", "n'... plus" (not "plus" = more).
            (Kind::Word, true, "plus") if before_is(3, &["ne", "n"]) => (true, false),
            // "X is deprecated", "avoid X": both ways.
            // Prohibitions, as stems: avoid, éviter, deprecated, obsolète.
            (Kind::Word, false, w)
                if w == "avoid"
                    || w.starts_with("deprecat")
                    || w.starts_with("obsolet")
                    || w.starts_with("evit") =>
            {
                (true, false)
            }
            _ => (false, false),
        };
        if !marker {
            continue;
        }
        found = true;
        let mut reach = |range: &mut dyn Iterator<Item = usize>| {
            for j in range {
                let x = &toks[j];
                if x.kind == Kind::Break {
                    break;
                }
                if !x.stop {
                    scope.insert(x.text.clone());
                }
            }
        };
        if !after_only {
            reach(&mut (i.saturating_sub(2)..i).rev());
        }
        reach(&mut (i + 1..(i + 5).min(toks.len())));
    }
    (found, scope)
}

/// `(anchor, value)` for each number, version, flag, code span or path,
/// the anchor being the nearest content word or flag before it (within
/// three tokens).
fn anchored_values(toks: &[Tok]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (i, t) in toks.iter().enumerate() {
        if matches!(t.kind, Kind::Word | Kind::Break) {
            continue;
        }
        let anchor = toks[i.saturating_sub(3)..i]
            .iter()
            .rev()
            .take_while(|p| p.kind != Kind::Break)
            .find(|p| (p.kind == Kind::Word && !p.stop) || p.text.starts_with("--"));
        if let Some(a) = anchor {
            out.push((a.text.clone(), t.text.clone()));
        }
    }
    out
}

/// Antonym pairs (about 40, English and French) with their stems.
fn antonyms() -> &'static [(&'static str, &'static str, String, String)] {
    static TABLE: OnceLock<Vec<(&'static str, &'static str, String, String)>> = OnceLock::new();
    TABLE.get_or_init(|| {
        const PAIRS: &[(&str, &str)] = &[
            ("enable", "disable"),
            ("enabled", "disabled"),
            ("true", "false"),
            ("allow", "deny"),
            ("allow", "forbid"),
            ("always", "never"),
            ("required", "optional"),
            ("mandatory", "optional"),
            ("before", "after"),
            ("sync", "async"),
            ("synchronous", "asynchronous"),
            ("keep", "remove"),
            ("keep", "delete"),
            ("include", "exclude"),
            ("start", "stop"),
            ("open", "close"),
            ("lock", "unlock"),
            ("public", "private"),
            ("local", "remote"),
            ("minimum", "maximum"),
            ("increase", "decrease"),
            ("accept", "reject"),
            ("show", "hide"),
            ("debug", "release"),
            ("static", "dynamic"),
            ("online", "offline"),
            ("valid", "invalid"),
            ("succeed", "fail"),
            ("serial", "parallel"),
            ("sequential", "parallel"),
            ("activer", "desactiver"),
            ("active", "desactive"),
            ("vrai", "faux"),
            ("autoriser", "interdire"),
            ("autorise", "interdit"),
            ("toujours", "jamais"),
            ("obligatoire", "facultatif"),
            ("obligatoire", "optionnel"),
            ("avant", "apres"),
            ("synchrone", "asynchrone"),
            ("garder", "supprimer"),
            ("conserver", "supprimer"),
            ("inclure", "exclure"),
            ("demarrer", "arreter"),
            ("ouvrir", "fermer"),
            ("public", "prive"),
            ("local", "distant"),
            ("augmenter", "diminuer"),
            ("accepter", "refuser"),
            ("afficher", "masquer"),
            ("sequentiel", "parallele"),
            ("serie", "parallele"),
        ];
        PAIRS
            .iter()
            .map(|(x, y)| (*x, *y, stem(&fold(x)), stem(&fold(y))))
            .collect()
    })
}

fn is_stop(word: &str) -> bool {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| STOP_WORDS.iter().copied().collect())
        .contains(word)
}

/// English and French stop words (folded: no accents). Negations are in
/// it: they are signals, not content.
const STOP_WORDS: &[&str] = &[
    // English
    "a", "about", "after", "all", "also", "an", "and", "any", "are", "as", "at", "be", "been",
    "before", "but", "by", "can", "cannot", "could", "do", "does", "doing", "done", "each", "for",
    "from", "had", "has", "have", "here", "how", "if", "in", "into", "is", "it", "its", "just",
    "may", "more", "most", "must", "never", "no", "nor", "not", "now", "of", "on", "once", "one",
    "only", "or", "other", "our", "out", "over", "same", "should", "so", "some", "such", "than",
    "that", "the", "their", "them", "then", "there", "these", "they", "this", "those", "through",
    "to", "too", "under", "until", "up", "very", "was", "we", "were", "what", "when", "where",
    "which", "while", "who", "why", "will", "with", "would", "you", "your", "instead", "always",
    "every", "via", "per", "still", "both", "own", "get", "gets", // French
    "au", "aux", "avec", "ce", "ces", "cet", "cette", "dans", "de", "des", "du", "elle", "en",
    "est", "et", "etre", "eux", "il", "ils", "je", "la", "le", "les", "leur", "lui", "ma", "mais",
    "me", "meme", "mes", "moi", "mon", "ne", "nos", "notre", "nous", "on", "ou", "par", "pas",
    "plus", "pour", "qu", "que", "qui", "sa", "se", "ses", "son", "sont", "sur", "ta", "te", "tes",
    "toi", "ton", "tu", "un", "une", "vos", "votre", "vous", "y", "ete", "sans", "sous", "tout",
    "tous", "toute", "toutes", "jamais", "aucun", "aucune", "lieu", "toujours", "doit", "faut",
    "peut", "comme", "donc", "car", "si", "lors", "quand", "ici", "deja", "encore", "aussi",
    "chaque", "entre", "vers", "chez", "fait", "faire", "il", "a",
];

/// A reason to look at a memory again (ADR 0015, 1), computed at read
/// time and never stored. These are the ones the store can compute; the
/// git flags come from the folder at read time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KbFlag {
    /// A memory not verified for [`crate::store::STALE_MEMORY_DAYS`] days.
    Old {
        days: i64,
    },
    /// The owning topic is Done or Dropped.
    TopicClosed(Status),
    /// An open pair.
    Conflict(KbId),
    Duplicate(KbId),
    /// Its context folder is in the trash.
    FolderRemoved,
}

impl KbFlag {
    /// The agent's marker (at most 30 chars): `?old 140d`, `?conflict K-9`.
    pub fn marker(&self) -> String {
        match self {
            KbFlag::Old { days } => format!("?old {days}d"),
            KbFlag::TopicClosed(s) => format!("?topic {}", s.as_str()),
            KbFlag::Conflict(id) => format!("?conflict {id}"),
            KbFlag::Duplicate(id) => format!("?dup {id}"),
            KbFlag::FolderRemoved => "?folder removed".into(),
        }
    }
}

/// What [`kb_flags`] reads about one entry.
#[derive(Clone, Debug)]
pub struct FlagFacts {
    pub kind: KbKind,
    /// The local day of its verified date.
    pub verified_on: Date,
    /// The status of the owning topic, for a topic's entry.
    pub topic_status: Option<Status>,
    /// Open pairs with live entries.
    pub pairs: Vec<(KbPairKind, KbId)>,
    pub folder_removed: bool,
}

/// The store-computable flags of one entry, most telling first.
pub fn kb_flags(f: &FlagFacts, today: Date) -> Vec<KbFlag> {
    let mut out = Vec::new();
    for (kind, other) in &f.pairs {
        out.push(match kind {
            KbPairKind::Conflict => KbFlag::Conflict(*other),
            KbPairKind::Duplicate => KbFlag::Duplicate(*other),
        });
    }
    out.sort_by_key(|f| match f {
        KbFlag::Conflict(id) => (0, id.0),
        KbFlag::Duplicate(id) => (1, id.0),
        _ => (2, 0),
    });
    if f.folder_removed {
        out.push(KbFlag::FolderRemoved);
    }
    if let Some(s @ (Status::Done | Status::Dropped)) = f.topic_status {
        out.push(KbFlag::TopicClosed(s));
    }
    let days = f.verified_on.days_until(today);
    if f.kind == KbKind::Memory && days >= crate::store::STALE_MEMORY_DAYS {
        out.push(KbFlag::Old { days });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(title: &str, summary: &str, body: &str) -> Profile {
        profile(&Text {
            title,
            summary,
            body,
        })
    }

    #[test]
    fn normalisation_folds_accents_drops_stop_words_and_keeps_code_whole() {
        assert_eq!(
            normalize("Les fichiers configurés de l'élève"),
            ["fichi", "configur", "elev"]
        );
        assert_eq!(normalize("The files are configured"), ["fil", "configur"]);
        assert_eq!(normalize("configuration"), ["configur"]);
        assert_eq!(
            normalize("Run `cargo test --workspace` in src/store/kb.rs on CI with --jobs 4"),
            [
                "run",
                "`cargo test --workspace`",
                "src/store/kb.rs",
                "ci",
                "--jobs",
                "4"
            ]
        );
        assert_eq!(
            normalize("Version v0.43.0, port 8080."),
            ["version", "0.43.0", "port", "8080"]
        );
        assert_eq!(normalize("Cargo.toml e.g. db"), ["cargo.toml", "db"]);
    }

    #[test]
    fn stems_meet_across_inflections() {
        for (a, b) in [
            ("file", "files"),
            ("configure", "configured"),
            ("configure", "configuration"),
            ("supprimer", "supprime"),
            ("branch", "branches"),
            ("tests", "testing"),
        ] {
            assert_eq!(stem(a), stem(b), "{a} / {b}");
        }
        assert_eq!(stem("user"), "user");
        assert_eq!(stem("class"), "class");
    }

    #[test]
    fn negation_and_prohibition_are_found_in_both_languages() {
        for text in [
            "Don't run the tests in parallel",
            "Never commit on main",
            "Avoid the global allocator",
            "The flag is deprecated",
            "Use rustls instead of OpenSSL",
            "Ne pas lancer les tests en parallèle",
            "Il ne faut plus utiliser ce script",
            "Éviter le cache global",
            "Utiliser rustls au lieu d'OpenSSL",
            "Cette option est obsolète",
        ] {
            assert!(p(text, "", "").negated, "{text}");
        }
        for text in [
            "Run the tests in parallel",
            "Plus de tests en parallèle",
            "Notes for Alexandre",
        ] {
            assert!(!p(text, "", "").negated, "{text}");
        }
    }

    #[test]
    fn signals_name_why() {
        let a = p("Request timeout", "The timeout is 30 s", "");
        let b = p("Request timeout", "The timeout is 60 s", "");
        assert_eq!(
            compare(&a, &b).signal,
            Some(Signal::Value("timeout".into()))
        );
        let a = p("Telemetry", "Telemetry is enabled by default", "");
        let b = p("Telemetry", "Telemetry is disabled by default", "");
        assert_eq!(
            compare(&a, &b).signal,
            Some(Signal::Antonym("enable", "disable"))
        );
        let a = p("Parallel tests", "Run the tests in parallel", "");
        let b = p("Parallel tests", "Do not run the tests in parallel", "");
        assert!(b.negated_words.contains("run"));
        assert_eq!(compare(&a, &b).signal, Some(Signal::Polarity));
        assert_eq!(compare(&a, &b).verdict, Verdict::Conflict);
    }

    #[test]
    fn verdicts_follow_the_thresholds() {
        let a = p(
            "Release checklist",
            "Steps before tagging",
            "Bump, changelog, tag.",
        );
        let same = p(
            "Release  checklist",
            "steps before tagging",
            "bump changelog tag",
        );
        assert_eq!(compare(&a, &same).verdict, Verdict::Same);
        let other = p("Font licence", "Inter is under the OFL", "See LICENSE.");
        let c = compare(&a, &other);
        assert_eq!(c.verdict, Verdict::Unrelated);
        assert!(c.s < RELATED);
        // s is the larger of the two scores.
        assert!((c.s - c.t.max(c.w)).abs() < 1e-9);
    }

    #[test]
    fn the_fts_query_quotes_prefixes() {
        let q = fts_terms(&Text {
            title: "Keychain entry",
            summary: "Alexandre's \"key\" OR name",
            body: "",
        })
        .unwrap();
        assert_eq!(
            q,
            "\"alexandr\"* OR \"keychain\"* OR \"entry\"* OR \"key\"* OR \"nam\"*"
        );
        assert_eq!(
            fts_terms(&Text {
                title: "the",
                summary: "",
                body: ""
            }),
            None
        );
    }

    #[test]
    fn flags_follow_age_topic_pairs_and_folder() {
        let today = Date::from_ymd(2026, 11, 22).unwrap();
        let facts = FlagFacts {
            kind: KbKind::Memory,
            verified_on: today.add_days(-140),
            topic_status: Some(Status::Done),
            pairs: vec![
                (KbPairKind::Duplicate, KbId(4)),
                (KbPairKind::Conflict, KbId(9)),
            ],
            folder_removed: true,
        };
        let flags = kb_flags(&facts, today);
        let markers: Vec<String> = flags.iter().map(KbFlag::marker).collect();
        assert_eq!(
            markers,
            [
                "?conflict K-9",
                "?dup K-4",
                "?folder removed",
                "?topic done",
                "?old 140d"
            ]
        );
        assert!(markers.iter().all(|m| m.len() <= 30));
        // A document does not age; 89 days is not old.
        let doc = FlagFacts {
            kind: KbKind::Document,
            pairs: vec![],
            topic_status: Some(Status::Active),
            folder_removed: false,
            ..facts.clone()
        };
        assert!(kb_flags(&doc, today).is_empty());
        let young = FlagFacts {
            verified_on: today.add_days(-89),
            ..doc.clone()
        };
        assert!(
            kb_flags(
                &FlagFacts {
                    kind: KbKind::Memory,
                    ..young
                },
                today
            )
            .is_empty()
        );
    }
}
