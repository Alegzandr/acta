//! What Acta knows about an attached file's content (ADR 0012): its kind
//! from its first bytes, an image's pixel size and thumbnail, the image the
//! agent receives, and a PDF's text. Everything works on bytes in memory;
//! nothing here touches the disk.

use crate::model::FileKind;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits};
use std::io::Cursor;

/// How much of a file decides whether it is text-like (ADR 0012, 6).
pub const SNIFF_BYTES: usize = 8 * 1024;
/// Long side of a stored thumbnail.
pub const THUMBNAIL_PX: u32 = 256;
/// Long side of the image the agent receives by default (about 1,600
/// tokens) and with `size="full"` (about 3,300; Claude resizes above it).
pub const MODEL_PX: u32 = 1_092;
pub const MODEL_FULL_PX: u32 = 1_568;
/// The largest image Acta decodes, per side: a decompression bomb stops
/// here instead of filling memory.
const MAX_SIDE: u32 = 16_384;
/// The most memory one decode may take: when a file is added (the app,
/// off the UI thread).
const MAX_ALLOC: u64 = 512 * 1024 * 1024;
/// The most memory one decode for the agent may take (`acta mcp` stays
/// small; a larger image is described in one line instead).
pub const MODEL_MAX_ALLOC: u64 = 128 * 1024 * 1024;

pub const MEDIA_PNG: &str = "image/png";
pub const MEDIA_JPEG: &str = "image/jpeg";
pub const MEDIA_PDF: &str = "application/pdf";
pub const MEDIA_TEXT: &str = "text/plain";
pub const MEDIA_BINARY: &str = "application/octet-stream";

/// The media type of a file from its first bytes (at least
/// [`SNIFF_BYTES`] of it, or all of a shorter file): the five image
/// formats Acta shows, PDF, text-like, or binary. The name is not trusted.
pub fn sniff(head: &[u8]) -> &'static str {
    match image::guess_format(head) {
        Ok(ImageFormat::Png) => return MEDIA_PNG,
        Ok(ImageFormat::Jpeg) => return MEDIA_JPEG,
        Ok(ImageFormat::Gif) => return "image/gif",
        Ok(ImageFormat::WebP) => return "image/webp",
        Ok(ImageFormat::Bmp) => return "image/bmp",
        _ => {}
    }
    if head.starts_with(b"%PDF-") {
        return MEDIA_PDF;
    }
    if is_text_like(head) {
        MEDIA_TEXT
    } else {
        MEDIA_BINARY
    }
}

/// Valid UTF-8 with no NUL byte in the first [`SNIFF_BYTES`]; a character
/// cut by the end of the sample counts as valid.
pub fn is_text_like(head: &[u8]) -> bool {
    let head = &head[..head.len().min(SNIFF_BYTES)];
    if head.contains(&0) {
        return false;
    }
    match std::str::from_utf8(head) {
        Ok(_) => true,
        // `error_len() == None`: the sample ends inside a character.
        Err(e) => e.error_len().is_none() && head.len() - e.valid_up_to() < 4,
    }
}

pub fn kind_of(media_type: &str) -> FileKind {
    match media_type {
        "image/png" | "image/jpeg" | "image/gif" | "image/webp" | "image/bmp" => FileKind::Image,
        MEDIA_PDF => FileKind::Pdf,
        t if t.starts_with("text/") => FileKind::Text,
        _ => FileKind::Other,
    }
}

/// Decodes an image with Acta's limits and turns it the way its EXIF
/// orientation says, so pixel sizes and thumbnails match what people see.
pub fn decode(bytes: &[u8]) -> Option<DynamicImage> {
    decode_within(bytes, MAX_ALLOC)
}

/// [`decode`] with at most `max_alloc` bytes for the pixels.
pub fn decode_within(bytes: &[u8], max_alloc: u64) -> Option<DynamicImage> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(max_alloc);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().ok()?;
    // The pixels must fit the allowance (the decoders do not all check it).
    if decoder.total_bytes() > max_alloc {
        return None;
    }
    let orientation = decoder.orientation().ok();
    let mut img = DynamicImage::from_decoder(decoder).ok()?;
    if let Some(o) = orientation {
        img.apply_orientation(o);
    }
    Some(img)
}

/// What adding an image stores: its pixel size and a PNG thumbnail of at
/// most [`THUMBNAIL_PX`] on the long side (never enlarged).
pub struct ImageInfo {
    pub width: u32,
    pub height: u32,
    pub thumbnail: Vec<u8>,
}

/// `None` when the bytes do not decode: the file is then kept as binary.
pub fn image_info(bytes: &[u8]) -> Option<ImageInfo> {
    let img = decode(bytes)?;
    let (width, height) = (img.width(), img.height());
    let small = fit(&img, THUMBNAIL_PX);
    let mut thumbnail = Vec::new();
    small
        .write_to(&mut Cursor::new(&mut thumbnail), ImageFormat::Png)
        .ok()?;
    Some(ImageInfo {
        width,
        height,
        thumbnail,
    })
}

