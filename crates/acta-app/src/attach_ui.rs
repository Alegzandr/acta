//! Files in the window (docs/design/components-attachments.md 5; ADR
//! 0012): the strip under a note, an entry or a description; the files an
//! editor is adding ([`Tray`]); the file menu; the viewer; Save as. File
//! content is decrypted in memory only: thumbnails and the viewer decode
//! from bytes read off the store, and nothing is written to disk except
//! where the human chooses with Save as.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use acta_core::Store;
use acta_core::model::{
    AttachOwner, Attachment, AttachmentId, Author, FileKind, ProjectId, Record,
};
use acta_core::store::{AttachSource, MAX_FILE_BYTES, Staged};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, AppContext, Asset, Bounds, ClipboardEntry, ClipboardItem, Context, Entity,
    EventEmitter, ExternalPaths, FocusHandle, Focusable, Global, ImageCacheError, ImageSource,
    InteractiveElement, IntoElement, KeyDownEvent, ObjectFit, ParentElement, PathPromptOptions,
    Pixels, Point, Render, RenderImage, ScrollHandle, SharedString, StatefulInteractiveElement,
    Styled, StyledImage, Task, WeakEntity, Window, div, img, px,
};

use crate::attach::{self, Draft, DraftFile, Paste, Refusal};
use crate::i18n::{Arg, Key, KeyName, Locale, text, tr};
use crate::icons::{Icon, icon};
use crate::settings;
use crate::theme::{RADIUS_SM, RADIUS_XL, Tokens};
use crate::type_scale;
use crate::ui::{self, Variant};
use crate::workspace::{Toast, ToastAction, ToastKind, Undo, Workspace, WorkspaceEvent};

/// How often an editor's files are looked at while they are being stored.
const TICK: Duration = Duration::from_millis(100);
/// How often an open editor restarts the age of its pending files (the
/// purge leaves pending blobs alone for an hour).
const KEEP_ALIVE: Duration = Duration::from_secs(30 * 60);

/// `Mod` on this platform: Cmd on macOS, Ctrl elsewhere.
fn mod_key(m: &gpui_kit::Modifiers) -> bool {
    if cfg!(target_os = "macos") {
        m.platform
    } else {
        m.control
    }
}

/// An image decoded in memory, as GPUI draws it (BGRA).
fn render_image(image: image::DynamicImage) -> Arc<RenderImage> {
    let mut rgba = image.into_rgba8();
    for px in rgba.pixels_mut() {
        px.0.swap(0, 2);
    }
    Arc::new(RenderImage::new(vec![image::Frame::new(rgba)]))
}

/// Which thumbnail a tile shows: a stored file's, or the one stored with
/// a file an editor is still adding.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ThumbKey {
    Stored(AttachmentId),
    Staged(Staged),
}

/// Thumbnails, read from the store and decoded off the UI thread, kept by
/// GPUI's asset cache (each a few tens of KB).
enum Thumb {}

impl Asset for Thumb {
    type Source = ThumbKey;
    type Output = Option<Arc<RenderImage>>;

    fn load(
        source: Self::Source,
        cx: &mut App,
    ) -> impl std::future::Future<Output = Self::Output> + Send + 'static {
        let db = cx
            .try_global::<FilesHub>()
            .map(|hub| hub.0.read(cx).ws.read(cx).db.clone());
        async move {
            let db = db?;
            let bytes = {
                let store = db.lock();
                match &source {
                    ThumbKey::Stored(id) => store.attachment_thumbnail(*id).ok().flatten(),
                    ThumbKey::Staged(s) => store.staged_thumbnail(s).ok().flatten(),
                }
            }?;
            acta_core::files::decode(&bytes).map(render_image)
        }
    }
}

fn thumb_source(key: ThumbKey) -> ImageSource {
    ImageSource::Custom(Arc::new(move |window, cx| {
        match window.use_asset::<Thumb>(&key, cx)? {
            Some(image) => Some(Ok(image)),
            None => Some(Err(ImageCacheError::Asset("no thumbnail".into()))),
        }
    }))
}

// ---- the hub: menus, the viewer, Save as ----

type BoundsMap = std::rc::Rc<std::cell::RefCell<HashMap<AttachmentId, Bounds<Pixels>>>>;

/// The one [`Files`] of the window, for strips drawn by any page.
pub struct FilesHub(pub Entity<Files>);

impl Global for FilesHub {}

/// What a file menu is about.
#[derive(Clone)]
pub enum MenuTarget {
    Stored {
        file: Attachment,
        siblings: Vec<Attachment>,
    },
    /// A file an editor has not saved yet: Remove only.
    Draft { tray: WeakEntity<Tray>, key: u64 },
}

pub enum FilesEvent {
    /// Open the viewer on `list[at]`.
    View(Vec<Attachment>, usize),
    /// Open the file menu at a window position.
    Menu(MenuTarget, Point<Pixels>),
}

/// The window's file actions: the viewer and the menu (opened by the
/// shell as overlays), Save as, Remove, and each file's focus handle.
pub struct Files {
    pub ws: Entity<Workspace>,
    /// The last folder Save as wrote to in this session.
    save_dir: Option<PathBuf>,
    focus: HashMap<AttachmentId, FocusHandle>,
    /// Each file as last drawn: the palette's "Save file as..." acts on
    /// the one with the keyboard.
    drawn: HashMap<AttachmentId, Attachment>,
    /// Where each file was last drawn (the menu opened from the keyboard).
    bounds: BoundsMap,
}

impl EventEmitter<FilesEvent> for Files {}

impl Files {
    pub fn new(ws: Entity<Workspace>) -> Self {
        Self {
            ws,
            save_dir: None,
            focus: HashMap::new(),
            drawn: HashMap::new(),
            bounds: Default::default(),
        }
    }

    /// The file of a strip that has the keyboard.
    pub fn focused(&self, window: &Window) -> Option<Attachment> {
        let (id, _) = self.focus.iter().find(|(_, h)| h.is_focused(window))?;
        self.drawn.get(id).cloned()
    }

    fn focus_for(&mut self, id: AttachmentId, cx: &mut App) -> FocusHandle {
        self.focus
            .entry(id)
            .or_insert_with(|| cx.focus_handle())
            .clone()
    }

    /// `Enter`, `Space` or a click: the viewer for images and text-like
    /// files, the menu for the others.
    pub fn activate(
        &mut self,
        file: &Attachment,
        siblings: &[Attachment],
        at: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if attach::viewable(file) {
            let (list, i) = attach::viewer_list(siblings, file);
            cx.emit(FilesEvent::View(list, i));
        } else {
            self.open_menu(file, siblings, at, cx);
        }
    }

    pub fn open_menu(
        &mut self,
        file: &Attachment,
        siblings: &[Attachment],
        at: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        cx.emit(FilesEvent::Menu(
            MenuTarget::Stored {
                file: file.clone(),
                siblings: siblings.to_vec(),
            },
            at,
        ));
    }

    /// The menu from the keyboard, under the file.
    fn open_menu_here(
        &mut self,
        file: &Attachment,
        siblings: &[Attachment],
        cx: &mut Context<Self>,
    ) {
        let at = self
            .bounds
            .borrow()
            .get(&file.id)
            .map(|b| b.bottom_left())
            .unwrap_or_default();
        self.open_menu(file, siblings, at, cx);
    }

