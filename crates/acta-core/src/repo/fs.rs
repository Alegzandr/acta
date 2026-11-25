//! The disk side of the Files view and the viewer (5.R.3, 5.R.4): a
//! folder's entries with git's ignore rules, the name search, a file read
//! for the viewer, and the "never run a program" check. No process, nothing
//! held open, links never followed into folders.

use crate::paths::Os;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use ignore::{Match, WalkBuilder};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Entries the name search visits at most (5.R.3, Filter by name).
pub const SEARCH_CAP: usize = 100_000;
/// The name search's time limit: past it, the results so far.
pub const SEARCH_LIMIT: Duration = Duration::from_secs(3);
/// Results the filter lists at most.
pub const MATCHES_SHOWN: usize = 200;
/// Text shown at most (5.R.4, Large text).
pub const TEXT_BYTES: usize = 1024 * 1024;
/// An image shown at most: bytes and pixels (5.R.4, Image).
pub const IMAGE_BYTES: u64 = 50 * 1024 * 1024;
pub const IMAGE_PIXELS: u64 = 100_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    Dir,
    File,
    /// A link to a folder: listed, never followed by a walk.
    LinkDir,
    /// A link to a file, or a broken link.
    LinkFile,
}

impl EntryKind {
    pub fn is_dir(self) -> bool {
        matches!(self, EntryKind::Dir | EntryKind::LinkDir)
    }
}

/// One row of the tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub kind: EntryKind,
    /// Ignored by git (shown only with "Show ignored files").
    pub ignored: bool,
}

/// The entries of `folder`/`rel` (`rel` relative, `/`-separated, `""` for
/// the folder itself): folders first, then files, each in natural order
/// ignoring case; `.git` never listed. In a git repository, what git
/// ignores is left out unless `show_ignored`, then marked.
///
/// A link to a folder expands only when its target stays inside the
/// boundary ([`boundary`]); else the error is [`outside_error`].
pub fn list_dir(folder: &Path, rel: &str, show_ignored: bool) -> io::Result<Vec<Entry>> {
    let dir = join_rel(folder, rel);
    if !inside(&boundary(folder), &dir)? {
        return Err(outside_error());
    }
    let rules = IgnoreRules::for_dir(&dir);
    let mut out = Vec::new();
    for e in fs::read_dir(&dir)? {
        let Ok(e) = e else { continue };
        let name = e.file_name().to_string_lossy().into_owned();
        if name == ".git" {
            continue;
        }
        let Ok(ft) = e.file_type() else { continue };
        let kind = if ft.is_symlink() {
            // Windows junctions read as links too.
            match fs::metadata(e.path()) {
                Ok(m) if m.is_dir() => EntryKind::LinkDir,
                _ => EntryKind::LinkFile,
            }
        } else if ft.is_dir() {
            EntryKind::Dir
        } else {
            EntryKind::File
        };
        let ignored = rules
            .as_ref()
            .is_some_and(|r| r.is_ignored(&e.path(), kind == EntryKind::Dir));
        if ignored && !show_ignored {
            continue;
        }
        out.push(Entry {
            name,
            kind,
            ignored,
        });
    }
    out.sort_by(|a, b| {
        b.kind
            .is_dir()
            .cmp(&a.kind.is_dir())
            .then_with(|| natural_cmp(&a.name, &b.name))
    });
    Ok(out)
}

/// What a link may point into: the worktree root of `folder`, or the
/// folder itself outside a repository.
pub fn boundary(folder: &Path) -> PathBuf {
    crate::agent::git::inspect(folder).map_or_else(|| folder.to_path_buf(), |i| i.root)
}

/// Whether `path`, links resolved, stays under `root`.
fn inside(root: &Path, path: &Path) -> io::Result<bool> {
    let root = fs::canonicalize(root)?;
    Ok(fs::canonicalize(path)?.starts_with(root))
}

/// The error of a link that leads out of the folder ("Link to outside the
/// folder").
pub fn outside_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "link to outside the folder",
    )
}

fn join_rel(folder: &Path, rel: &str) -> PathBuf {
    let mut p = folder.to_path_buf();
    for part in rel.split('/').filter(|s| !s.is_empty() && *s != ".") {
        p.push(part);
    }
    p
}

