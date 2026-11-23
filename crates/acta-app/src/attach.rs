//! Files in the interface (docs/design/components-attachments.md 5; ADR
//! 0012): how sizes and names are shown, what a paste attaches, the files
//! an editor is adding, and where the viewer stands. No GPUI here: the
//! views are in `attach_ui`.

use std::path::{Path, PathBuf};

use acta_core::model::{Attachment, FileKind};
use acta_core::store::{LARGE_FILE_BYTES, MAX_FILE_BYTES, MAX_FILES_PER_OWNER, Staged};

use crate::i18n::time::LocalTime;
use crate::i18n::{Arg, Key, Locale, percent, text, tr};

/// A file name in a chip: at most this many characters (the chip's name
/// is at most 200 wide in `type.ui`).
pub const CHIP_NAME_CHARS: usize = 28;
/// The text viewer shows this much of a file (ADR 0012, 6).
pub const TEXT_VIEW_BYTES: usize = 1024 * 1024;
/// A file still being stored after this long shows its progress.
pub const PROGRESS_AFTER_MS: u64 = 400;

/// A size as the system file managers on Windows show it: bytes under
/// 1 KB, then KB, MB and GB with one decimal under 10 and none above,
/// 1 KB = 1,024 bytes ("812 B", "2.1 MB", "12 MB"; "2,1 Mo" in French,
/// with U+202F before the unit).
pub fn size_label(locale: Locale, bytes: u64) -> String {
    let (space, units): (&str, [&str; 4]) = match locale {
        Locale::En => (" ", ["B", "KB", "MB", "GB"]),
        Locale::Fr => ("\u{202f}", ["o", "Ko", "Mo", "Go"]),
    };
    if bytes < 1024 {
        return format!("{bytes}{space}{}", units[0]);
    }
    let mut value = bytes as f64 / 1024.;
    let mut unit = 1;
    loop {
        let tenths = (value * 10.).round() / 10.;
        let whole = value.round();
        let shown_whole = tenths >= 10.;
        if (shown_whole && whole >= 1024. || !shown_whole && tenths >= 1024.) && unit < 3 {
            value /= 1024.;
            unit += 1;
            continue;
        }
        let number = if shown_whole {
            format!("{whole:.0}")
        } else {
            let s = format!("{tenths:.1}");
            match locale {
                Locale::En => s,
                Locale::Fr => s.replace('.', ","),
            }
        };
        return format!("{number}{space}{}", units[unit]);
    }
}

/// A name cut in the middle so the extension stays ("Quarterly-rev…al.pdf"),
/// at most `max` characters.
pub fn middle_name(name: &str, max: usize) -> String {
    let count = name.chars().count();
    if count <= max {
        return name.to_owned();
    }
    let ext: String = match name.rsplit_once('.') {
        Some((stem, e)) if !stem.is_empty() && !e.is_empty() && e.chars().count() <= 8 => {
            format!(".{e}")
        }
        _ => String::new(),
    };
    let ext_len = ext.chars().count();
    let stem: Vec<char> = name.chars().take(count - ext_len).collect();
    let keep = max.saturating_sub(ext_len + 1).max(2);
    let tail = (keep / 3).min(stem.len());
    let head = keep - tail;
    let mut out: String = stem[..head.min(stem.len())].iter().collect();
    out.push('\u{2026}');
    out.extend(&stem[stem.len() - tail..]);
    out.push_str(&ext);
    out
}

/// The name of a pasted screenshot: "Pasted image 2026-11-21 14.05.png";
/// the date and time are never localised (a file name, 10.2), and dots
/// stand for the colon Windows refuses in file names.
pub fn pasted_name(locale: Locale, at: LocalTime) -> String {
    let date = format!("{:04}-{:02}-{:02}", at.year, at.month, at.day);
    let time = format!("{:02}.{:02}", at.hour, at.minute);
    tr(
        locale,
        Key::AttachPastedName,
        &[("date", Arg::Text(&date)), ("time", Arg::Text(&time))],
    )
}