    /// Debug builds: the viewer on the first (`attach-viewer`) or second
    /// (`-2`) image of the first note with files, or on its text file
    /// (`-text`, a file over 1 MB first); the menu of its PDF
    /// (`attach-menu`).
    #[cfg(debug_assertions)]
    pub fn dev_open(&mut self, which: &str, cx: &mut Context<Self>) {
        let which = which.to_owned();
        let task = self.ws.read(cx).db.run(cx, |store| {
            let mut all = Vec::new();
            for p in store.projects()? {
                let notes = store.inbox(p.id, acta_core::model::PageReq::first(50))?;
                for n in notes.items {
                    all.push(store.attachments(AttachOwner::Note(n.id))?);
                }
                for t in store.tree(p.id)?.loose_topics {
                    all.push(store.attachments(AttachOwner::Topic(t.id))?);
                }
            }
            Ok(all)
        });
        cx.spawn(async move |this, cx| {
            let Ok(lists) = task.await else { return };
            let _ = this.update(cx, |files, cx| {
                let pick = |kind: FileKind, big: bool| {
                    lists.iter().find_map(|list| {
                        list.iter()
                            .find(|f| f.kind == kind && (!big || f.size > 1024 * 1024))
                            .map(|f| (list.clone(), f.clone()))
                    })
                };
                match which.as_str() {
                    "attach-viewer" | "attach-viewer-2" => {
                        if let Some((list, f)) = pick(FileKind::Image, false) {
                            let (list, mut at) = attach::viewer_list(&list, &f);
                            if which.ends_with("-2") {
                                at = attach::step(at, list.len(), true);
                            }
                            cx.emit(FilesEvent::View(list, at));
                        }
                    }
                    "attach-viewer-text" => {
                        if let Some((list, f)) =
                            pick(FileKind::Text, true).or_else(|| pick(FileKind::Text, false))
                        {
                            let (list, at) = attach::viewer_list(&list, &f);
                            cx.emit(FilesEvent::View(list, at));
                        }
                    }
                    "attach-menu" => {
                        if let Some((list, f)) = pick(FileKind::Pdf, false) {
                            files.open_menu_here(&f, &list, cx);
                        }
                    }
                    // Save as without the dialog: to `ACTA_DEV_SAVE_TO`.
                    "attach-save" => {
                        if let (Some((_, f)), Ok(to)) = (
                            pick(FileKind::Pdf, false),
                            std::env::var("ACTA_DEV_SAVE_TO"),
                        ) {
                            files.save_to(f, PathBuf::from(to), cx);
                        }
                    }
                    _ => {}
                }
            });
        })
        .detach();
    }

    /// Remove: to the trash, "File removed." with Undo.
    pub fn remove(&mut self, file: &Attachment, cx: &mut Context<Self>) {
        let record = Record::Attachment(file.id);
        let project = Some(file.project);
        self.ws.update(cx, |ws, cx| ws.delete(record, project, cx));
    }

    /// Save as...: the system's save dialog with the file's name, in the
    /// last folder used this session.
    pub fn save_as(&mut self, file: &Attachment, cx: &mut Context<Self>) {
        let dir = self.save_dir.clone().unwrap_or_default();
        let chosen = cx.prompt_for_new_path(&dir, Some(&file.name));
        let file = file.clone();
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(path))) = chosen.await else {
                return;
            };
            let _ = this.update(cx, |files, cx| files.save_to(file, path, cx));
        })
        .detach();
    }

    /// Writes the decrypted file to `path` (chosen by the human), off the
    /// UI thread; a toast says where, with "Show in folder".
    pub fn save_to(&mut self, file: Attachment, path: PathBuf, cx: &mut Context<Self>) {
        self.save_dir = path.parent().map(Path::to_path_buf);
        let db = self.ws.read(cx).db.clone();
        let (id, target) = (file.id, path.clone());
        let task = cx
            .background_executor()
            .spawn(async move { write_file(&db.lock(), id, &target) });
        cx.spawn(async move |this, cx| {
            let written = task.await;
            let _ = this.update(cx, |files, cx| {
                let locale = settings::locale(cx);
                let toast = match written {
                    Ok(()) => {
                        // Cut in the middle by folder to fit the toast.
                        let folder = path
                            .parent()
                            .map(|p| {
                                crate::paths_view::fit(
                                    &p.display().to_string(),
                                    crate::paths_view::budget(220.),
                                )
                            })
                            .unwrap_or_default();
                        Toast {
                            message: tr(
                                locale,
                                Key::AttachSaved,
                                &[("folder", Arg::Text(&folder))],
                            ),
                            kind: ToastKind::Done,
                            action: Some(ToastAction::Reveal(path)),
                        }
                    }
                    Err(err) => {
                        // The log says what kind of failure, never the path.
                        crate::log::line(&format!("files: save as failed: {:?}", err.kind()));
                        Toast {
                            message: tr(
                                locale,
                                Key::AttachSaveFailed,
                                &[
                                    ("name", Arg::Text(&file.name)),
                                    ("reason", Arg::Text(&err.to_string())),
                                ],
                            ),
                            kind: ToastKind::Error,
                            action: Some(ToastAction::RetrySave(Box::new(file), path)),
                        }
                    }
                };
                files
                    .ws
                    .update(cx, |_, cx| cx.emit(WorkspaceEvent::Toast(toast)));
            });
        })
        .detach();
    }
}

/// Streams a stored file's content into a new file at `path` (Save as).
/// A failure leaves no partial file behind.
fn write_file(store: &Store, id: AttachmentId, path: &Path) -> std::io::Result<()> {
    let mut reader = store
        .read_attachment(id)
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    write_replacing(&mut reader, path)
}

/// Writes `reader` to a temporary file beside `path`, flushes it to disk,
/// then renames it over `path`: a failure leaves the file that was there
/// untouched and removes only the temporary one.
fn write_replacing(reader: &mut dyn Read, path: &Path) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temp = dir.join(format!(
        ".{name}.{}-{}.part",
        std::process::id(),
        acta_core::time::now_ms()
    ));
    let result = (|| {
        let mut out = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        std::io::copy(reader, &mut out)?;
        out.flush()?;
        out.sync_all()?;
        drop(out);
        std::fs::rename(&temp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}

fn hub(cx: &App) -> Option<Entity<Files>> {
    cx.try_global::<FilesHub>().map(|h| h.0.clone())
}

/// "Save file as..." from the palette: the file of a strip that has the
/// keyboard, if any.
pub fn focused_file(window: &Window, cx: &App) -> Option<Attachment> {
    hub(cx)?.read(cx).focused(window)
}

/// The palette's "Save file as..." on the focused file.
pub fn save_focused(window: &Window, cx: &mut App) {
    let Some(hub) = hub(cx) else { return };
    let Some(file) = hub.read(cx).focused(window) else {
        return;
    };
    hub.update(cx, |h, cx| h.save_as(&file, cx));
}

/// The editors that take files, each with its tray, so the palette's
/// "Attach files..." finds the tray of the editor that has (or last had)
/// the focus.
#[derive(Default)]
struct Trays(HashMap<gpui_kit::EntityId, WeakEntity<Tray>>);

impl Global for Trays {}

/// Pair an editor with its tray (each frame it is drawn).
pub fn pair(
    editor: &Entity<gpui_kit::component::input::TextareaState>,
    tray: &Entity<Tray>,
    cx: &mut App,
) {
    let trays = cx.default_global::<Trays>();
    trays.0.retain(|_, t| t.upgrade().is_some());
    trays.0.insert(editor.entity_id(), tray.downgrade());
}

/// The editor that has, or last had, the focus (the one dictation acts
/// on), when it takes files, with its tray.
pub fn target_tray(
    cx: &App,
) -> Option<(
    Entity<gpui_kit::component::input::TextareaState>,
    Entity<Tray>,
)> {
    let editor = crate::voice::get(cx)?.read(cx).target_editor()?;
    let tray = cx
        .try_global::<Trays>()?
        .0
        .get(&editor.entity_id())?
        .upgrade()?;
    Some((editor, tray))
}

/// The palette's "Attach files...": the open dialog of that editor's
/// tray, the editor focused again.
pub fn attach_to_target(window: &mut Window, cx: &mut App) {
    let Some((editor, tray)) = target_tray(cx) else {
        return;
    };
    editor.update(cx, |s, cx| s.focus(window, cx));
    tray.update(cx, |tray, cx| tray.pick(cx));
}

// ---- the strip ----

/// How a strip is drawn.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StripStyle {
    /// Thumbnails and chips with their actions.
    Full,
    /// A filed note: names and sizes in `text.secondary`, no thumbnails.
    Filed,
}

/// The strip of a note, an entry or a description: images first as
/// 64 x 64 tiles, then the other files as chips. Absent without files.
pub fn strip(
    prefix: &str,
    files: &[Attachment],
    style: StripStyle,
    t: &Tokens,
    cx: &mut App,
) -> Option<AnyElement> {
    if files.is_empty() {
        return None;
    }
    let hub = hub(cx)?;
    let locale = settings::locale(cx);
    let ordered = attach::strip_order(files);
    let siblings: Arc<Vec<Attachment>> = Arc::new(files.to_vec());
    let ids: Arc<Vec<AttachmentId>> = Arc::new(ordered.iter().map(|f| f.id).collect());
    let bounds = std::rc::Rc::clone(&hub.read(cx).bounds);
    let mut tiles = Vec::new();
    let mut chips = Vec::new();
    for (i, file) in ordered.iter().enumerate() {
        let handle = hub.update(cx, |h, cx| {
            h.drawn.insert(file.id, (*file).clone());
            h.focus_for(file.id, cx)
        });
        // One stop: the first file; arrows move inside the strip.
        let handle = handle.tab_stop(i == 0);
        let el = file_element(
            prefix, file, &siblings, &ids, &handle, style, &hub, &bounds, locale, t,
        );
        if style == StripStyle::Full && file.kind == FileKind::Image {
            tiles.push(el);
        } else {
            chips.push(el);
        }
    }
    Some(
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .when(!tiles.is_empty(), |this| {
                this.child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .gap(px(8.))
                        .children(tiles),
                )
            })
            .when(!chips.is_empty(), |this| {
                this.child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .gap(px(8.))
                        .children(chips),
                )
            })
            .into_any_element(),
    )
}