/// git's ignore rules that apply to one directory's entries: the global
/// excludes file, `info/exclude`, and each `.gitignore` from the worktree
/// root down to the directory (deeper wins, the last pattern of a file
/// wins). A directory inside an ignored one is ignored whole.
struct IgnoreRules {
    /// Deepest first.
    levels: Vec<Gitignore>,
    exclude: Gitignore,
    global: Gitignore,
    /// The directory itself, or one of its parents, is ignored.
    inside_ignored: bool,
}

impl IgnoreRules {
    fn for_dir(dir: &Path) -> Option<IgnoreRules> {
        let info = crate::agent::git::inspect(dir)?;
        let root = info.root;
        let global = GitignoreBuilder::new(&root).build_global().0;
        let mut b = GitignoreBuilder::new(&root);
        b.add(info.common_dir.join("info").join("exclude"));
        let exclude = b.build().unwrap_or_else(|_| Gitignore::empty());
        let mut rules = IgnoreRules {
            levels: Vec::new(),
            exclude,
            global,
            inside_ignored: false,
        };
        let rel = dir.strip_prefix(&root).ok()?;
        let mut level = root.clone();
        rules.add_level(&level);
        for part in rel.components() {
            level.push(part);
            if rules.is_ignored(&level, true) {
                rules.inside_ignored = true;
                return Some(rules);
            }
            rules.add_level(&level);
        }
        Some(rules)
    }

    fn add_level(&mut self, dir: &Path) {
        let file = dir.join(".gitignore");
        if !file.is_file() {
            return;
        }
        let mut b = GitignoreBuilder::new(dir);
        b.add(file);
        if let Ok(g) = b.build() {
            self.levels.insert(0, g);
        }
    }

    fn is_ignored(&self, path: &Path, is_dir: bool) -> bool {
        if self.inside_ignored {
            return true;
        }
        for g in self.levels.iter().chain([&self.exclude, &self.global]) {
            match g.matched(path, is_dir) {
                Match::Ignore(_) => return true,
                Match::Whitelist(_) => return false,
                Match::None => {}
            }
        }
        false
    }
}

/// "file2" before "file10", case ignored, then the exact name.
pub fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let (mut x, mut y) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (x.peek().copied(), y.peek().copied()) {
            (None, None) => return a.cmp(b),
            (None, _) => return Ordering::Less,
            (_, None) => return Ordering::Greater,
            (Some(c), Some(d)) if c.is_ascii_digit() && d.is_ascii_digit() => {
                let take = |it: &mut std::iter::Peekable<std::str::Chars>| {
                    let mut s = String::new();
                    while let Some(c) = it.peek().copied().filter(char::is_ascii_digit) {
                        s.push(c);
                        it.next();
                    }
                    s
                };
                let (m, n) = (take(&mut x), take(&mut y));
                let (mt, nt) = (m.trim_start_matches('0'), n.trim_start_matches('0'));
                let o = mt.len().cmp(&nt.len()).then_with(|| mt.cmp(nt));
                if o != Ordering::Equal {
                    return o;
                }
            }
            (Some(c), Some(d)) => {
                let o = c.to_lowercase().cmp(d.to_lowercase());
                if o != Ordering::Equal {
                    return o;
                }
                x.next();
                y.next();
            }
        }
    }
}

/// The name search's walk.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Search {
    /// Files, relative to the folder, `/`-separated, in walk order.
    pub files: Vec<String>,
    /// Stopped at [`SEARCH_CAP`] entries or at [`SEARCH_LIMIT`].
    pub capped: bool,
}

/// Every file under `folder` (5.R.3, Filter by name): dotfiles in, `.git`
/// out, links never followed, git's ignore rules unless `show_ignored`; at
/// most [`SEARCH_CAP`] entries within `limit`.
pub fn search_files(folder: &Path, show_ignored: bool, limit: Duration) -> Search {
    let deadline = Instant::now() + limit;
    let mut s = Search::default();
    let walk = WalkBuilder::new(folder)
        .hidden(false)
        .follow_links(false)
        .ignore(false)
        .git_ignore(!show_ignored)
        .git_exclude(!show_ignored)
        .git_global(!show_ignored)
        .parents(true)
        .require_git(true)
        .filter_entry(|e| e.file_name() != ".git")
        .build();
    let mut seen = 0usize;
    for e in walk {
        seen += 1;
        if seen > SEARCH_CAP || Instant::now() >= deadline {
            s.capped = true;
            break;
        }
        let Ok(e) = e else { continue };
        let Some(t) = e.file_type() else { continue };
        if e.depth() == 0 || t.is_dir() {
            continue;
        }
        // A link (or a Windows junction) to a folder is not a file.
        if t.is_symlink() && fs::metadata(e.path()).is_ok_and(|m| m.is_dir()) {
            continue;
        }
        if let Ok(rel) = e.path().strip_prefix(folder) {
            s.files.push(super::slash_path(rel));
        }
    }
    s
}

