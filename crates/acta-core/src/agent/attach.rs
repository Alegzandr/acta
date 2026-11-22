//! Files for the agent (ADR 0012, 10): the `· files:` segment of list lines
//! and the `attachment` tool. Names and sizes only, everywhere, unless the
//! agent asks for one file; then its text, a PDF's text, or one image
//! within the pixel caps, never raw bytes.

use super::args::AttachmentArgs;
use super::render::{self, Fence, Text, budget};
use super::tools::{Agent, err};
use crate::files::{self, PdfPages, PdfText};
use crate::model::{AttachOwner, Attachment, AttachmentId, FileKind};
use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

/// What the `attachment` tool returns: text, and for an image the
/// re-encoded picture with its media type.
pub struct Answer {
    pub text: String,
    pub image: Option<(Vec<u8>, &'static str)>,
}

pub type AttachReply = Result<Answer, String>;

/// The `attachment` tool in two halves, so that the slow part runs without
/// the store (the MCP server holds its state lock only for the first):
/// [`Agent::attachment_step`] checks and reads under the lock, and
/// [`Step::finish`] parses a PDF (in a child process) or decodes an image
/// after it.
pub enum Step {
    Done(Answer),
    Pdf(PdfJob),
    Image(ImageJob),
}

impl Step {
    pub fn finish(self) -> AttachReply {
        match self {
            Step::Done(a) => Ok(a),
            Step::Pdf(j) => j.finish(),
            Step::Image(j) => Ok(j.finish()),
        }
    }
}

/// A PDF to read: which pages, and where its text comes from.
pub struct PdfJob {
    id: AttachmentId,
    hash: String,
    head: String,
    name: String,
    from: u32,
    to: u32,
    /// `pages` was given (else: the first pages that fit).
    asked: bool,
    /// The content, read only when no worker is set (tests): the parser
    /// then runs in this process.
    bytes: Option<Vec<u8>>,
}

/// An image to send, read under the lock, decoded after it.
pub struct ImageJob {
    id: AttachmentId,
    head: String,
    name: String,
    size: i64,
    pixels: Option<(u32, u32)>,
    media_type: String,
    max: u32,
    bytes: Vec<u8>,
}

/// Limits of one `acta pdf-text` child (ADR 0012, 10, as amended by the
/// review): time, memory of its whole tree, and output.
pub const PDF_TIME: Duration = Duration::from_secs(10);
pub const PDF_MEMORY: u64 = 512 * 1024 * 1024;
pub const PDF_OUTPUT: usize = 1024 * 1024;
/// Characters a PDF read collects: a little more than the budget shows.
const PDF_CHARS: usize = budget::KB_READ + 2_000;
/// Pages a read without `pages` looks at.
const PDF_DEFAULT_PAGES: u32 = 30;
/// Characters of PDF text kept in memory across calls, for all files.
const PDF_CACHE_CHARS: usize = 2 * 1024 * 1024;

static PDF_WORKER: OnceLock<PathBuf> = OnceLock::new();

/// The executable whose `pdf-text` command parses PDFs (the CLI's own
/// path, set by `main`). Unset (unit tests), the parser runs in-process.
pub fn set_pdf_worker(exe: PathBuf) {
    let _ = PDF_WORKER.set(exe);
}

/// Extracted pages per content hash, for this process: a second read of
/// the same pages runs no parser. Plaintext in memory only, like the rest
/// of what the store reads.
#[derive(Default)]
struct PdfCache {
    docs: HashMap<String, (u32, BTreeMap<u32, String>)>,
    chars: usize,
}

fn cache() -> &'static Mutex<PdfCache> {
    static CACHE: OnceLock<Mutex<PdfCache>> = OnceLock::new();
    CACHE.get_or_init(Mutex::default)
}

fn lock_cache() -> std::sync::MutexGuard<'static, PdfCache> {
    match cache().lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    }
}

/// Runs `acta pdf-text` on one attachment with the limits above. A
/// timeout, a crash, a memory failure or too much output: unreadable.
pub fn pdf_pages_in_child(exe: &std::path::Path, id: AttachmentId, from: u32, to: u32) -> PdfText {
    let mut cmd = std::process::Command::new(exe);
    cmd.args([
        "pdf-text".to_owned(),
        id.to_string(),
        "--from".into(),
        from.to_string(),
        "--to".into(),
        to.to_string(),
        "--max-chars".into(),
        PDF_CHARS.to_string(),
    ]);
    let limits = crate::process::Limits {
        time: PDF_TIME,
        memory: Some(PDF_MEMORY),
        output: Some(PDF_OUTPUT),
    };
    match crate::process::output_limited(&mut cmd, limits) {
        Ok(Some(out)) if out.status.success() => serde_json::from_slice::<PdfPages>(&out.stdout)
            .map_or(PdfText::Unreadable, PdfText::Pages),
        _ => PdfText::Unreadable,
    }
}