#[allow(clippy::too_many_arguments)]
fn file_element(
    prefix: &str,
    file: &Attachment,
    siblings: &Arc<Vec<Attachment>>,
    ids: &Arc<Vec<AttachmentId>>,
    handle: &FocusHandle,
    style: StripStyle,
    hub: &Entity<Files>,
    bounds: &BoundsMap,
    locale: Locale,
    t: &Tokens,
) -> AnyElement {
    let size = attach::size_label(locale, file.size.max(0) as u64);
    let tooltip = format!("{} \u{b7} {size}", file.name);
    let id = SharedString::from(format!("{prefix}-file-{}", file.id.0));
    let base = div()
        .id(id.clone())
        .track_focus(handle)
        .focus_visible({
            let ring = ui::ring_shadow(t, false);
            move |s| s.shadow(ring)
        })
        .cursor_pointer()
        .tooltip(ui::tip(tooltip))
        .child({
            let bounds = std::rc::Rc::clone(bounds);
            let fid = file.id;
            ui::measure(move |b, _| {
                bounds.borrow_mut().insert(fid, b);
            })
        });
    let base = {
        let (hub, file, siblings) = (hub.clone(), file.clone(), Arc::clone(siblings));
        base.on_click(move |event, _, cx| {
            let at = event.position();
            hub.update(cx, |h, cx| h.activate(&file, &siblings, at, cx));
        })
    };
    let base = {
        let (hub, file, siblings) = (hub.clone(), file.clone(), Arc::clone(siblings));
        base.on_mouse_down(gpui_kit::MouseButton::Right, move |event, _, cx| {
            let at = event.position;
            hub.update(cx, |h, cx| h.open_menu(&file, &siblings, at, cx));
            cx.stop_propagation();
        })
    };
    let base = {
        let (hub, file, siblings, ids) = (
            hub.clone(),
            file.clone(),
            Arc::clone(siblings),
            Arc::clone(ids),
        );
        base.on_key_down(move |event: &KeyDownEvent, window, cx| {
            let k = &event.keystroke;
            let here = ids.iter().position(|i| *i == file.id).unwrap_or(0);
            let go = |to: usize, window: &mut Window, cx: &mut App| {
                if let Some(target) = ids.get(to) {
                    let handle = hub.update(cx, |h, cx| h.focus_for(*target, cx));
                    window.focus(&handle, cx);
                }
            };
            match k.key.as_str() {
                "left" if !k.modifiers.modified() => go(here.saturating_sub(1), window, cx),
                "right" if !k.modifiers.modified() => go(here + 1, window, cx),
                "home" => go(0, window, cx),
                "end" => go(ids.len().saturating_sub(1), window, cx),
                "s" if mod_key(&k.modifiers) => hub.update(cx, |h, cx| h.save_as(&file, cx)),
                "delete" => hub.update(cx, |h, cx| h.remove(&file, cx)),
                "backspace" if k.modifiers.platform => hub.update(cx, |h, cx| h.remove(&file, cx)),
                "f10" if k.modifiers.shift => {
                    hub.update(cx, |h, cx| h.open_menu_here(&file, &siblings, cx))
                }
                "menu" => hub.update(cx, |h, cx| h.open_menu_here(&file, &siblings, cx)),
                "escape" => window.focus_prev(cx),
                _ => return,
            }
            cx.stop_propagation();
        })
    };
    if style == StripStyle::Full && file.kind == FileKind::Image {
        let strong = t.border_strong;
        base.relative()
            .size(px(64.))
            .flex_shrink_0()
            .rounded(px(RADIUS_SM))
            .overflow_hidden()
            .bg(t.bg_sunken)
            .border_1()
            .border_color(t.border_subtle)
            .hover(move |s| s.border_color(strong))
            .child(
                img(thumb_source(ThumbKey::Stored(file.id)))
                    .size_full()
                    .object_fit(ObjectFit::Cover),
            )
            .into_any_element()
    } else {
        chip(base, file_icon(file.kind), &file.name, size, style, t).into_any_element()
    }
}

fn file_icon(kind: FileKind) -> Icon {
    match kind {
        FileKind::Text | FileKind::Pdf => Icon::FileText,
        FileKind::Image | FileKind::Other => Icon::File,
    }
}

/// A file chip: 28 high, padding 0 x 8, `radius.sm`, 1px
/// `border.default`; kind icon, the name cut in the middle, the size.
fn chip(
    base: gpui_kit::Stateful<gpui_kit::Div>,
    glyph: Icon,
    name: &str,
    size: String,
    style: StripStyle,
    t: &Tokens,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    let (hover, secondary) = (t.fill_hover, t.text_secondary);
    let group = SharedString::from(format!("chip-{name}"));
    let name_color = if style == StripStyle::Filed {
        t.text_secondary
    } else {
        t.text_primary
    };
    base.group(group.clone())
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.))
        .h(px(28.))
        .px(px(8.))
        .rounded(px(RADIUS_SM))
        .border_1()
        .border_color(t.border_default)
        .hover(move |s| s.bg(hover))
        .child(icon(glyph, 16., t.text_secondary))
        .child(
            type_scale::UI
                .el()
                .max_w(px(200.))
                .whitespace_nowrap()
                .overflow_hidden()
                .text_color(name_color)
                .child(attach::middle_name(name, attach::CHIP_NAME_CHARS)),
        )
        .child(
            type_scale::SMALL
                .el()
                .whitespace_nowrap()
                .font_features(type_scale::tabular())
                .text_color(t.text_tertiary)
                .group_hover(group, move |s| s.text_color(secondary))
                .child(size),
        )
}

// ---- an editor's files ----

/// What the clipboard holds, as far as a paste in an editor cares.
#[derive(Clone, Debug, Default)]
pub struct Clip {
    pub text: bool,
    pub files: Vec<PathBuf>,
    /// An image's bytes (PNG, or any format GPUI read from the clipboard).
    pub image: Option<Vec<u8>>,
}

impl Clip {
    pub fn of(item: &ClipboardItem) -> Self {
        let mut clip = Clip::default();
        for entry in item.entries() {
            match entry {
                ClipboardEntry::String(s) => clip.text |= !s.text().is_empty(),
                ClipboardEntry::ExternalPaths(p) => clip.files.extend(p.paths().iter().cloned()),
                ClipboardEntry::Image(image) => {
                    if clip.image.is_none() {
                        clip.image = Some(image.bytes.clone());
                    }
                }
            }
        }
        clip
    }
}

/// One file of an editor, shared with the background work that stores it
/// (which goes on if the editor closes, and drops what it stored then).
#[derive(Default)]
struct Slot {
    size: Option<u64>,
    done: u64,
    result: Option<Result<Staged, Refusal>>,
    /// The editor was cancelled: what is stored goes again.
    discarded: bool,
}

/// The files an editor is adding (components-attachments.md 5, Adding
/// files): staged in the store as soon as they arrive, given their rows
/// when the note or entry is saved.
pub struct Tray {
    ws: Entity<Workspace>,
    /// The record being edited, if it exists already: its files show in
    /// the editor's strip, and count toward the 20.
    pub owner: Option<AttachOwner>,
    held: Vec<Attachment>,
    draft: Draft,
    slots: HashMap<u64, Arc<Mutex<Slot>>>,
    focus: HashMap<u64, FocusHandle>,
    _tick: Task<()>,
    /// Touches the held pending blobs every 30 minutes.
    _keep: Task<()>,
    /// A note card at rest: what is dropped on it is attached once stored,
    /// in this project.
    auto: Option<ProjectId>,
}

