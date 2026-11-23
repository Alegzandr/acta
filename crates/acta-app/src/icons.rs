//! The icons the design docs list (Lucide, ISC), embedded as SVG with
//! `stroke-width="1.5"` so strokes land at 1px at 16 (docs/design/layout.md 4.1), and
//! the app mark. The SVGs in `assets/icons/lucide` are copies of the Lucide
//! files bundled in `gpui-kit-assets` 0.10.0 with only the stroke width
//! changed; their licence ships from `scripts/licenses/third-party`.
//! The design docs' `trash-2` is Lucide's `trash` today (Lucide renamed it); the
//! file keeps the design docs' name.

use std::borrow::Cow;

use crate::zoom::px;
use gpui_kit::{AssetSource, Hsla, Result, SharedString, Styled, Svg, svg};

macro_rules! icons {
    ($( $variant:ident => $file:literal, )*) => {
        /// An icon of docs/design/layout.md 4.1. Only the ones a screen uses are bundled.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Icon {
            $( $variant, )*
        }

        impl Icon {
            pub const ALL: &'static [Icon] = &[ $( Icon::$variant, )* ];

            /// The asset path GPUI loads it from.
            pub fn path(self) -> &'static str {
                match self {
                    $( Icon::$variant => concat!("icons/", $file, ".svg"), )*
                }
            }

            fn bytes(self) -> &'static [u8] {
                match self {
                    $( Icon::$variant => include_bytes!(concat!(
                        "../../../assets/icons/lucide/", $file, ".svg"
                    )), )*
                }
            }
        }
    };
}

icons! {
    ArrowLeft => "arrow-left",
    ChevronLeft => "chevron-left",
    ChevronRight => "chevron-right",
    ChevronUp => "chevron-up",
    CircleDot => "circle-dot",
    CirclePause => "circle-pause",
    CornerDownRight => "corner-down-right",
    Ellipsis => "ellipsis",
    FileText => "file-text",
    FolderOpen => "folder-open",
    FolderX => "folder-x",
    GitCommitHorizontal => "git-commit-horizontal",
    Inbox => "inbox",
    Info => "info",
    Layers => "layers",
    Library => "library",
    MessageSquare => "message-square",
    Minus => "minus",
    Pencil => "pencil",
    Pin => "pin",
    Plus => "plus",
    RotateCw => "rotate-cw",
    Signpost => "signpost",
    StickyNote => "sticky-note",
    TriangleAlert => "triangle-alert",
    X => "x",
    ArrowRight => "arrow-right",
    Check => "check",
    ChevronDown => "chevron-down",
    Circle => "circle",
    CircleCheck => "circle-check",
    CircleDashed => "circle-dashed",
    CircleSlash => "circle-slash",
    Folder => "folder",
    MessageCircle => "message-circle",
    PanelLeft => "panel-left",
    Search => "search",
    Settings => "settings",
    SquareTerminal => "square-terminal",
    Trash2 => "trash-2",
    ExternalLink => "external-link",
    ListTree => "list-tree",
    PanelBottom => "panel-bottom",
    PanelRight => "panel-right",
    PanelsTopLeft => "panels-top-left",
    Globe => "globe",
    Square => "square",
    Mic => "mic",
    Play => "play",
    Bot => "bot",
    Clock => "clock",
    ClockAlert => "clock-alert",
    FolderGit2 => "folder-git-2",
    GitBranch => "git-branch",
    Code => "code",
    BookOpen => "book-open",
    Package => "package",
    Server => "server",
    Smartphone => "smartphone",
    Database => "database",
    FlaskConical => "flask-conical",
    Palette => "palette",
    CircleQuestionMark => "circle-question-mark",
    Users => "users",
    User => "user",
    Paperclip => "paperclip",
    File => "file",
}

/// The app mark, shown in the updater window only (docs/design/components-app.md 5).
pub const APP_MARK: &str = "acta.png";
const APP_MARK_BYTES: &[u8] = include_bytes!("../../../assets/icons/acta.png");

/// Acta's assets. Anything else is asked of gpui-kit's bundle, which its
/// components expect.
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path == APP_MARK {
            return Ok(Some(Cow::Borrowed(APP_MARK_BYTES)));
        }
        if let Some(icon) = Icon::ALL.iter().find(|icon| icon.path() == path) {
            return Ok(Some(Cow::Borrowed(icon.bytes())));
        }
        gpui_kit::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        gpui_kit::assets::Assets.list(path)
    }
}

/// `icon` at `size` (16 by default, 14 for status glyphs), never filled.
pub fn icon(icon: Icon, size: f32, color: Hsla) -> Svg {
    svg()
        .path(icon.path())
        .size(px(size))
        .flex_shrink_0()
        .text_color(color)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_is_drawn_at_a_1_5_stroke() {
        for icon in Icon::ALL {
            let text = std::str::from_utf8(icon.bytes()).unwrap();
            assert!(text.contains("stroke-width=\"1.5\""), "{icon:?}");
            assert!(text.contains("fill=\"none\""), "{icon:?}");
        }
    }

    #[test]
    fn our_paths_are_served() {
        for icon in Icon::ALL {
            assert!(Assets.load(icon.path()).unwrap().is_some(), "{icon:?}");
        }
        assert!(Assets.load(APP_MARK).unwrap().is_some());
    }
}