impl PdfJob {
    /// The pages asked, from the cache where it has them, the rest from
    /// the parser; then the reply.
    pub fn finish(self) -> AttachReply {
        let mut got: Vec<(u32, String)> = Vec::new();
        let mut used = 0usize;
        let mut count: Option<u32> = None;
        let mut next = self.from;
        {
            let c = lock_cache();
            if let Some((n, pages)) = c.docs.get(&self.hash) {
                count = Some(*n);
                while next <= self.to.min(*n) && used < PDF_CHARS {
                    let Some(t) = pages.get(&next) else { break };
                    used += t.chars().count();
                    got.push((next, t.clone()));
                    next += 1;
                }
            }
        }
        let done = count.is_some_and(|n| next > self.to.min(n)) || used >= PDF_CHARS;
        if !done {
            let fresh = match (&self.bytes, PDF_WORKER.get()) {
                (Some(b), _) => files::pdf_pages(b, next, self.to, PDF_CHARS - used),
                (None, Some(exe)) => pdf_pages_in_child(exe, self.id, next, self.to),
                (None, None) => PdfText::Unreadable,
            };
            let PdfText::Pages(fresh) = fresh else {
                return Ok(one_line(
                    &format!("{} · could not read this PDF", self.head),
                    &self.name,
                ));
            };
            count = Some(fresh.count);
            let mut c = lock_cache();
            if c.chars > PDF_CACHE_CHARS {
                *c = PdfCache::default();
            }
            let added: usize = fresh.pages.iter().map(|(_, t)| t.chars().count()).sum();
            c.chars += added;
            let entry = c
                .docs
                .entry(self.hash.clone())
                .or_insert_with(|| (fresh.count, BTreeMap::new()));
            for (p, t) in &fresh.pages {
                entry.1.insert(*p, t.clone());
            }
            got.extend(fresh.pages);
        }
        let count = count.unwrap_or(0);
        self.render(count, &got)
    }

    fn render(&self, count: u32, got: &[(u32, String)]) -> AttachReply {
        let head = &self.head;
        let name = &self.name;
        if self.from > count {
            return Err(format!("{} has {count} pages", self.id));
        }
        let to = self.to.min(count);
        if got.iter().all(|(_, t)| t.trim().is_empty()) && got.len() as u32 > to - self.from {
            let what = if self.from == 1 && to == count {
                "no text layer (scanned images); not readable here".to_owned()
            } else {
                format!("no text in pages {}-{to}", self.from)
            };
            return Ok(one_line(&format!("{head} · {count} pages · {what}"), name));
        }
        const FOOT_ROOM: usize = 90;
        let mut out = Text::new(budget::KB_READ - FOOT_ROOM);
        out.line_clipped(format!("{head} · {count} pages"));
        out.open_fence(&Fence::new());
        out.line_clipped(name);
        let mut cut: Option<u32> = None;
        let mut last_full = self.from - 1;
        'pages: for (p, text) in got {
            if !out.line(format!("--- page {p} ---")) {
                cut = Some(*p);
                break;
            }
            for l in text.lines().filter(|l| !l.trim().is_empty()) {
                for chunk in l.chars().collect::<Vec<_>>().chunks(WRAP) {
                    if !out.line(chunk.iter().collect::<String>()) {
                        cut = Some(*p);
                        break 'pages;
                    }
                }
            }
            last_full = *p;
        }
        let mut text = out.finish();
        let last = if self.asked { to } else { count };
        match cut {
            Some(p) => text.push_str(&format!(
                "\n[cut in page {p}: read on with pages=\"{p}-{last}\"]"
            )),
            None if last_full < last => text.push_str(&format!(
                "\n[pages {}-{last_full} of {count}: more with pages=\"{}-{last}\"]",
                self.from,
                last_full + 1
            )),
            None => {}
        }
        Ok(Answer {
            text: render::finish(text, budget::KB_READ),
            image: None,
        })
    }
}