impl Tray {
    /// An editor's files. `held` is what the record being edited already
    /// holds (none for a new note): shown in the strip, and counted
    /// toward the 20 from the first file added.
    pub fn new(
        ws: Entity<Workspace>,
        owner: Option<AttachOwner>,
        held: Vec<Attachment>,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut draft = Draft::default();
        draft.held = held.len();
        Self {
            ws,
            owner,
            held,
            draft,
            slots: HashMap::new(),
            focus: HashMap::new(),
            _tick: Task::ready(()),
            _keep: Self::keep(cx),
            auto: None,
        }
    }

    /// Every 30 minutes, the pending blobs of the files this editor holds
    /// are touched, so a purge in another process never takes them (this
    /// process's purge skips them anyway).
    fn keep(cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |tray, cx| {
            loop {
                cx.background_executor().timer(KEEP_ALIVE).await;
                let Ok(()) = tray.update(cx, |tray, cx| tray.touch(cx)) else {
                    return;
                };
            }
        })
    }

    fn touch(&self, cx: &mut Context<Self>) {
        let staged: Vec<Staged> = self
            .slots
            .values()
            .filter_map(
                |s| match &s.lock().unwrap_or_else(|p| p.into_inner()).result {
                    Some(Ok(staged)) => Some(staged.clone()),
                    _ => None,
                },
            )
            .collect();
        if !staged.is_empty() {
            self.ws
                .read(cx)
                .db
                .run(cx, move |store| store.touch_staged(&staged))
                .detach();
        }
    }

    /// The drop target of a note card at rest (components-attachments.md
    /// 5, Drop): `held` is what the card shows; dropped files are attached
    /// as soon as they are stored.
    pub fn for_card(
        ws: Entity<Workspace>,
        owner: AttachOwner,
        project: ProjectId,
        held: Vec<Attachment>,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut tray = Self::new(ws, Some(owner), held, cx);
        tray.auto = Some(project);
        tray
    }

    /// A card's own files changed (a reload).
    pub fn set_held(&mut self, held: Vec<Attachment>) {
        self.draft.held = held.len();
        self.held = held;
    }

    /// The refusal lines alone (a card at rest).
    pub fn render_refusals(&self, cx: &App) -> Vec<AnyElement> {
        let t = cx.global::<Tokens>();
        let locale = settings::locale(cx);
        self.draft
            .refusals
            .iter()
            .map(|r| {
                line(
                    Icon::TriangleAlert,
                    t.danger,
                    r.message(locale),
                    t.text_primary,
                )
            })
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.draft.files.is_empty()
    }

    /// `Mod+V`: files or an image without text are attached (and the paste
    /// is taken); text is left to the editor.
    pub fn paste(&mut self, clip: Clip, cx: &mut Context<Self>) -> bool {
        match attach::paste_kind(clip.text, &clip.files, clip.image.is_some()) {
            Paste::Text => false,
            Paste::Files(paths) => {
                self.add_paths(paths, cx);
                true
            }
            Paste::Image => {
                if let Some(bytes) = clip.image {
                    self.add_image(bytes, cx);
                }
                true
            }
        }
    }

    /// Files from a drop or the open dialog, as one group.
    pub fn add_paths(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        let group = self.draft.start_group();
        for path in paths {
            let name = attach::name_of(&path);
            self.begin(group, name, AttachSource::Path(path), cx);
        }
        cx.notify();
    }

    /// A pasted image, stored as a PNG named after the time.
    pub fn add_image(&mut self, bytes: Vec<u8>, cx: &mut Context<Self>) {
        let group = self.draft.start_group();
        let name = attach::pasted_name(
            settings::locale(cx),
            crate::when::local(acta_core::time::now_ms()),
        );
        self.begin(group, name, AttachSource::Bytes(bytes), cx);
        cx.notify();
    }

    fn begin(&mut self, group: u64, name: String, source: AttachSource, cx: &mut Context<Self>) {
        let now = now_ms();
        let Some(key) = self.draft.begin(group, name.clone(), now) else {
            return;
        };
        // An image is laid out as a tile from the start, by its name.
        if guess_image(&name)
            && let Some(f) = self.draft.files.iter_mut().find(|f| f.key == key)
        {
            f.kind = Some(FileKind::Image);
        }
        let slot = Arc::new(Mutex::new(Slot::default()));
        self.slots.insert(key, Arc::clone(&slot));
        let db = self.ws.read(cx).db.clone();
        cx.background_executor()
            .spawn(async move { stage(db, source, name, slot) })
            .detach();
        self.tick(cx);
    }

    /// While files are being stored, their progress is looked at every
    /// 100 ms; the tick stops once every file is settled.
    fn tick(&mut self, cx: &mut Context<Self>) {
        self._tick = cx.spawn(async move |tray, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                let settled = tray
                    .update(cx, |tray, cx| {
                        tray.sync();
                        cx.notify();
                        let settled = tray.draft.settled();
                        if settled && let (Some(project), Some(owner)) = (tray.auto, tray.owner) {
                            tray.commit(owner, project, cx);
                        }
                        settled
                    })
                    .unwrap_or(true);
                if settled {
                    return;
                }
            }
        });
    }

    /// Copy what the background work learned into the draft.
    fn sync(&mut self) {
        let keys: Vec<u64> = self.draft.files.iter().map(|f| f.key).collect();
        for key in keys {
            let Some(slot) = self.slots.get(&key) else {
                continue;
            };
            let (size, done, result) = {
                let s = slot.lock().unwrap_or_else(|p| p.into_inner());
                (s.size, s.done, s.result.clone())
            };
            let known = self
                .draft
                .files
                .iter()
                .find(|f| f.key == key)
                .and_then(|f| f.size);
            if let (Some(size), None) = (size, known)
                && !self.draft.sized(key, size)
            {
                self.slots.remove(&key);
                continue;
            }
            self.draft.progress(key, done);
            match result {
                Some(Ok(staged)) => {
                    let pending = self
                        .draft
                        .files
                        .iter()
                        .any(|f| f.key == key && f.staged.is_none());
                    if pending {
                        self.draft.ready(key, staged);
                    }
                }
                Some(Err(refusal)) => {
                    self.draft.failed(key, refusal);
                    self.slots.remove(&key);
                }
                None => {}
            }
        }
    }

    /// Remove on a file not saved yet.
    pub fn remove_draft(&mut self, key: u64, cx: &mut Context<Self>) {
        self.draft.remove(key);
        if let Some(slot) = self.slots.remove(&key) {
            self.discard_slot(slot, cx);
        }
        cx.notify();
    }

    fn discard_slot(&self, slot: Arc<Mutex<Slot>>, cx: &mut Context<Self>) {
        let staged = {
            let mut s = slot.lock().unwrap_or_else(|p| p.into_inner());
            s.discarded = true;
            match &s.result {
                Some(Ok(staged)) => Some(staged.clone()),
                _ => None,
            }
        };
        if let Some(staged) = staged {
            self.ws
                .read(cx)
                .db
                .run(cx, move |store| store.discard_staged(&staged))
                .detach();
        }
    }

    /// The editor was cancelled: its new files go (the record's own stay).
    pub fn discard(&mut self, cx: &mut Context<Self>) {
        for (_, slot) in std::mem::take(&mut self.slots) {
            self.discard_slot(slot, cx);
        }
        self.draft.clear(false);
        cx.notify();
    }

    /// The note or entry was saved as `owner`: each group of files gets
    /// its rows once stored, one undo step per group, in the background
    /// (saving is never blocked). The editor starts empty again.
    pub fn commit(&mut self, owner: AttachOwner, project: ProjectId, cx: &mut Context<Self>) {
        // Their age restarts now; they keep counting toward the 20 until
        // the record's files are read again.
        self.touch(cx);
        let files: Vec<(DraftFile, Arc<Mutex<Slot>>)> = self
            .draft
            .hand_over()
            .into_iter()
            .filter_map(|f| self.slots.remove(&f.key).map(|s| (f, s)))
            .collect();
        // A record that stays (a card, an edited note) keeps counting them;
        // a capture's next note starts from none.
        if self.owner.is_some() {
            self.draft.held += files.len();
        }
        cx.notify();
        if files.is_empty() {
            return;
        }
        let ws = self.ws.clone();
        cx.spawn(async move |_, cx| {
            // Wait for every file to be stored (or refused).
            loop {
                let open = files
                    .iter()
                    .any(|(_, s)| s.lock().unwrap_or_else(|p| p.into_inner()).result.is_none());
                if !open {
                    break;
                }
                cx.background_executor().timer(TICK).await;
            }
            let mut groups: Vec<(u64, Vec<(String, Staged)>)> = Vec::new();
            for (f, slot) in &files {
                let result = slot
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .result
                    .clone();
                let Some(Ok(staged)) = result else { continue };
                match groups.iter_mut().find(|(g, _)| *g == f.group) {
                    Some((_, list)) => list.push((f.name.clone(), staged)),
                    None => groups.push((f.group, vec![(f.name.clone(), staged)])),
                }
            }
            ws.update(cx, |ws, cx| {
                for (_, list) in groups {
                    ws.undoable(cx, Some(project), None, move |store| {
                        Ok((attach_group(store, owner, &list)?, ()))
                    });
                }
            });
        })
        .detach();
    }

    /// The open dialog (the paperclip, `Mod+Shift+A`), several files.
    pub fn pick(&mut self, cx: &mut Context<Self>) {
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: None,
        });
        cx.spawn(async move |tray, cx| {
            if let Ok(Ok(Some(paths))) = chosen.await {
                let _ = tray.update(cx, |tray, cx| tray.add_paths(paths, cx));
            }
        })
        .detach();
    }

    fn draft_focus(&mut self, key: u64, cx: &mut App) -> FocusHandle {
        self.focus
            .entry(key)
            .or_insert_with(|| cx.focus_handle())
            .clone()
    }

    /// The editor's strip (the record's files, then the new ones), the
    /// refusals and the large-file notice; `None` when there is nothing.
    pub fn render_strip(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let now = now_ms();
        let stored = strip("tray", &self.held, StripStyle::Full, &t, cx);
        let mut tiles = Vec::new();
        let mut chips = Vec::new();
        let ordered: Vec<DraftFile> = self.draft.ordered().into_iter().cloned().collect();
        let me = cx.entity().downgrade();
        for f in &ordered {
            let handle = self.draft_focus(f.key, cx);
            let key = f.key;
            let label = f
                .adding_label(locale, now)
                .or_else(|| f.size.map(|s| attach::size_label(locale, s)))
                .unwrap_or_default();
            let base = div()
                .id(SharedString::from(format!("draft-file-{key}")))
                .track_focus(&handle)
                .tab_index(0)
                .focus_visible({
                    let ring = ui::ring_shadow(&t, false);
                    move |s| s.shadow(ring)
                })
                .tooltip(ui::tip(format!("{} \u{b7} {label}", f.name)))
                .on_key_down(cx.listener(move |tray, event: &KeyDownEvent, _, cx| {
                    if event.keystroke.key == "delete" {
                        tray.remove_draft(key, cx);
                        cx.stop_propagation();
                    }
                }))
                .on_mouse_down(gpui_kit::MouseButton::Right, {
                    let me = me.clone();
                    move |event, _, cx| {
                        if let Some(hub) = hub(cx) {
                            let target = MenuTarget::Draft {
                                tray: me.clone(),
                                key,
                            };
                            hub.update(cx, |_, cx| {
                                cx.emit(FilesEvent::Menu(target, event.position))
                            });
                        }
                        cx.stop_propagation();
                    }
                });
            if f.is_image() {
                let strong = t.border_strong;
                let thumb = f.staged.clone().map(|s| {
                    img(thumb_source(ThumbKey::Staged(s)))
                        .size_full()
                        .object_fit(ObjectFit::Cover)
                });
                tiles.push(
                    base.size(px(64.))
                        .flex_shrink_0()
                        .rounded(px(RADIUS_SM))
                        .overflow_hidden()
                        .bg(t.bg_sunken)
                        .border_1()
                        .border_color(t.border_subtle)
                        .hover(move |s| s.border_color(strong))
                        .children(thumb)
                        .into_any_element(),
                );
            } else {
                let glyph = match f.kind {
                    Some(kind) => file_icon(kind),
                    None => Icon::File,
                };
                chips.push(
                    chip(base, glyph, &f.name, label, StripStyle::Full, &t).into_any_element(),
                );
            }
        }
        let refusals: Vec<AnyElement> = self
            .draft
            .refusals
            .iter()
            .map(|r| {
                line(
                    Icon::TriangleAlert,
                    t.danger,
                    r.message(locale),
                    t.text_primary,
                )
            })
            .collect();
        let notice = self.draft.notice.then(|| {
            line(
                Icon::Info,
                t.text_tertiary,
                text(locale, Key::AttachLargeNotice),
                t.text_tertiary,
            )
        });
        if stored.is_none()
            && tiles.is_empty()
            && chips.is_empty()
            && refusals.is_empty()
            && notice.is_none()
        {
            return None;
        }
        Some(
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .children(stored)
                .when(!tiles.is_empty(), |this| {
                    this.child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .gap(px(8.))
                            .children(tiles),
                    )
                })
                .when(!chips.is_empty(), |this| {
                    this.child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .gap(px(8.))
                            .children(chips),
                    )
                })
                .children(refusals)
                .children(notice)
                .into_any_element(),
        )
    }
}

