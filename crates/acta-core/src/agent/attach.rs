//! Files for the agent (ADR 0012, 10): the `· files:` segment of list lines
//! and the `attachment` tool. Names and sizes only, everywhere, unless the
//! agent asks for one file; then its text, a PDF's text, or one image
//! within the pixel caps, never raw bytes.

use super::args::AttachmentArgs;
use super::render::{self, Fence, Text, budget};
use super::tools::{Agent, err};
use crate::files::{self, PdfText};
use crate::model::{AttachOwner, Attachment, AttachmentId, FileKind};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};

/// What the `attachment` tool returns: text, and for an image the
/// re-encoded picture with its media type.
pub struct Answer {
    pub text: String,
    pub image: Option<(Vec<u8>, &'static str)>,
}

pub type AttachReply = Result<Answer, String>;

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
    /// `attachment(id, lines?, pages?, size?)`: one file of this project.
    pub fn attachment(&mut self, a: AttachmentArgs) -> AttachReply {
        self.project()?;
        let id = AttachmentId::parse(&a.id)
            .filter(|_| a.id.trim().to_ascii_uppercase().starts_with("A-"))
            .ok_or_else(|| format!("{} is not a file id (A-n)", render::clip(&a.id, 20)))?;
        let att = self.store.attachment(id).map_err(err)?;
        self.check_project(att.project, &id.to_string())?;
        let mismatch = |param: &str, wanted: &str| {
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
            FileKind::Text => self.text_file(&att, &head, &name, a.lines.as_deref()),
            FileKind::Pdf => self.pdf_file(&att, &head, &name, a.pages.as_deref()),
            FileKind::Image => self.image_file(&att, &head, &name, a.size.as_deref()),
            FileKind::Other => Ok(binary(&head, &name)),
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

    fn pdf_file(
        &self,
        att: &Attachment,
        head: &str,
        name: &str,
        pages: Option<&str>,
    ) -> AttachReply {
        let bytes = self
            .store
            .attachment_prefix(att.id, att.size.max(0) as usize)
            .map_err(err)?;
        let all = match files::pdf_text(&bytes) {
            PdfText::Pages(p) => p,
            PdfText::Unreadable => {
                return Ok(one_line(
                    &format!("{head} · could not read its text (damaged or encrypted)"),
                    name,
                ));
            }
        };
        let count = all.len();
        if all.iter().all(|p| p.trim().is_empty()) {
            return Ok(one_line(
                &format!(
                    "{head} · {count} pages · no text layer (scanned images); not readable here"
                ),
                name,
            ));
        }
        let (from, to) = match pages {
            Some(r) => range(r, "pages")?,
            None => (1, count),
        };
        if from > count {
            return Err(format!("{} has {count} pages", att.id));
        }
        let to = to.min(count);
        const FOOT_ROOM: usize = 90;
        let mut out = Text::new(budget::KB_READ - FOOT_ROOM);
        out.line_clipped(format!("{head} · {count} pages"));
        out.open_fence(&Fence::new());
        out.line_clipped(name);
        let mut cut: Option<usize> = None;
        let mut last_full = from - 1;
        'pages: for p in from..=to {
            if !out.line(format!("--- page {p} ---")) {
                cut = Some(p);
                break;
            }
            for l in all[p - 1].lines().filter(|l| !l.trim().is_empty()) {
                for chunk in l.chars().collect::<Vec<_>>().chunks(WRAP) {
                    if !out.line(chunk.iter().collect::<String>()) {
                        cut = Some(p);
                        break 'pages;
                    }
                }
            }
            last_full = p;
        }
        let mut text = out.finish();
        match cut {
            Some(p) => text.push_str(&format!(
                "\n[cut in page {p}: read on with pages=\"{p}-{to}\"]"
            )),
            None if to < count => text.push_str(&format!(
                "\n[pages {from}-{last_full} of {count}: more with pages=\"{}-{count}\"]",
                last_full + 1
            )),
            None => {}
        }
        Ok(Answer {
            text: render::finish(text, budget::KB_READ),
            image: None,
        })
    }

    fn image_file(
        &self,
        att: &Attachment,
        head: &str,
        name: &str,
        size: Option<&str>,
    ) -> AttachReply {
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
        let Some(img) = files::for_model(&bytes, &att.media_type, max) else {
            return Ok(binary(head, name));
        };
        let pixels = att
            .pixels
            .map_or(String::new(), |(w, h)| format!(" · {w}x{h}"));
        let line = format!(
            "{} · image{pixels} · {} · sent {}x{}",
            att.id,
            files::size_label(att.size),
            img.width,
            img.height
        );
        Ok(Answer {
            text: render::finish(
                format!("{line}\n{}", Fence::new().wrap(name)),
                budget::KB_READ,
            ),
            image: Some((img.bytes, img.media_type)),
        })
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