/// The name a path gives a file.
pub fn name_of(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".into())
}

/// What `Mod+V` does in an editor that takes files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Paste {
    /// Text, whatever else the clipboard holds: the words were meant.
    Text,
    /// Files copied in a file manager.
    Files(Vec<PathBuf>),
    /// An image and no text (a screenshot): attached as a PNG.
    Image,
}

/// What a paste does, from what the clipboard holds.
pub fn paste_kind(has_text: bool, files: &[PathBuf], has_image: bool) -> Paste {
    if has_text {
        Paste::Text
    } else if !files.is_empty() {
        Paste::Files(files.to_vec())
    } else if has_image {
        Paste::Image
    } else {
        Paste::Text
    }
}

/// Why a file was not added (one line each under the strip).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    TooLarge { name: String, size: u64 },
    TooMany,
    ReadFailed { name: String },
}

impl Refusal {
    pub fn message(&self, locale: Locale) -> String {
        match self {
            Refusal::TooLarge { name, size } => tr(
                locale,
                Key::AttachTooLarge,
                &[
                    ("name", Arg::Text(name)),
                    ("size", Arg::Text(&size_label(locale, *size))),
                ],
            ),
            Refusal::TooMany => text(locale, Key::AttachTooMany),
            Refusal::ReadFailed { name } => {
                tr(locale, Key::AttachReadFailed, &[("name", Arg::Text(name))])
            }
        }
    }
}

/// A file an editor is adding: shown at once, stored in the background.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DraftFile {
    pub key: u64,
    pub name: String,
    /// The paste, drop or dialog it came with (one undo step each).
    pub group: u64,
    /// Known once the file was looked at.
    pub size: Option<u64>,
    pub kind: Option<FileKind>,
    pub done: u64,
    pub started_ms: u64,
    pub staged: Option<Staged>,
}

impl DraftFile {
    /// "Adding… 40%" once it has taken over 400 ms, else nothing.
    pub fn adding_label(&self, locale: Locale, now_ms: u64) -> Option<String> {
        if self.staged.is_some() || now_ms.saturating_sub(self.started_ms) < PROGRESS_AFTER_MS {
            return None;
        }
        let pct = match self.size {
            Some(total) if total > 0 => (self.done.min(total) * 100 / total).min(99),
            _ => 0,
        };
        Some(tr(
            locale,
            Key::AttachAdding,
            &[("percent", Arg::Text(&percent(locale, pct)))],
        ))
    }

    pub fn is_image(&self) -> bool {
        self.kind == Some(FileKind::Image)
    }
}

/// The files an editor is adding, its refusals and its one-time notice
/// (components-attachments.md 5, Adding files).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Draft {
    pub files: Vec<DraftFile>,
    pub refusals: Vec<Refusal>,
    /// "Files over 10 MB…" was shown in this editor session.
    pub notice: bool,
    /// Files the record already holds (an editor on a saved note or entry).
    pub held: usize,
    next_key: u64,
    next_group: u64,
}

impl Draft {
    /// A paste, drop or dialog starts: the refusals of the last add go.
    pub fn start_group(&mut self) -> u64 {
        self.refusals.clear();
        self.next_group += 1;
        self.next_group
    }

    /// A file of `group` appears at once; refused when the owner would
    /// hold more than 20.
    pub fn begin(&mut self, group: u64, name: String, now_ms: u64) -> Option<u64> {
        if self.held + self.files.len() >= MAX_FILES_PER_OWNER as usize {
            if !self.refusals.contains(&Refusal::TooMany) {
                self.refusals.push(Refusal::TooMany);
            }
            return None;
        }
        self.next_key += 1;
        self.files.push(DraftFile {
            key: self.next_key,
            name,
            group,
            size: None,
            kind: None,
            done: 0,
            started_ms: now_ms,
            staged: None,
        });
        Some(self.next_key)
    }