/// The rows of one paste, drop or dialog on `owner`, in the caller's
/// transaction; the undo step takes them all back (components-attachments.md
/// 5, Undo).
pub fn attach_group(
    store: &mut Store,
    owner: AttachOwner,
    files: &[(String, Staged)],
) -> acta_core::Result<Undo> {
    let mut steps = Vec::new();
    for (name, staged) in files {
        let a = store.attach_staged(owner, name, staged, Author::Human)?;
        steps.push(Undo::Delete(Record::Attachment(a.id)));
    }
    // Taken back last first, so a redo restores them in their order (a
    // restored file goes to the end of its strip).
    steps.reverse();
    Ok(Undo::Many(steps))
}

/// One line under a strip: a 14 icon and `type.small` text.
fn line(
    glyph: Icon,
    color: gpui_kit::Hsla,
    message: String,
    text_color: gpui_kit::Hsla,
) -> AnyElement {
    type_scale::SMALL
        .el()
        .flex()
        .flex_row()
        .items_start()
        .gap(px(8.))
        .text_color(text_color)
        .child(
            div()
                .h(px(16.))
                .flex()
                .items_center()
                .child(icon(glyph, 14., color)),
        )
        .child(div().flex_1().min_w_0().child(message))
        .into_any_element()
}

fn now_ms() -> u64 {
    acta_core::time::now_ms().max(0) as u64
}

fn guess_image(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp"]
        .iter()
        .any(|e| lower.ends_with(e))
}

/// Off the UI thread: the file's size (refused over 25 MB before any
/// read), then stored in the store's chunks with its progress.
fn stage(db: crate::db::Db, source: AttachSource, name: String, slot: Arc<Mutex<Slot>>) {
    let set = |f: &mut dyn FnMut(&mut Slot)| {
        let mut s = slot.lock().unwrap_or_else(|p| p.into_inner());
        f(&mut s);
    };
    let size = match &source {
        AttachSource::Path(p) => std::fs::metadata(p).map(|m| m.len()),
        AttachSource::Bytes(b) => Ok(b.len() as u64),
    };
    let size = match size {
        Ok(size) => size,
        Err(_) => {
            set(&mut |s| s.result = Some(Err(Refusal::ReadFailed { name: name.clone() })));
            return;
        }
    };
    set(&mut |s| s.size = Some(size));
    if size > MAX_FILE_BYTES {
        set(&mut |s| {
            s.result = Some(Err(Refusal::TooLarge {
                name: name.clone(),
                size,
            }))
        });
        return;
    }
    let source = match source {
        // A pasted image that is not a PNG (a bitmap from the clipboard)
        // is stored as one.
        AttachSource::Bytes(bytes) if !bytes.starts_with(b"\x89PNG") => {
            match acta_core::files::decode(&bytes).and_then(|i| png_bytes(&i)) {
                Some(png) => AttachSource::Bytes(png),
                None => AttachSource::Bytes(bytes),
            }
        }
        other => other,
    };
    // The store's lock is taken per chunk, not for the whole file.
    let slow = dev_slow();
    let result = acta_core::store::stage_with(|| db.lock(), &source, &mut |done, _| {
        {
            let mut s = slot.lock().unwrap_or_else(|p| p.into_inner());
            s.done = done;
        }
        if let Some(pause) = slow {
            std::thread::sleep(pause);
        }
    });
    let mut s = slot.lock().unwrap_or_else(|p| p.into_inner());
    match result {
        Ok(staged) => {
            if s.discarded {
                let _ = db.lock().discard_staged(&staged);
            }
            s.result = Some(Ok(staged));
        }
        Err(acta_core::Error::AttachmentTooLarge { size, .. }) => {
            s.result = Some(Err(Refusal::TooLarge { name, size }));
        }
        Err(err) => {
            crate::log::line(&format!(
                "files: could not store a file: {}",
                crate::workspace::err_kind(&err)
            ));
            s.result = Some(Err(Refusal::ReadFailed { name }));
        }
    }
}