/// The image scaled down to `max` on its long side; unchanged when it
/// already fits (never enlarged).
fn fit(img: &DynamicImage, max: u32) -> DynamicImage {
    let (w, h) = (img.width(), img.height());
    if w.max(h) <= max {
        return img.clone();
    }
    let (nw, nh) = scaled(w, h, max);
    img.resize_exact(nw, nh, image::imageops::FilterType::Triangle)
}

/// `(w, h)` scaled so the long side is `max`, aspect kept, each side at
/// least 1.
pub fn scaled(w: u32, h: u32, max: u32) -> (u32, u32) {
    if w.max(h) <= max {
        return (w, h);
    }
    let r = max as f64 / w.max(h) as f64;
    let nw = ((w as f64 * r).round() as u32).clamp(1, max);
    let nh = ((h as f64 * r).round() as u32).clamp(1, max);
    (nw, nh)
}

/// The image the agent receives (ADR 0012, 10): at most `max` pixels on
/// the long side, JPEG for photos (a JPEG or WebP source) and PNG for
/// screenshots and diagrams, re-encoded in memory.
pub struct ModelImage {
    pub bytes: Vec<u8>,
    pub media_type: &'static str,
    pub width: u32,
    pub height: u32,
}

pub fn for_model(bytes: &[u8], source_type: &str, max: u32) -> Option<ModelImage> {
    let img = fit(&decode_within(bytes, MODEL_MAX_ALLOC)?, max);
    let photo = matches!(source_type, "image/jpeg" | "image/webp");
    let mut out = Vec::new();
    let media_type = if photo || !img.color().has_alpha() && png_too_big(&img) {
        let rgb = DynamicImage::ImageRgb8(img.to_rgb8());
        let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 85);
        rgb.write_with_encoder(enc).ok()?;
        MEDIA_JPEG
    } else {
        img.write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
            .ok()?;
        MEDIA_PNG
    };
    Some(ModelImage {
        bytes: out,
        media_type,
        width: img.width(),
        height: img.height(),
    })
}

/// A PNG of an opaque image over 1.5 MB is a photo saved as PNG: JPEG then.
fn png_too_big(img: &DynamicImage) -> bool {
    let mut probe = Vec::new();
    img.write_to(&mut Cursor::new(&mut probe), ImageFormat::Png)
        .map_or(true, |_| probe.len() > 1_500_000)
}

/// Text of some pages of a PDF.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PdfPages {
    /// Pages in the document.
    pub count: u32,
    /// `(page number, text)`, in order, from the first page asked.
    pub pages: Vec<(u32, String)>,
}

/// What a PDF holds as text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PdfText {
    Pages(PdfPages),
    /// The file did not parse (damaged, encrypted, or not really a PDF),
    /// or its parser ran out of time or memory.
    Unreadable,
}

/// The text of pages `from..=to` (1-based), stopping once `max_chars`
/// characters are collected; a page is cut at `max_chars`. Pure Rust
/// (pdf-extract, MIT). Runs in the `acta pdf-text` child process, so a
/// malformed file can only cost that process ([`crate::agent::attach`]
/// gives it a time, memory and output limit). Panics are caught.
pub fn pdf_pages(bytes: &[u8], from: u32, to: u32, max_chars: usize) -> PdfText {
    let run = std::panic::catch_unwind(|| -> Option<PdfPages> {
        let doc = pdf_extract::Document::load_mem(bytes).ok()?;
        if doc.is_encrypted() {
            return None;
        }
        let count = doc.get_pages().len() as u32;
        let mut pages = Vec::new();
        let mut used = 0usize;
        for p in from.max(1)..=to.min(count) {
            let mut text = String::new();
            {
                let mut out = pdf_extract::PlainTextOutput::new(&mut text);
                // A page that fails reads as empty; the others still count.
                if pdf_extract::output_doc_page(&doc, &mut out, p).is_err() {
                    text.clear();
                }
            }
            let text: String = text.chars().take(max_chars).collect();
            used += text.chars().count();
            pages.push((p, text));
            if used >= max_chars {
                break;
            }
        }
        Some(PdfPages { count, pages })
    });
    match run {
        Ok(Some(p)) => PdfText::Pages(p),
        _ => PdfText::Unreadable,
    }
}

/// A size as the UI and the CLI show it (components-attachments.md 5):
/// bytes under 1 KB, then KB and MB with one decimal under 10 and none
/// above, 1 KB = 1,024 bytes. English separators.
pub fn size_label(bytes: i64) -> String {
    let b = bytes.max(0) as f64;
    let (v, unit) = if bytes < 1024 {
        return format!("{bytes} B");
    } else if b < 1024.0 * 1024.0 {
        (b / 1024.0, "KB")
    } else {
        (b / (1024.0 * 1024.0), "MB")
    };
    if v < 10.0 {
        let t = format!("{v:.1}");
        // 9.96 KB would read "10.0 KB": no decimal from 10 on.
        if t.starts_with("10") {
            format!("10 {unit}")
        } else {
            format!("{t} {unit}")
        }
    } else {
        format!("{} {unit}", v.round() as i64)
    }
}