    /// Its size is known: refused over 25 MB; the notice once over 10 MB.
    /// `false` when it was refused (and removed).
    pub fn sized(&mut self, key: u64, size: u64) -> bool {
        let Some(at) = self.files.iter().position(|f| f.key == key) else {
            return false;
        };
        if size > MAX_FILE_BYTES {
            let f = self.files.remove(at);
            self.refusals.push(Refusal::TooLarge { name: f.name, size });
            return false;
        }
        if size > LARGE_FILE_BYTES {
            self.notice = true;
        }
        self.files[at].size = Some(size);
        true
    }

    pub fn progress(&mut self, key: u64, done: u64) {
        if let Some(f) = self.files.iter_mut().find(|f| f.key == key) {
            f.done = done;
        }
    }

    pub fn ready(&mut self, key: u64, staged: Staged) {
        if let Some(f) = self.files.iter_mut().find(|f| f.key == key) {
            f.size = Some(staged.size());
            f.kind = Some(staged.kind());
            f.done = staged.size();
            f.staged = Some(staged);
        }
    }

    /// The file could not be stored: removed, with its refusal.
    pub fn failed(&mut self, key: u64, refusal: Refusal) {
        self.files.retain(|f| f.key != key);
        self.refusals.push(refusal);
    }

    /// Taken out of the editor (Remove on a file not saved yet).
    pub fn remove(&mut self, key: u64) -> Option<DraftFile> {
        let at = self.files.iter().position(|f| f.key == key)?;
        Some(self.files.remove(at))
    }

    /// The editor starts over (saved or cancelled); keys keep counting so a
    /// late result of an old file never lands on a new one.
    pub fn clear(&mut self, keep_notice: bool) {
        self.files.clear();
        self.refusals.clear();
        self.notice &= keep_notice;
    }

    /// The files were handed to the record being saved; their refusals
    /// and the notice stay until the next add.
    pub fn hand_over(&mut self) -> Vec<DraftFile> {
        std::mem::take(&mut self.files)
    }

    /// Whether every file is stored.
    pub fn settled(&self) -> bool {
        self.files.iter().all(|f| f.staged.is_some())
    }

    /// Images first, then the others, each in the order they came.
    pub fn ordered(&self) -> Vec<&DraftFile> {
        let (images, others): (Vec<&DraftFile>, Vec<&DraftFile>) =
            self.files.iter().partition(|f| f.is_image());
        images.into_iter().chain(others).collect()
    }
}

/// A strip's files: images first, then the others, each in the order
/// they were added.
/// A pan of `d` (the content moves the other way) from the scroll offset
/// `o`, kept inside `0` to `-max` on each axis: what `Shift`+arrows and a
/// drag at 100% do in the viewer (components-attachments.md 5, Image).
pub fn panned(o: (f32, f32), max: (f32, f32), d: (f32, f32)) -> (f32, f32) {
    (
        (o.0 - d.0).clamp(-max.0.max(0.), 0.),
        (o.1 - d.1).clamp(-max.1.max(0.), 0.),
    )
}

pub fn strip_order(files: &[Attachment]) -> Vec<&Attachment> {
    let (images, others): (Vec<&Attachment>, Vec<&Attachment>) =
        files.iter().partition(|f| f.kind == FileKind::Image);
    images.into_iter().chain(others).collect()
}

/// Whether the viewer shows it (images and text-like files).
pub fn viewable(file: &Attachment) -> bool {
    matches!(file.kind, FileKind::Image | FileKind::Text)
}

/// The files the viewer steps through from `file`: its owner's viewable
/// files in strip order, and where `file` is among them.
pub fn viewer_list(files: &[Attachment], file: &Attachment) -> (Vec<Attachment>, usize) {
    let list: Vec<Attachment> = strip_order(files)
        .into_iter()
        .filter(|f| viewable(f))
        .cloned()
        .collect();
    let at = list.iter().position(|f| f.id == file.id).unwrap_or(0);
    (list, at)
}