/// Debug builds: `ACTA_DEV_SLOW_ATTACH=1` pauses between stored chunks, so
/// the progress of a file can be captured.
fn dev_slow() -> Option<Duration> {
    (cfg!(debug_assertions) && std::env::var_os("ACTA_DEV_SLOW_ATTACH").is_some())
        .then_some(Duration::from_millis(300))
}

fn png_bytes(image: &image::DynamicImage) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    image
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .ok()?;
    Some(out)
}

/// An editor that takes files: its paste hook, drop target and
/// `Mod+Shift+A`, around `content` (the editor's box). Draws "Drop to
/// attach" over it while files are dragged over the window.
pub fn drop_zone(
    id: &str,
    tray: &Entity<Tray>,
    radius: f32,
    content: gpui_kit::Div,
    cx: &App,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    let t = cx.global::<Tokens>();
    let locale = settings::locale(cx);
    // `accent.subtle` over the target and the line centred in it; the
    // content stays in place under it. The label's own drag-over style
    // (it covers the target) shows it while files are over it.
    let label = type_scale::UI
        .el()
        .id(SharedString::from(format!("drop-label-{id}")))
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius))
        .bg(t.accent_subtle)
        .text_color(t.text_primary)
        .child(text(locale, Key::AttachDrop))
        .invisible()
        .drag_over::<ExternalPaths>(|s, _, _, _| s.visible());
    let (drop_tray, key_tray) = (tray.clone(), tray.clone());
    content
        .id(SharedString::from(format!("drop-zone-{id}")))
        .relative()
        .on_drop(move |paths: &ExternalPaths, _, cx| {
            let paths = paths.paths().to_vec();
            drop_tray.update(cx, |tray, cx| tray.add_paths(paths, cx));
        })
        .capture_key_down(move |event: &KeyDownEvent, _, cx| {
            let k = &event.keystroke;
            if k.key == "a" && k.modifiers.shift && mod_key(&k.modifiers) {
                key_tray.update(cx, |tray, cx| tray.pick(cx));
                cx.stop_propagation();
            }
        })
        .child(label)
}

/// The paperclip of an editor's footer: ghost icon button 24, "Attach
/// files..." with `Mod+Shift+A`.
pub fn attach_button(id: &str, tray: &Entity<Tray>, cx: &App) -> AnyElement {
    let t = cx.global::<Tokens>();
    let locale = settings::locale(cx);
    let macos = cfg!(target_os = "macos");
    let keys = vec![
        KeyName::Mod.label(locale, macos),
        KeyName::Shift.label(locale, macos),
        "A",
    ];
    let tray = tray.clone();
    ui::ghost_icon_button(
        format!("attach-{id}"),
        Icon::Paperclip,
        24.,
        false,
        false,
        true,
        t,
    )
    .tooltip(ui::tip_keys(text(locale, Key::AttachButton), keys))
    .on_click(move |_, _, cx| tray.update(cx, |tray, cx| tray.pick(cx)))
    .into_any_element()
}

/// The paste hook of an editor's `Textarea`.
pub fn on_paste(
    tray: &Entity<Tray>,
) -> impl Fn(&ClipboardItem, &mut Window, &mut App) -> bool + 'static {
    let tray = tray.downgrade();
    move |item, _, cx| {
        let clip = Clip::of(item);
        tray.update(cx, |tray, cx| tray.paste(clip, cx))
            .unwrap_or(false)
    }
}

// ---- the file menu ----

pub enum OverlayEvent {
    Close,
}

/// View, Save as..., Remove (components-attachments.md 5, Keys), at a
/// window position.
pub struct FileMenu {
    target: MenuTarget,
    at: Point<Pixels>,
    highlight: usize,
    focus: FocusHandle,
}

impl EventEmitter<OverlayEvent> for FileMenu {}

impl Focusable for FileMenu {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MenuItem {
    View,
    SaveAs,
    Remove,
}

impl FileMenu {
    pub fn new(
        target: MenuTarget,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        Self {
            target,
            at,
            highlight: 0,
            focus,
        }
    }

    fn items(&self) -> Vec<MenuItem> {
        match &self.target {
            MenuTarget::Stored { file, .. } => {
                // Files the viewer cannot show offer Save as first.
                let mut items = Vec::new();
                if attach::viewable(file) {
                    items.push(MenuItem::View);
                }
                items.push(MenuItem::SaveAs);
                items.push(MenuItem::Remove);
                items
            }
            MenuTarget::Draft { .. } => vec![MenuItem::Remove],
        }
    }

    fn choose(&mut self, item: MenuItem, cx: &mut Context<Self>) {
        cx.emit(OverlayEvent::Close);
        let Some(hub) = hub(cx) else { return };
        match (&self.target, item) {
            (MenuTarget::Stored { file, siblings }, MenuItem::View) => {
                let (list, at) = attach::viewer_list(siblings, file);
                hub.update(cx, |_, cx| cx.emit(FilesEvent::View(list, at)));
            }
            (MenuTarget::Stored { file, .. }, MenuItem::SaveAs) => {
                let file = file.clone();
                hub.update(cx, |h, cx| h.save_as(&file, cx));
            }
            (MenuTarget::Stored { file, .. }, MenuItem::Remove) => {
                let file = file.clone();
                hub.update(cx, |h, cx| h.remove(&file, cx));
            }
            (MenuTarget::Draft { tray, key }, MenuItem::Remove) => {
                let key = *key;
                let _ = tray.update(cx, |tray, cx| tray.remove_draft(key, cx));
            }
            _ => {}
        }
    }
}

impl Render for FileMenu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let macos = cfg!(target_os = "macos");
        let items = self.items();
        let mut panel = crate::menu::panel("file-menu", &t)
            .track_focus(&self.focus)
            .on_mouse_down_out(cx.listener(|_, _, _, cx| cx.emit(OverlayEvent::Close)))
            .on_key_down(cx.listener(move |menu, event: &KeyDownEvent, _, cx| {
                let n = menu.items().len();
                match event.keystroke.key.as_str() {
                    "escape" => cx.emit(OverlayEvent::Close),
                    "down" => menu.highlight = (menu.highlight + 1) % n,
                    "up" => menu.highlight = (menu.highlight + n - 1) % n,
                    "enter" | "space" => {
                        let item = menu.items()[menu.highlight.min(n - 1)];
                        menu.choose(item, cx);
                    }
                    _ => return,
                }
                cx.stop_propagation();
                cx.notify();
            }));
        for (i, item) in items.iter().copied().enumerate() {
            let (label, hint) = match item {
                MenuItem::View => (text(locale, Key::AttachView), None),
                MenuItem::SaveAs => (
                    text(locale, Key::AttachSaveAs),
                    Some(ui::kbd(&[KeyName::Mod.label(locale, macos), "S"], &t).into_any_element()),
                ),
                MenuItem::Remove => (text(locale, Key::AttachRemove), None),
            };
            if item == MenuItem::Remove && i > 0 {
                panel = panel.child(crate::menu::separator(&t));
            }
            panel = panel.child(
                crate::menu::item(
                    ("file-menu-item", i),
                    None,
                    label,
                    hint,
                    self.highlight == i,
                    &t,
                )
                .on_click(cx.listener(move |menu, _, _, cx| menu.choose(item, cx))),
            );
        }
        gpui_kit::anchored()
            .position(self.at)
            .snap_to_window_with_margin(px(8.))
            .child(panel)
    }
}

// ---- the viewer ----

enum Content {
    Loading,
    Image(Arc<RenderImage>, (u32, u32)),
    Text(Entity<gpui_kit::component::input::TextareaState>, bool),
    Failed,
}

/// The viewer (components-attachments.md 5, Viewer): images and text-like
/// files, decrypted in memory only, over the scrim.
pub struct Viewer {
    files: Vec<Attachment>,
    at: usize,
    content: Content,
    /// Images: fitted, or at 100%.
    fit: bool,
    scroll: ScrollHandle,
    /// At 100%, the image dragged by the pointer: where it last was.
    grab: Option<Point<Pixels>>,
    focus: FocusHandle,
    _load: Task<()>,
}