/// A ranked match of the filter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ranked {
    /// Index into the searched paths.
    pub index: usize,
    /// Byte offsets of the matched characters in the path (drawn strong).
    pub positions: Vec<usize>,
    /// The query matched within the file name.
    pub in_name: bool,
}

/// Fuzzy matching on the relative path, case ignored: name matches first,
/// then shorter paths, then by path. At most `shown` results, and the
/// count of all matches ("200 of 1,240 matches").
pub fn rank(paths: &[String], query: &str, shown: usize) -> (Vec<Ranked>, usize) {
    let q: Vec<char> = query
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect();
    if q.is_empty() {
        return (Vec::new(), 0);
    }
    let mut all: Vec<Ranked> = paths
        .iter()
        .enumerate()
        .filter_map(|(index, p)| {
            let name_at = p.rfind('/').map_or(0, |i| i + 1);
            if let Some(positions) = subsequence(&p[name_at..], &q) {
                return Some(Ranked {
                    index,
                    positions: positions.into_iter().map(|i| i + name_at).collect(),
                    in_name: true,
                });
            }
            subsequence(p, &q).map(|positions| Ranked {
                index,
                positions,
                in_name: false,
            })
        })
        .collect();
    let total = all.len();
    all.sort_by(|a, b| {
        b.in_name
            .cmp(&a.in_name)
            .then_with(|| paths[a.index].len().cmp(&paths[b.index].len()))
            .then_with(|| paths[a.index].cmp(&paths[b.index]))
    });
    all.truncate(shown);
    (all, total)
}

/// Byte offsets of `q`'s characters found in order in `s`, case ignored.
fn subsequence(s: &str, q: &[char]) -> Option<Vec<usize>> {
    let mut out = Vec::with_capacity(q.len());
    let mut want = q.iter().peekable();
    for (i, c) in s.char_indices() {
        let Some(&&w) = want.peek() else { break };
        if c.to_lowercase().eq(std::iter::once(w)) {
            out.push(i);
            want.next();
        }
    }
    want.peek().is_none().then_some(out)
}

/// What the viewer shows of a file (5.R.4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileContent {
    /// UTF-8 with no NUL in the first 8 KB, a BOM dropped; the first
    /// [`TEXT_BYTES`] when larger (`truncated`).
    Text {
        text: String,
        size: u64,
        truncated: bool,
    },
    /// One of the five image formats, within the limits, in memory.
    Image {
        bytes: Vec<u8>,
        width: u32,
        height: u32,
    },
    /// A NUL byte in the first 8 KB, or an image that does not decode.
    Binary { size: u64 },
    /// No NUL, but not UTF-8.
    NotUtf8 { size: u64 },
    /// An image over the limits.
    TooLarge { size: u64 },
    /// A link whose target is outside the boundary: nothing is read.
    LinkOutside,
    /// Not a regular file (a folder, a FIFO, a device): never opened.
    NotAFile,
}