/// The viewer's next (`forward`) or previous file, staying at the ends.
pub fn step(at: usize, len: usize, forward: bool) -> usize {
    if forward {
        (at + 1).min(len.saturating_sub(1))
    } else {
        at.saturating_sub(1)
    }
}

/// "3.4 MB · 3024 x 4032" (the pixel size for images).
pub fn viewer_meta(locale: Locale, file: &Attachment) -> String {
    let size = size_label(locale, file.size.max(0) as u64);
    match file.pixels {
        Some((w, h)) => format!("{size} \u{b7} {w} x {h}"),
        None => size,
    }
}

/// The scale that fits `image` inside `area`, never above 100%.
pub fn fit_scale(image: (f32, f32), area: (f32, f32)) -> f32 {
    if image.0 <= 0. || image.1 <= 0. {
        return 1.;
    }
    (area.0 / image.0).min(area.1 / image.1).clamp(0., 1.)
}

/// The text viewer's content from the first bytes of a file of `size`:
/// at most [`TEXT_VIEW_BYTES`], cut on a character, and whether the file
/// goes on past it.
pub fn text_view(prefix: &[u8], size: u64) -> (String, bool) {
    let cut = &prefix[..prefix.len().min(TEXT_VIEW_BYTES)];
    let valid = match std::str::from_utf8(cut) {
        Ok(_) => cut.len(),
        // A character cut by the end of the window, or bytes that are not
        // UTF-8 further on (shown replaced).
        Err(e) if e.error_len().is_none() => e.valid_up_to(),
        Err(_) => cut.len(),
    };
    let text = String::from_utf8_lossy(&cut[..valid]).into_owned();
    (text, size > valid as u64)
}