impl EventEmitter<OverlayEvent> for Viewer {}

impl Focusable for Viewer {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Viewer {
    pub fn new(
        files: Vec<Attachment>,
        at: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let mut viewer = Self {
            files,
            at,
            content: Content::Loading,
            fit: true,
            scroll: ScrollHandle::new(),
            grab: None,
            focus,
            _load: Task::ready(()),
        };
        viewer.load(window, cx);
        viewer
    }

    fn file(&self) -> Option<&Attachment> {
        self.files.get(self.at)
    }

    fn load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.drop_image(window, cx);
        self.content = Content::Loading;
        self.fit = true;
        let Some(file) = self.file().cloned() else {
            return;
        };
        let Some(hub) = hub(cx) else { return };
        let db = hub.read(cx).ws.read(cx).db.clone();
        let task = cx.background_executor().spawn(async move {
            let store = db.lock();
            match file.kind {
                FileKind::Image => {
                    let mut bytes = Vec::new();
                    store
                        .read_attachment(file.id)
                        .ok()?
                        .read_to_end(&mut bytes)
                        .ok()?;
                    drop(store);
                    let image = acta_core::files::decode(&bytes)?;
                    let size = (image.width(), image.height());
                    Some(Loaded::Image(render_image(image), size))
                }
                _ => {
                    let prefix = store
                        .attachment_prefix(file.id, attach::TEXT_VIEW_BYTES)
                        .ok()?;
                    let (text, cut) = attach::text_view(&prefix, file.size.max(0) as u64);
                    Some(Loaded::Text(text, cut))
                }
            }
        });
        self._load = cx.spawn_in(window, async move |viewer, cx| {
            let loaded = task.await;
            let _ = viewer.update_in(cx, |viewer, window, cx| {
                viewer.content = match loaded {
                    Some(Loaded::Image(image, size)) => Content::Image(image, size),
                    Some(Loaded::Text(body, cut)) => {
                        let state = cx.new(|cx| {
                            gpui_kit::component::input::TextareaState::new(window, cx)
                                .soft_wrap(false)
                                .default_value(body)
                        });
                        Content::Text(state, cut)
                    }
                    None => Content::Failed,
                };
                cx.notify();
            });
        });
        cx.notify();
    }

    /// The decoded image leaves GPU memory with the viewer.
    fn drop_image(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Content::Image(image, _) = &self.content {
            let _ = window.drop_image(Arc::clone(image));
        }
        let _ = cx;
    }

    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.drop_image(window, cx);
        self.content = Content::Loading;
        cx.emit(OverlayEvent::Close);
    }

    fn go(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let to = attach::step(self.at, self.files.len(), forward);
        if to != self.at {
            self.at = to;
            self.load(window, cx);
        }
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let Some(file) = self.file().cloned() else {
            return;
        };
        if let Some(hub) = hub(cx) {
            hub.update(cx, |h, cx| h.save_as(&file, cx));
        }
    }

    fn toggle_fit(&mut self, cx: &mut Context<Self>) {
        if matches!(self.content, Content::Image(..)) {
            self.fit = !self.fit;
            self.scroll.set_offset(Point::default());
            cx.notify();
        }
    }

    fn pan(&mut self, dx: f32, dy: f32) {
        let o = self.scroll.offset();
        let max = self.scroll.max_offset();
        let (x, y) = attach::panned(
            (f32::from(o.x), f32::from(o.y)),
            (f32::from(max.x), f32::from(max.y)),
            (dx, dy),
        );
        self.scroll.set_offset(Point { x: px(x), y: px(y) });
    }

    /// At 100%, the pointer drags the image: it follows the pointer.
    fn drag_to(&mut self, at: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(from) = self.grab else { return };
        self.grab = Some(at);
        self.pan(f32::from(from.x - at.x), f32::from(from.y - at.y));
        cx.notify();
    }
}

enum Loaded {
    Image(Arc<RenderImage>, (u32, u32)),
    Text(String, bool),
}