impl ImageJob {
    /// Decodes and re-encodes within the agent's allowance
    /// ([`files::MODEL_MAX_ALLOC`]); an image over it gets one line.
    pub fn finish(self) -> Answer {
        let Some(img) = files::for_model(&self.bytes, &self.media_type, self.max) else {
            return one_line(
                &format!("{} · could not decode this image here", self.head),
                &self.name,
            );
        };
        let pixels = self
            .pixels
            .map_or(String::new(), |(w, h)| format!(" · {w}x{h}"));
        let line = format!(
            "{} · image{pixels} · {} · sent {}x{}",
            self.id,
            files::size_label(self.size),
            img.width,
            img.height
        );
        Answer {
            text: render::finish(
                format!("{line}\n{}", Fence::new().wrap(&self.name)),
                budget::KB_READ,
            ),
            image: Some((img.bytes, img.media_type)),
        }
    }
}

/// Longest file name in a list line.
const NAME_IN_LIST: usize = 40;
/// Characters of a text line before it is split (as `kb_read` counts).
const WRAP: usize = 400;
/// Display lines kept from one range; more than any budget shows.
const KEEP_LINES: usize = 400;

/// ` · files: A-4 plan.pdf 2.1M, A-5 IMG_2041.jpg 3.4M +1`: at most `shown`
/// files, names cut to 40 chars; empty when there is none. Names are stored
/// text: callers put the line inside a fence, or defuse it as here.
pub fn files_segment(list: &[Attachment], shown: usize) -> String {
    if list.is_empty() {
        return String::new();
    }
    let names: Vec<String> = list
        .iter()
        .take(shown)
        .map(|a| {
            format!(
                "{} {} {}",
                a.id,
                render::defuse(&render::clip(&render::one_line(&a.name), NAME_IN_LIST)),
                files::short_size(a.size)
            )
        })
        .collect();
    let more = if list.len() > shown {
        format!(" +{}", list.len() - shown)
    } else {
        String::new()
    };
    format!(" · files: {}{more}", names.join(", "))
}

/// The segment for one owner from a batch read.
pub fn segment_of(
    map: &HashMap<AttachOwner, Vec<Attachment>>,
    owner: AttachOwner,
    shown: usize,
) -> String {
    map.get(&owner)
        .map_or(String::new(), |l| files_segment(l, shown))
}

/// `1-80` or `3`: a 1-based inclusive range.
fn range(text: &str, what: &str) -> Result<(usize, usize), String> {
    let t = text.trim();
    let (a, b) = t.split_once('-').unwrap_or((t, t));
    let (a, b) = (a.trim().parse::<usize>(), b.trim().parse::<usize>());
    match (a, b) {
        (Ok(a), Ok(b)) if a >= 1 && a <= b => Ok((a, b)),
        _ => Err(format!("{what} must look like 3-5")),
    }
}

fn kind_word(k: FileKind) -> &'static str {
    match k {
        FileKind::Image => "image",
        FileKind::Text => "text",
        FileKind::Pdf => "PDF",
        FileKind::Other => "file",
    }
}