/// The agent's short form: `812B`, `340K`, `2.1M`, `12M`.
pub fn short_size(bytes: i64) -> String {
    let l = size_label(bytes);
    let (n, unit) = l.split_once(' ').unwrap_or((&l, ""));
    format!("{n}{}", &unit[..1])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbaImage::from_fn(w, h, |x, y| {
            image::Rgba([(x % 256) as u8, (y % 256) as u8, 90, 255])
        });
        let mut out = Vec::new();
        DynamicImage::ImageRgba8(img)
            .write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
            .unwrap();
        out
    }

    fn jpeg(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbImage::from_fn(w, h, |x, y| {
            image::Rgb([
                (x * 7 % 256) as u8,
                (y * 3 % 256) as u8,
                ((x + y) % 256) as u8,
            ])
        });
        let mut out = Vec::new();
        DynamicImage::ImageRgb8(img)
            .write_to(&mut Cursor::new(&mut out), ImageFormat::Jpeg)
            .unwrap();
        out
    }

    #[test]
    fn sniffs_kinds_from_bytes_not_names() {
        assert_eq!(sniff(&png(4, 4)), MEDIA_PNG);
        assert_eq!(sniff(&jpeg(4, 4)), MEDIA_JPEG);
        assert_eq!(sniff(b"%PDF-1.7\n..."), MEDIA_PDF);
        assert_eq!(sniff("Notes de Alexandre, café".as_bytes()), MEDIA_TEXT);
        assert_eq!(sniff(b""), MEDIA_TEXT);
        assert_eq!(sniff(b"PK\x03\x04\x00\x00binary"), MEDIA_BINARY);
        assert_eq!(sniff(&[0xff, 0xfe, 0x41, 0x00]), MEDIA_BINARY);
        // A character cut by the end of the sample is still text.
        let mut t = vec![b'a'; SNIFF_BYTES - 1];
        t.extend_from_slice("é".as_bytes());
        assert!(is_text_like(&t[..SNIFF_BYTES]));
        assert_eq!(kind_of(MEDIA_TEXT), FileKind::Text);
        assert_eq!(kind_of("image/gif"), FileKind::Image);
        assert_eq!(kind_of(MEDIA_BINARY), FileKind::Other);
    }

    #[test]
    fn thumbnails_fit_256_and_never_enlarge() {
        let info = image_info(&png(1000, 400)).unwrap();
        assert_eq!((info.width, info.height), (1000, 400));
        let t = decode(&info.thumbnail).unwrap();
        assert_eq!((t.width(), t.height()), (256, 102));
        let small = image_info(&png(40, 30)).unwrap();
        let t = decode(&small.thumbnail).unwrap();
        assert_eq!((t.width(), t.height()), (40, 30));
        assert!(image_info(b"\x89PNG\r\n\x1a\nbroken").is_none());
    }

    #[test]
    fn model_images_respect_the_pixel_caps() {
        // A tall photo: JPEG, long side 1,092 by default, 1,568 in full.
        let photo = jpeg(1200, 1800);
        let m = for_model(&photo, MEDIA_JPEG, MODEL_PX).unwrap();
        assert_eq!(m.media_type, MEDIA_JPEG);
        assert_eq!((m.width, m.height), (728, 1092));
        let decoded = decode(&m.bytes).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (728, 1092));
        let full = for_model(&photo, MEDIA_JPEG, MODEL_FULL_PX).unwrap();
        assert_eq!((full.width, full.height), (1045, 1568));
        // A screenshot: PNG, wide.
        let shot = for_model(&png(3000, 1000), MEDIA_PNG, MODEL_PX).unwrap();
        assert_eq!(shot.media_type, MEDIA_PNG);
        assert_eq!((shot.width, shot.height), (1092, 364));
        // Never enlarged.
        let tiny = for_model(&png(300, 200), MEDIA_PNG, MODEL_FULL_PX).unwrap();
        assert_eq!((tiny.width, tiny.height), (300, 200));
        assert_eq!(scaled(5000, 3, 1092), (1092, 1));
    }

    #[test]
    fn sizes_read_as_the_file_managers_show_them() {
        assert_eq!(size_label(812), "812 B");
        assert_eq!(size_label(340 * 1024), "340 KB");
        assert_eq!(size_label(2_202_009), "2.1 MB");
        assert_eq!(size_label(12 * 1024 * 1024), "12 MB");
        assert_eq!(size_label(10_199), "10 KB");
        assert_eq!(short_size(2_202_009), "2.1M");
        assert_eq!(short_size(812), "812B");
        assert_eq!(short_size(340 * 1024), "340K");
    }

    #[test]
    fn garbage_pdfs_are_unreadable_not_a_panic() {
        assert_eq!(
            pdf_pages(b"%PDF-1.4 garbage", 1, 5, 100),
            PdfText::Unreadable
        );
    }

    #[test]
    fn agent_decodes_stop_at_their_memory_allowance() {
        // The allowance is checked before the pixels are allocated.
        let img = png(1000, 1000);
        assert!(decode_within(&img, MODEL_MAX_ALLOC).is_some());
        assert!(decode_within(&img, 1024 * 1024).is_none());
    }
}