impl Render for Viewer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let macos = cfg!(target_os = "macos");
        let viewport = window.viewport_size();
        let (vw, vh) = (f32::from(viewport.width), f32::from(viewport.height));
        let inset = if vw < 880. || vh < 560. { 24. } else { 32. };
        let Some(file) = self.file().cloned() else {
            return div().into_any_element();
        };
        let n = self.files.len();
        let header = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .h(px(44.))
            .px(px(16.))
            .flex_shrink_0()
            .border_b_1()
            .border_color(t.border_default)
            .child(
                type_scale::UI_STRONG
                    .el()
                    .min_w_0()
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .text_color(t.text_primary)
                    .child(attach::middle_name(&file.name, 60)),
            )
            .child(
                type_scale::SMALL
                    .el()
                    .flex_shrink_0()
                    .whitespace_nowrap()
                    .font_features(type_scale::tabular())
                    .text_color(t.text_tertiary)
                    .child(attach::viewer_meta(locale, &file)),
            )
            .child(div().flex_1())
            .when(n > 1, |this| {
                this.child(
                    type_scale::SMALL
                        .el()
                        .whitespace_nowrap()
                        .font_features(type_scale::tabular())
                        .text_color(t.text_tertiary)
                        .child(tr(
                            locale,
                            Key::AttachPosition,
                            &[
                                ("i", Arg::Count(self.at as u64 + 1)),
                                ("n", Arg::Count(n as u64)),
                            ],
                        )),
                )
                .child(
                    ui::ghost_icon_button(
                        "viewer-prev",
                        Icon::ChevronLeft,
                        24.,
                        false,
                        self.at == 0,
                        true,
                        &t,
                    )
                    .tooltip(ui::tip_keys(
                        text(locale, Key::AttachPrev),
                        vec!["\u{2190}"],
                    ))
                    .on_click(cx.listener(|v, _, window, cx| v.go(false, window, cx))),
                )
                .child(
                    ui::ghost_icon_button(
                        "viewer-next",
                        Icon::ChevronRight,
                        24.,
                        false,
                        self.at + 1 >= n,
                        true,
                        &t,
                    )
                    .tooltip(ui::tip_keys(
                        text(locale, Key::AttachNext),
                        vec!["\u{2192}"],
                    ))
                    .on_click(cx.listener(|v, _, window, cx| v.go(true, window, cx))),
                )
            })
            .child(
                ui::button(
                    "viewer-save",
                    text(locale, Key::AttachSaveAs),
                    Variant::Ghost,
                    false,
                    &t,
                )
                .tooltip(ui::tip_keys(
                    text(locale, Key::AttachSaveAs),
                    vec![KeyName::Mod.label(locale, macos), "S"],
                ))
                .on_click(cx.listener(|v, _, _, cx| v.save(cx))),
            )
            .child(
                ui::ghost_icon_button("viewer-close", Icon::X, 24., false, false, true, &t)
                    .tooltip(ui::tip_keys(
                        text(locale, Key::CommonClose),
                        vec![KeyName::Esc.label(locale, macos)],
                    ))
                    .on_click(cx.listener(|v, _, window, cx| v.close(window, cx))),
            );
        // The body's room: the layer less its border and the header.
        let area = (vw - 2. * inset - 2., vh - 2. * inset - 44. - 2.);
        let body: AnyElement = match &self.content {
            Content::Loading => div().flex_1().bg(t.bg_sunken).into_any_element(),
            Content::Failed => div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .bg(t.bg_sunken)
                .child(type_scale::UI.el().text_color(t.text_secondary).child(tr(
                    locale,
                    Key::AttachReadFailed,
                    &[("name", Arg::Text(&file.name))],
                )))
                .into_any_element(),
            Content::Image(image, (w, h)) => {
                let (w, h) = (*w as f32, *h as f32);
                let scale = if self.fit {
                    attach::fit_scale((w, h), area)
                } else {
                    1.
                };
                let (dw, dh) = (w * scale, h * scale);
                let fit = self.fit;
                let tip = text(
                    locale,
                    if fit {
                        Key::AttachActual
                    } else {
                        Key::AttachFit
                    },
                );
                div()
                    .id("viewer-image-scroll")
                    .flex_1()
                    .min_h_0()
                    .bg(t.bg_sunken)
                    .rounded_b(px(RADIUS_XL - 1.))
                    .when(!fit, |this| {
                        // At 100% the pointer drags the image (the wheel
                        // still scrolls it).
                        let grabbing = self.grab.is_some();
                        this.overflow_scroll()
                            .track_scroll(&self.scroll)
                            .cursor(if grabbing {
                                gpui_kit::CursorStyle::ClosedHand
                            } else {
                                gpui_kit::CursorStyle::OpenHand
                            })
                            .on_mouse_down(
                                gpui_kit::MouseButton::Left,
                                cx.listener(|v, e: &gpui_kit::MouseDownEvent, _, cx| {
                                    v.grab = Some(e.position);
                                    cx.notify();
                                }),
                            )
                            .on_mouse_move(cx.listener(|v, e: &gpui_kit::MouseMoveEvent, _, cx| {
                                if e.pressed_button == Some(gpui_kit::MouseButton::Left) {
                                    v.drag_to(e.position, cx);
                                } else if v.grab.take().is_some() {
                                    cx.notify();
                                }
                            }))
                            .on_mouse_up(
                                gpui_kit::MouseButton::Left,
                                cx.listener(|v, _, _, cx| {
                                    v.grab = None;
                                    cx.notify();
                                }),
                            )
                    })
                    .child(
                        div()
                            .min_w_full()
                            .min_h_full()
                            .w(px(dw.max(area.0)))
                            .h(px(dh.max(area.1)))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                img(ImageSource::Render(Arc::clone(image)))
                                    .id("viewer-image")
                                    .w(px(dw))
                                    .h(px(dh))
                                    .tooltip(ui::tip_keys(tip, vec!["0"]))
                                    .on_click(cx.listener(
                                        |v, event: &gpui_kit::ClickEvent, _, cx| {
                                            if event.click_count() == 2 {
                                                v.toggle_fit(cx);
                                            }
                                        },
                                    )),
                            ),
                    )
                    .into_any_element()
            }
            Content::Text(state, cut) => div()
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .child(
                    type_scale::CODE
                        .el()
                        .flex_1()
                        .min_h_0()
                        .p(px(16.))
                        .text_color(t.text_primary)
                        .child(
                            gpui_kit::component::input::Textarea::new(state)
                                .appearance(false)
                                .readonly(true)
                                .h_full()
                                .font_family(crate::theme::FONT_MONO)
                                .text_size(px(type_scale::CODE.size()))
                                .line_height(px(type_scale::CODE.line())),
                        ),
                )
                .when(*cut, |this| {
                    this.child(
                        div()
                            .px(px(16.))
                            .py(px(8.))
                            .border_t_1()
                            .border_color(t.border_default)
                            .child(line(
                                Icon::Info,
                                t.text_tertiary,
                                text(locale, Key::AttachTruncated),
                                t.text_secondary,
                            )),
                    )
                })
                .into_any_element(),
        };
        div()
            .id("viewer-scrim")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(t.scrim)
            .occlude()
            .child(
                div()
                    .id("viewer")
                    .track_focus(&self.focus)
                    .absolute()
                    .top(px(inset))
                    .left(px(inset))
                    .right(px(inset))
                    .bottom(px(inset))
                    .flex()
                    .flex_col()
                    .bg(t.bg_overlay)
                    .border_1()
                    .border_color(t.border_default)
                    .rounded(px(RADIUS_XL))
                    .shadow(vec![t.shadow_overlay()])
                    .overflow_hidden()
                    .on_key_down(cx.listener(|v, event: &KeyDownEvent, window, cx| {
                        let k = &event.keystroke;
                        let image = matches!(v.content, Content::Image(..));
                        let step = 48.;
                        match k.key.as_str() {
                            "escape" => v.close(window, cx),
                            "s" if mod_key(&k.modifiers) => v.save(cx),
                            "0" if image => v.toggle_fit(cx),
                            "left" if image && k.modifiers.shift => v.pan(-step, 0.),
                            "right" if image && k.modifiers.shift => v.pan(step, 0.),
                            "up" if image && k.modifiers.shift => v.pan(0., -step),
                            "down" if image && k.modifiers.shift => v.pan(0., step),
                            "left" if !k.modifiers.modified() && v.focus.is_focused(window) => {
                                v.go(false, window, cx)
                            }
                            "right" if !k.modifiers.modified() && v.focus.is_focused(window) => {
                                v.go(true, window, cx)
                            }
                            _ => return,
                        }
                        cx.stop_propagation();
                        cx.notify();
                    }))
                    .child(header)
                    .child(body),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_support::temp_store;

    fn stage(store: &mut Store, bytes: &[u8]) -> Staged {
        store
            .stage(&AttachSource::Bytes(bytes.to_vec()), &mut |_, _| {})
            .unwrap()
    }

    #[test]
    fn one_drop_is_one_undo_step_and_the_redo_brings_the_files_back() {
        let mut temp = temp_store();
        let s = &mut temp.store;
        let p = s.create_project("Acta", "", Author::Human).unwrap();
        let note = s.add_note(p, "Logs from Alexandre", Author::Human).unwrap();
        let owner = AttachOwner::Note(note);
        let files = vec![
            ("a.log".to_owned(), stage(s, b"first log")),
            ("b.log".to_owned(), stage(s, b"second log")),
        ];
        let undo = s.atomically(|s| attach_group(s, owner, &files)).unwrap();
        assert_eq!(s.attachments(owner).unwrap().len(), 2);
        let redo = undo.apply(s).unwrap().expect("an inverse");
        assert!(s.attachments(owner).unwrap().is_empty(), "both go");
        redo.apply(s).unwrap();
        let names: Vec<String> = s
            .attachments(owner)
            .unwrap()
            .into_iter()
            .map(|a| a.name)
            .collect();
        assert_eq!(names, ["a.log", "b.log"]);
    }

    #[test]
    fn a_save_as_writes_the_whole_file_and_nothing_on_failure() {
        let mut temp = temp_store();
        let s = &mut temp.store;
        let p = s.create_project("Acta", "", Author::Human).unwrap();
        let note = s.add_note(p, "Quote", Author::Human).unwrap();
        let data: Vec<u8> = (0..3_000_000u32).map(|i| (i % 251) as u8).collect();
        let a = s
            .attach(
                AttachOwner::Note(note),
                "quote.bin",
                AttachSource::Bytes(data.clone()),
                Author::Human,
                &mut |_, _| {},
            )
            .unwrap();
        let out = temp.dir.path().join("quote.bin");
        write_file(s, a.id, &out).unwrap();
        assert_eq!(std::fs::read(&out).unwrap(), data);
        // A folder that does not exist: an error, no file.
        let bad = temp.dir.path().join("missing").join("quote.bin");
        assert!(write_file(s, a.id, &bad).is_err());
        assert!(!bad.exists());
    }

    /// A reader that fails after some bytes (a disk or store error mid-copy).
    struct Failing(usize);

    impl Read for Failing {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            if self.0 == 0 {
                return Err(std::io::Error::other("broken"));
            }
            let n = out.len().min(self.0);
            out[..n].fill(7);
            self.0 -= n;
            Ok(n)
        }
    }

    #[test]
    fn a_failed_save_leaves_the_existing_file_and_no_temporary_one() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("quote.pdf");
        std::fs::write(&target, b"Alexandre's signed copy").unwrap();
        assert!(write_replacing(&mut Failing(100_000), &target).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"Alexandre's signed copy");
        let left: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
        assert_eq!(left.len(), 1, "only the target");
        // A good save replaces it.
        write_replacing(&mut &b"new"[..], &target).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"new");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn a_clipboard_is_read_as_text_files_or_an_image() {
        let text = ClipboardItem::new_string("Alexandre".into());
        let clip = Clip::of(&text);
        assert!(clip.text);
        assert!(clip.files.is_empty() && clip.image.is_none());
        let image = gpui_kit::Image::from_bytes(gpui_kit::ImageFormat::Png, b"\x89PNG".to_vec());
        let clip = Clip::of(&ClipboardItem::new_image(&image));
        assert!(!clip.text);
        assert_eq!(clip.image.as_deref(), Some(&b"\x89PNG"[..]));
    }

    #[test]
    fn only_image_names_lay_out_as_tiles_before_they_are_read() {
        assert!(guess_image("Pasted image 2026-11-21 14.05.png"));
        assert!(guess_image("IMG_2041.JPG"));
        assert!(!guess_image("trace.zip"));
        assert!(!guess_image("png"));
    }
}