impl Agent<'_> {
    /// `attachment(id, lines?, pages?, size?)`: one file of this project,
    /// both halves in a row (the CLI and tests).
    pub fn attachment(&mut self, a: AttachmentArgs) -> AttachReply {
        self.attachment_step(a)?.finish()
    }

    /// The first half of `attachment`: checks, text files, and what the
    /// second half needs.
    pub fn attachment_step(&mut self, a: AttachmentArgs) -> Result<Step, String> {
        self.project()?;
        let id = AttachmentId::parse(&a.id)
            .filter(|_| a.id.trim().to_ascii_uppercase().starts_with("A-"))
            .ok_or_else(|| format!("{} is not a file id (A-n)", render::clip(&a.id, 20)))?;
        let att = self.store.attachment(id).map_err(err)?;
        self.check_project(att.project, &id.to_string())?;
        let mismatch = |param: &str, wanted: &str| -> Result<Step, String> {
            Err(format!(
                "{param} applies to {wanted}; {id} is {}",
                match att.kind {
                    FileKind::Image => "an image",
                    FileKind::Text => "text (use lines)",
                    FileKind::Pdf => "a PDF (use pages)",
                    FileKind::Other => "binary",
                }
            ))
        };
        if a.lines.is_some() && att.kind != FileKind::Text {
            return mismatch("lines", "text files");
        }
        if a.pages.is_some() && att.kind != FileKind::Pdf {
            return mismatch("pages", "PDFs");
        }
        if a.size.is_some() && att.kind != FileKind::Image {
            return mismatch("size", "images");
        }
        let head = format!(
            "{id} · {} · {}",
            kind_word(att.kind),
            files::size_label(att.size)
        );
        let name = format!(
            "Name: {}",
            render::clip(&render::one_line(&att.name), files_name_max())
        );
        match att.kind {
            FileKind::Text => self
                .text_file(&att, &head, &name, a.lines.as_deref())
                .map(Step::Done),
            FileKind::Pdf => self.pdf_job(&att, head, name, a.pages.as_deref()),
            FileKind::Image => self.image_job(&att, head, name, a.size.as_deref()),
            FileKind::Other => Ok(Step::Done(binary(&head, &name))),
        }
    }

    fn text_file(
        &self,
        att: &Attachment,
        head: &str,
        name: &str,
        lines: Option<&str>,
    ) -> AttachReply {
        let (from, to) = match lines {
            Some(r) => range(r, "lines")?,
            None => (1, usize::MAX),
        };
        // One pass over the content, a chunk in memory at a time: count
        // every display line, keep the requested ones.
        let reader = self.store.read_attachment(att.id).map_err(err)?;
        let mut buf = BufReader::new(reader);
        let mut raw = Vec::new();
        let mut total = 0usize;
        let mut part: Vec<String> = Vec::new();
        loop {
            raw.clear();
            let n = buf.read_until(b'\n', &mut raw).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            let line = String::from_utf8_lossy(&raw);
            let line = line.trim_end_matches(['\n', '\r']);
            let chars: Vec<char> = line.chars().collect();
            let pieces = chars.len().div_ceil(WRAP).max(1);
            for i in 0..pieces {
                total += 1;
                if total >= from && total <= to && part.len() < KEEP_LINES {
                    let end = ((i + 1) * WRAP).min(chars.len());
                    part.push(chars[(i * WRAP).min(end)..end].iter().collect());
                }
            }
        }
        if from > total.max(1) {
            return Err(format!("{} has {total} lines", att.id));
        }
        let what = match lines {
            Some(_) => format!("lines {from}-{} of {total}", to.min(total)),
            None => format!("{total} lines"),
        };
        Ok(Answer {
            text: super::tools::cut_body(head, &what, name, &part, from, total, budget::KB_READ),
            image: None,
        })
    }

    fn pdf_job(
        &self,
        att: &Attachment,
        head: String,
        name: String,
        pages: Option<&str>,
    ) -> Result<Step, String> {
        let (from, to) = match pages {
            Some(r) => {
                let (a, b) = range(r, "pages")?;
                (a as u32, b.min(u32::MAX as usize) as u32)
            }
            None => (1, PDF_DEFAULT_PAGES),
        };
        let hash = self.store.attachment_hash(att.id).map_err(err)?;
        let bytes = match PDF_WORKER.get() {
            Some(_) => None,
            None => Some(
                self.store
                    .attachment_prefix(att.id, att.size.max(0) as usize)
                    .map_err(err)?,
            ),
        };
        Ok(Step::Pdf(PdfJob {
            id: att.id,
            hash,
            head,
            name,
            from,
            to,
            asked: pages.is_some(),
            bytes,
        }))
    }

    fn image_job(
        &self,
        att: &Attachment,
        head: String,
        name: String,
        size: Option<&str>,
    ) -> Result<Step, String> {
        let max = match size.map(|s| s.trim().to_ascii_lowercase()) {
            None => files::MODEL_PX,
            Some(s) if s == "full" => files::MODEL_FULL_PX,
            Some(s) if s == "default" => files::MODEL_PX,
            Some(_) => return Err("size must be full (or omitted)".into()),
        };
        let bytes = self
            .store
            .attachment_prefix(att.id, att.size.max(0) as usize)
            .map_err(err)?;
        Ok(Step::Image(ImageJob {
            id: att.id,
            head,
            name,
            size: att.size,
            pixels: att.pixels,
            media_type: att.media_type.clone(),
            max,
            bytes,
        }))
    }
}

/// The name is stored text: inside the fence.
fn files_name_max() -> usize {
    200
}

fn one_line(head: &str, name: &str) -> Answer {
    Answer {
        text: render::finish(format!("{head}\n{}", Fence::new().wrap(name)), 600),
        image: None,
    }
}

fn binary(head: &str, name: &str) -> Answer {
    one_line(&format!("{head} · binary, not readable here"), name)
}