/// Settings, Data: "Attached files: 312 MB in 48 files".
pub fn totals_line(locale: Locale, bytes: u64, files: u64) -> String {
    let count = tr(locale, Key::AttachCount, &[("n", Arg::Count(files))]);
    tr(
        locale,
        Key::SettingsAttachments,
        &[
            ("size", Arg::Text(&size_label(locale, bytes))),
            ("count", Arg::Text(&count)),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use acta_core::model::{AttachOwner, AttachmentId, Author, NoteId, ProjectId, Stamp};

    fn stamp() -> Stamp {
        Stamp {
            uuid: String::new(),
            created_at: 0,
            updated_at: 0,
            updated_by: Author::Human,
            deleted_at: None,
            deleted_by: None,
        }
    }

    #[test]
    fn sizes_read_as_file_managers_show_them() {
        let en = |b| size_label(Locale::En, b);
        assert_eq!(en(0), "0 B");
        assert_eq!(en(812), "812 B");
        assert_eq!(en(1023), "1023 B");
        assert_eq!(en(1024), "1.0 KB");
        assert_eq!(en(340 * 1024), "340 KB");
        assert_eq!(en(2_202_010), "2.1 MB");
        assert_eq!(en(12 * 1024 * 1024 + 300), "12 MB");
        assert_eq!(en(48 * 1024 * 1024), "48 MB");
        assert_eq!(en(25 * 1024 * 1024), "25 MB");
        // 1023.9 KB rounds to the next unit, never "1024 KB".
        assert_eq!(en(1024 * 1024 - 10), "1.0 MB");
        assert_eq!(en(312 * 1024 * 1024), "312 MB");
        assert_eq!(en(3 * 1024 * 1024 * 1024), "3.0 GB");
        let fr = |b| size_label(Locale::Fr, b);
        assert_eq!(fr(812), "812\u{202f}o");
        assert_eq!(fr(340 * 1024), "340\u{202f}Ko");
        assert_eq!(fr(2_202_010), "2,1\u{202f}Mo");
        assert_eq!(fr(12 * 1024 * 1024), "12\u{202f}Mo");
    }

    #[test]
    fn names_are_cut_in_the_middle_and_keep_their_extension() {
        assert_eq!(middle_name("notes.md", 28), "notes.md");
        let cut = middle_name("Quarterly-review-of-the-vendor-final.pdf", 28);
        assert_eq!(cut.chars().count(), 28);
        assert!(cut.starts_with("Quarterly-"), "{cut}");
        assert!(cut.ends_with("final.pdf"), "{cut}");
        assert!(cut.contains('\u{2026}'));
        // No extension: cut all the same.
        let plain = middle_name(&"x".repeat(60), 28);
        assert_eq!(plain.chars().count(), 28);
        // Characters, not bytes.
        let accents = middle_name(&format!("{}.txt", "é".repeat(40)), 20);
        assert_eq!(accents.chars().count(), 20);
    }

    #[test]
    fn a_pasted_image_is_named_with_digits_never_localised() {
        let at = LocalTime {
            year: 2026,
            month: 11,
            day: 21,
            hour: 14,
            minute: 5,
            second: 9,
            unix: 0,
        };
        assert_eq!(
            pasted_name(Locale::En, at),
            "Pasted image 2026-11-21 14.05.png"
        );
        assert_eq!(
            pasted_name(Locale::Fr, at),
            "Image collée 2026-11-21 14.05.png"
        );
    }

    #[test]
    fn text_wins_a_paste_then_files_then_an_image() {
        let files = vec![PathBuf::from("a.pdf")];
        assert_eq!(paste_kind(true, &files, true), Paste::Text);
        assert_eq!(paste_kind(false, &files, true), Paste::Files(files.clone()));
        assert_eq!(paste_kind(false, &[], true), Paste::Image);
        assert_eq!(paste_kind(false, &[], false), Paste::Text);
    }

    #[test]
    fn an_editor_refuses_past_the_limits_and_shows_the_notice_once() {
        let mut d = Draft {
            held: 18,
            ..Draft::default()
        };
        let g = d.start_group();
        let a = d.begin(g, "a.png".into(), 0).unwrap();
        let b = d.begin(g, "b.zip".into(), 0).unwrap();
        assert_eq!(d.begin(g, "c.txt".into(), 0), None);
        assert_eq!(d.refusals, [Refusal::TooMany]);
        assert!(d.begin(g, "d.txt".into(), 0).is_none());
        assert_eq!(d.refusals.len(), 1, "said once");
        assert!(d.sized(a, LARGE_FILE_BYTES + 1));
        assert!(d.notice);
        assert!(!d.sized(b, 48 * 1024 * 1024));
        assert_eq!(d.files.len(), 1);
        assert_eq!(
            d.refusals[1].message(Locale::En),
            "b.zip is 48 MB; the limit is 25 MB per file."
        );
        // The next add clears the refusals; the notice stays shown once.
        d.start_group();
        assert!(d.refusals.is_empty());
        assert!(d.notice);
        d.failed(
            a,
            Refusal::ReadFailed {
                name: "a.png".into(),
            },
        );
        assert!(d.files.is_empty());
        assert_eq!(
            d.refusals[0].message(Locale::En),
            "Could not read a.png. It may have moved or be in use."
        );
    }

    #[test]
    fn progress_shows_after_400_ms_until_stored() {
        let mut d = Draft::default();
        let g = d.start_group();
        let k = d.begin(g, "trace.zip".into(), 1_000).unwrap();
        d.sized(k, 1000);
        d.progress(k, 400);
        let f = d.files[0].clone();
        assert_eq!(f.adding_label(Locale::En, 1_399), None);
        assert_eq!(
            f.adding_label(Locale::En, 1_400).as_deref(),
            Some("Adding\u{2026} 40%")
        );
        assert_eq!(
            f.adding_label(Locale::Fr, 1_400).as_deref(),
            Some("Ajout\u{2026} 40\u{202f}%")
        );
        assert!(!d.settled());
    }

    fn file(id: i64, kind: FileKind) -> Attachment {
        Attachment {
            id: AttachmentId(id),
            owner: AttachOwner::Note(NoteId(1)),
            project: ProjectId(1),
            name: format!("f{id}"),
            size: 3_565_158,
            kind,
            media_type: String::new(),
            pixels: (kind == FileKind::Image).then_some((3024, 4032)),
            position: id,
            moved_from: None,
            created_by: Author::Human,
            stamp: stamp(),
        }
    }

    #[test]
    fn images_come_first_and_the_viewer_steps_through_viewable_files() {
        let files = vec![
            file(1, FileKind::Pdf),
            file(2, FileKind::Image),
            file(3, FileKind::Text),
            file(4, FileKind::Image),
            file(5, FileKind::Other),
        ];
        let order: Vec<i64> = strip_order(&files).iter().map(|f| f.id.0).collect();
        assert_eq!(order, [2, 4, 1, 3, 5]);
        let (list, at) = viewer_list(&files, &files[2]);
        assert_eq!(list.iter().map(|f| f.id.0).collect::<Vec<_>>(), [2, 4, 3]);
        assert_eq!(at, 2);
        assert_eq!(step(at, list.len(), true), 2, "stays at the end");
        assert_eq!(step(0, list.len(), false), 0);
        assert_eq!(step(0, list.len(), true), 1);
        assert_eq!(
            viewer_meta(Locale::En, &files[1]),
            "3.4 MB \u{b7} 3024 x 4032"
        );
        assert_eq!(viewer_meta(Locale::En, &files[0]), "3.4 MB");
    }

    #[test]
    fn images_fit_and_are_never_scaled_up() {
        assert_eq!(fit_scale((100., 50.), (800., 600.)), 1.);
        assert_eq!(fit_scale((3024., 4032.), (1008., 672.)), 672. / 4032.);
        assert_eq!(fit_scale((0., 0.), (10., 10.)), 1.);
    }

    #[test]
    fn the_text_view_stops_at_one_megabyte_on_a_character() {
        let (t, cut) = text_view(b"# Plan\nAlexandre", 16);
        assert_eq!(t, "# Plan\nAlexandre");
        assert!(!cut);
        let mut big = "é".repeat(TEXT_VIEW_BYTES / 2).into_bytes();
        big.insert(0, b'a');
        let (t, cut) = text_view(&big, big.len() as u64);
        assert!(cut);
        assert!(t.len() <= TEXT_VIEW_BYTES);
        assert!(t.ends_with('é'), "no broken character at the end");
    }

    #[test]
    fn settings_say_the_total_of_attached_files() {
        assert_eq!(
            totals_line(Locale::En, 312 * 1024 * 1024, 48),
            "Attached files: 312 MB in 48 files"
        );
        assert_eq!(
            totals_line(Locale::En, 812, 1),
            "Attached files: 812 B in 1 file"
        );
        assert_eq!(
            totals_line(Locale::Fr, 312 * 1024 * 1024, 48),
            "Fichiers joints\u{a0}: 312\u{202f}Mo dans 48 fichiers"
        );
    }

    #[test]
    fn panning_follows_the_pointer_and_stops_at_the_edges() {
        // Dragging the image left by 30 scrolls 30 to the right.
        assert_eq!(panned((0., 0.), (100., 50.), (30., 0.)), (-30., 0.));
        assert_eq!(panned((-90., -40.), (100., 50.), (30., 30.)), (-100., -50.));
        assert_eq!(panned((-10., 0.), (100., 50.), (-30., -5.)), (0., 0.));
        // An image that fits on an axis never moves on it.
        assert_eq!(panned((0., 0.), (0., 0.), (30., 30.)), (0., 0.));
    }
}