/// Reads `folder`/`rel` for the viewer: never more than [`TEXT_BYTES`] of
/// text or [`IMAGE_BYTES`] of image; the first 8 KB decide the kind. A
/// link is followed only to a target inside [`boundary`]; anything but a
/// regular file is refused before it is opened.
pub fn read_file(folder: &Path, rel: &str) -> io::Result<FileContent> {
    let path = join_rel(folder, rel);
    // Every link on the way resolved: the file must stay inside.
    fs::symlink_metadata(&path)?;
    if !inside(&boundary(folder), &path)? {
        return Ok(FileContent::LinkOutside);
    }
    let path = fs::canonicalize(&path)?;
    if !fs::metadata(&path)?.is_file() {
        return Ok(FileContent::NotAFile);
    }
    let mut f = fs::File::open(&path)?;
    let meta = f.metadata()?;
    if !meta.is_file() {
        return Ok(FileContent::NotAFile);
    }
    let size = meta.len();
    let mut head = Vec::with_capacity(crate::files::SNIFF_BYTES);
    (&mut f)
        .take(crate::files::SNIFF_BYTES as u64)
        .read_to_end(&mut head)?;
    let media = crate::files::sniff(&head);
    if media.starts_with("image/") {
        if size > IMAGE_BYTES {
            return Ok(FileContent::TooLarge { size });
        }
        let mut bytes = head;
        (&mut f).take(IMAGE_BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > IMAGE_BYTES {
            return Ok(FileContent::TooLarge {
                size: bytes.len() as u64,
            });
        }
        let dims = image::ImageReader::new(io::Cursor::new(&bytes))
            .with_guessed_format()
            .ok()
            .and_then(|r| r.into_dimensions().ok());
        return Ok(match dims {
            Some((w, h)) if u64::from(w) * u64::from(h) > IMAGE_PIXELS => {
                FileContent::TooLarge { size }
            }
            Some((width, height)) => FileContent::Image {
                bytes,
                width,
                height,
            },
            None => FileContent::Binary { size },
        });
    }
    if head.contains(&0) {
        return Ok(FileContent::Binary { size });
    }
    let mut bytes = head;
    (&mut f)
        .take((TEXT_BYTES - bytes.len().min(TEXT_BYTES)) as u64)
        .read_to_end(&mut bytes)?;
    let truncated = size > bytes.len() as u64;
    let text = match std::str::from_utf8(&bytes) {
        Ok(t) => t,
        // Cut inside a character by the cap: keep what is whole.
        Err(e) if truncated && e.error_len().is_none() => {
            std::str::from_utf8(&bytes[..e.valid_up_to()]).unwrap_or_default()
        }
        Err(_) => return Ok(FileContent::NotUtf8 { size }),
    };
    let text = text.strip_prefix('\u{feff}').unwrap_or(text).to_owned();
    Ok(FileContent::Text {
        text,
        size,
        truncated,
    })
}

/// `PATHEXT` when it is not set.
const DEFAULT_PATHEXT: &str = ".COM;.EXE;.BAT;.CMD;.VBS;.VBE;.JS;.JSE;.WSF;.WSH;.MSC";

/// Whether the system opener would run this file as a program (5.R.3,
/// Never run a program): "Open in default app" is then disabled. `name` is
/// the file name, `executable` its execute bit (Unix), `pathext` Windows'
/// `PATHEXT`.
pub fn runs_as_program(name: &str, executable: bool, os: Os, pathext: &str) -> bool {
    let ext = |name: &str| {
        name.rsplit_once('.')
            .map(|(_, e)| format!(".{}", e.to_ascii_lowercase()))
    };
    match os {
        Os::Windows => {
            // Windows drops trailing dots and spaces from a name.
            let Some(e) = ext(name.trim_end_matches(['.', ' '])) else {
                return false;
            };
            pathext
                .split(';')
                .map(|p| p.trim().to_ascii_lowercase())
                .filter(|p| !p.is_empty())
                .any(|p| p == e)
                || [
                    ".msi", ".lnk", ".scr", ".ps1", ".vbs", ".wsf", ".hta", ".reg", ".url", ".jar",
                    ".cpl", ".pif",
                ]
                .contains(&e.as_str())
        }
        Os::MacOs => {
            executable
                || ext(name).is_some_and(|e| {
                    [".app", ".command", ".pkg", ".terminal", ".tool"].contains(&e.as_str())
                })
        }
        Os::Linux => {
            executable || ext(name).is_some_and(|e| [".desktop", ".appimage"].contains(&e.as_str()))
        }
    }
}

/// [`runs_as_program`] for a file on this computer. A file that cannot be
/// read counts as a program.
pub fn is_program(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    #[cfg(unix)]
    let executable = {
        use std::os::unix::fs::PermissionsExt as _;
        match fs::metadata(path) {
            Ok(m) => !m.is_dir() && m.permissions().mode() & 0o111 != 0,
            Err(_) => return true,
        }
    };
    #[cfg(not(unix))]
    let executable = false;
    let pathext = std::env::var("PATHEXT").unwrap_or_else(|_| DEFAULT_PATHEXT.to_owned());
    runs_as_program(&name, executable, Os::CURRENT, &pathext)
}

/// A file's size and modification time, checked every 2 s by the viewer
/// and the tree to read again only what changed (5.R.3 and 5.R.4, Staying
/// current). `None` when it is gone.
pub fn stamp(path: &Path) -> Option<(u64, std::time::SystemTime)> {
    let m = fs::metadata(path).ok()?;
    Some((m.len(), m.modified().ok()?))
}
