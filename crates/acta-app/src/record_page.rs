//! The project, epic and topic pages (docs/design/layout.md 4.2; screens.md
//! 8.1, 8.6, 8.14, 8.20): title, the properties block, then the sections in
//! their fixed order. A topic: current state, description, knowledge,
//! timeline. An epic: "Needs attention" when something does, description,
//! knowledge, topic table. A project: the Overview, description, knowledge,
//! the topic table grouped by epic.
//!
//! Writes are sent to the workspace and the page reloads when its revision
//! moves, whoever wrote; an editor being used is never overwritten by a
//! reload (interaction.md 6.5 rule 4: the human's text stays).

use std::path::PathBuf;

use crate::zoom::{self, px};
use acta_core::date::{Date, DueState, due_state};
use acta_core::model::{
    AttachOwner, Author, Entry, EntryId, EntryKind, KbId, KbKind, KbSummary, Owner, Record, Status,
    TopicId, TopicSummary,
};
use acta_core::store::{EpicPatch, OverviewFilter, ProjectPatch, TopicPatch};
use gpui_kit::component::input::{Escape, Input, InputEvent, InputState, Textarea, TextareaState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, AppContext, ClipboardItem, Context, Entity, EventEmitter, FocusHandle,
    Focusable, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, PathPromptOptions,
    Render, SharedString, StatefulInteractiveElement, Styled, Subscription, Task, Window, div,
};

use crate::due::{self, Outcome, Tone};
use crate::due_picker::{AnchorBounds, DuePicker, DuePickerEvent};
use crate::i18n::{Arg, Key, KeyName, Locale, number, text, tr};
use crate::icons::{Icon, icon};
use crate::kb_model;
use crate::markdown_view;
use crate::menu;
use crate::paths_view;
use crate::record::{self, EntryFilterKind, Loaded};
use crate::route::Route;
use crate::settings;
use crate::table::{self, Column};
use crate::theme::{RADIUS_LG, RADIUS_SM, RADIUS_XS, Tokens};
use crate::type_scale;
use crate::ui::{self, Variant};
use crate::when;
use crate::workspace::{FolderRef, Undo, Workspace};

mod folders;
pub mod questions;

/// What a page asks the window to do.
/// A knowledge row drawn: its entry, revision, owner, kind and state (the
/// palette's target).
type KbRowId = (KbId, i64, Owner, KbKind, acta_core::model::KbState);

#[derive(Clone, Debug, PartialEq)]
pub enum PageEvent {
    Open(Route),
    /// A draft was saved as `route`: the history steps to it (the draft
    /// itself took no step, so Back returns to where it was started).
    Saved(Route),
    /// Start a document or memory owned by this page's record.
    NewKb(crate::kb_page::Draft),
    /// The page learned which project it belongs to (a document opened by
    /// id): the active project follows it.
    Located(acta_core::model::ProjectId),
    /// "Show terminal" on the Claude Code line: the page's own session
    /// (layout.md 4.5.1).
    ShowTerminal(Owner),
    /// Show the palette as a picker of these rows; the chosen one is
    /// handed to the page's callback.
    Pick(crate::palette::Picker, crate::palette::OnPick),
    /// Ask first (interaction.md 6.4): a git action in a folder where a
    /// Claude Code session is working.
    Confirm(crate::dialogs::Confirm),
    /// "File with Claude Code" (screens.md 8.2): run the triage command in
    /// the project's terminal.
    Triage(acta_core::model::ProjectId),
    /// The knowledge page with a chip: the Overview's memory line ("To
    /// check", Kind: Memories), or the palette.
    KbChip(
        acta_core::model::ProjectId,
        crate::kb_model::Chip,
        Option<KbKind>,
    ),
    /// A merge's survivor opens in its editor (components-knowledge.md 5,
    /// Merge).
    EditKb(KbId),
    /// The Overview's late answers: the Questions page of this scope with
    /// Overdue selected.
    LateAnswers(Owner),
    /// "Logged as a decision": the topic opens with that entry focused
    /// (components-questions.md 5, Meta line).
    OpenEntry(TopicId, EntryId),
}

gpui_kit::actions!(
    acta_page,
    [
        /// `S`: the status menu of the page's topic.
        OpenStatusMenu,
        /// `E` / `F2`: rename the page's record.
        EditTitle,
        /// `D`: the due date picker of the page's record.
        OpenDuePicker,
    ]
);

pub struct RecordPage {
    ws: Entity<Workspace>,
    pub owner: Owner,
    data: Option<Loaded>,
    /// The record went to the trash while open (interaction.md 6.5 rule 6).
    gone: bool,
    filter: EntryFilterKind,
    focus: FocusHandle,
    title_edit: Option<Entity<InputState>>,
    saving_title: bool,
    state_edit: Option<Entity<TextareaState>>,
    description_edit: Option<Entity<TextareaState>>,
    /// The files the description editor is adding (topics and epics).
    description_tray: Option<Entity<crate::attach_ui::Tray>>,
    /// The files the entry editor is adding.
    entry_tray: Option<Entity<crate::attach_ui::Tray>>,
    /// The status menu, with its highlighted row.
    status_menu: Option<usize>,
    /// The entry menu (components-records.md 5, Timeline entry).
    entry_menu: Option<EntryMenu>,
    /// An entry being edited in place, with its editor.
    entry_edit: Option<(EntryId, Entity<TextareaState>)>,
    /// The page's scroll, and a newly opened entry editor to bring into
    /// view once it is laid out (it may open near the bottom).
    scroll: gpui_kit::ScrollHandle,
    reveal_editor: std::rc::Rc<std::cell::Cell<bool>>,
    /// The knowledge section's New menu, with its highlighted row.
    kb_new_menu: Option<usize>,
    /// Where the status, entry and knowledge menus hang.
    status_anchor: AnchorBounds,
    entry_anchor: AnchorBounds,
    kb_new_anchor: AnchorBounds,
    /// The working folders (components-folders.md 5).
    folder_ui: folders::FolderUi,
    table_filter: table::Filter,
    table_sort: Option<table::Sort>,
    kb_open: Option<KbId>,
    /// The unfolded memory's notice and body, once loaded.
    kb_open_q: Option<crate::kbq_ui::EntryQuality>,
    kb_inherited_open: bool,
    /// Memory quality of the section's rows (ADR 0015): the store's flags
    /// with the page, git's later; one focus handle and entry per row
    /// drawn (`V`, the palette's target).
    kb_flags: crate::kbq_model::FlagSet,
    kb_row_focus: std::cell::RefCell<Vec<FocusHandle>>,
    kb_row_ids: std::cell::RefCell<Vec<KbRowId>>,
    _kb_git: Task<()>,
    _kb_open: Task<()>,
    id_copied: bool,
    /// The pointer is over the properties block: its add chips show.
    props_hover: bool,
    /// The topic table's body height last laid out, and the least it keeps
    /// after a segment changed.
    table_height: std::rc::Rc<std::cell::Cell<f32>>,
    table_floor: f32,
    seen_revision: u64,
    /// The due date picker, open on the Due row (components-records.md 5).
    due_picker: Option<Entity<DuePicker>>,
    _due_sub: Option<Subscription>,
    due_anchor: AnchorBounds,
    due_focus: FocusHandle,
    /// Scroll the Due row into view once laid out (opened from the
    /// palette).
    reveal_due: std::rc::Rc<std::cell::Cell<bool>>,
    /// The table's filter chip, set from the Overview or the palette.
    chip: Option<OverviewFilter>,
    /// Scroll the table into view and focus its first row once the list
    /// is loaded (a figure of the Overview was opened).
    reveal_table: std::rc::Rc<std::cell::Cell<bool>>,
    focus_first_row: bool,
    table_focus: FocusHandle,
    /// One focus handle per table row, in the order drawn, and the topic
    /// each row showed in the last frame: `D` on a focused row dates it.
    row_focus: Vec<FocusHandle>,
    row_ids: std::cell::RefCell<Vec<TopicId>>,
    /// The due date picker on a table row, anchored to its title.
    table_due: Option<TableDue>,
    /// The Questions section and the Report to row.
    q: questions::QuestionUi,
    /// The blocks `Up` / `Down` step through (interaction.md 6.1), in the
    /// order last drawn, and their focus handles.
    block_order: Vec<record::Block>,
    block_focus: std::collections::HashMap<record::Block, FocusHandle>,
    /// A block to bring into view once laid out (stepped to, or the entry
    /// "Logged as a decision" opened).
    reveal_block: std::rc::Rc<std::cell::Cell<Option<record::Block>>>,
    /// An entry to focus once the timeline is loaded.
    pending_entry: Option<EntryId>,
    _load: Task<()>,
    _subs: Vec<Subscription>,
}

/// The due date picker opened on a row of the topic table (`D`).
struct TableDue {
    row: usize,
    topic: TopicId,
    picker: Entity<DuePicker>,
    anchor: AnchorBounds,
    _sub: Subscription,
}

/// The entry menu: Edit, Copy as Markdown, Change type, Delete; "Change
/// type" turns it into the three types.
#[derive(Clone, Debug)]
struct EntryMenu {
    entry: Entry,
    kinds: bool,
    highlight: usize,
}

/// The types an entry can be changed to, in menu order.
const ENTRY_KINDS: [EntryKind; 3] = [
    EntryKind::Progress,
    EntryKind::Decision,
    EntryKind::Discussion,
];

impl EventEmitter<PageEvent> for RecordPage {}

impl Focusable for RecordPage {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

/// The record a page is about, as a trash record.
fn record_of(owner: Owner) -> Record {
    match owner {
        Owner::Project(id) => Record::Project(id),
        Owner::Epic(id) => Record::Epic(id),
        Owner::Topic(id) => Record::Topic(id),
    }
}

impl RecordPage {
    #[cfg(windows)]
    /// The status menu is open (the chat's web view hides under it).
    pub fn menu_open(&self) -> bool {
        self.status_menu.is_some()
            || self.entry_menu.is_some()
            || self.kb_new_menu.is_some()
            || self.due_picker.is_some()
            || self.table_due.is_some()
            || self.questions_menu_open()
            || self.folder_menu_open()
    }

    pub fn new(
        ws: Entity<Workspace>,
        owner: Owner,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let subs = vec![
            cx.observe(&ws, |page, _, cx| page.reload_if_moved(cx)),
            // The Overview counts in local days: a new day recounts.
            cx.observe_global::<due::Today>(|page, cx| page.reload(cx)),
        ];
        let mut page = Self {
            ws,
            owner,
            data: None,
            gone: false,
            filter: EntryFilterKind::All,
            focus: cx.focus_handle(),
            title_edit: None,
            saving_title: false,
            state_edit: None,
            description_edit: None,
            description_tray: None,
            entry_tray: None,
            status_menu: None,
            entry_menu: None,
            entry_edit: None,
            scroll: gpui_kit::ScrollHandle::new(),
            reveal_editor: Default::default(),
            kb_new_menu: None,
            status_anchor: AnchorBounds::default(),
            entry_anchor: AnchorBounds::default(),
            kb_new_anchor: AnchorBounds::default(),
            folder_ui: folders::FolderUi::default(),
            table_filter: table::Filter::Open,
            table_sort: None,
            kb_open: None,
            kb_open_q: None,
            kb_inherited_open: false,
            kb_flags: Default::default(),
            kb_row_focus: Default::default(),
            kb_row_ids: Default::default(),
            _kb_git: Task::ready(()),
            _kb_open: Task::ready(()),
            id_copied: false,
            props_hover: false,
            table_height: Default::default(),
            table_floor: 0.,
            seen_revision: 0,
            due_picker: None,
            _due_sub: None,
            due_anchor: AnchorBounds::default(),
            due_focus: cx.focus_handle(),
            reveal_due: Default::default(),
            chip: None,
            reveal_table: Default::default(),
            focus_first_row: false,
            table_focus: cx.focus_handle(),
            row_focus: Vec::new(),
            row_ids: Default::default(),
            table_due: None,
            q: questions::QuestionUi::default(),
            block_order: Vec::new(),
            block_focus: Default::default(),
            reveal_block: Default::default(),
            pending_entry: None,
            _load: Task::ready(()),
            _subs: subs,
        };
        page.reload(cx);
        page
    }

    pub fn topic(&self) -> Option<TopicId> {
        match self.owner {
            Owner::Topic(id) => Some(id),
            _ => None,
        }
    }

    fn reload_if_moved(&mut self, cx: &mut Context<Self>) {
        let ws = self.ws.read(cx);
        let (revision, epoch) = (ws.revision, ws.git_epoch);
        if revision != self.seen_revision {
            self.seen_revision = revision;
            self.reload(cx);
        } else if self.folder_ui_epoch_moved(epoch) {
            // The window regained focus: the git state is read again.
            self.read_git(cx);
        }
    }

    pub fn reload(&mut self, cx: &mut Context<Self>) {
        let owner = self.owner;
        let filter = self.filter;
        let today = due::today(cx);
        let chip = self.chip;
        let task = self.ws.read(cx).db.run(cx, move |store| {
            record::load(store, owner, filter, today, chip)
        });
        self._load = cx.spawn(async move |page, cx| {
            let loaded = task.await;
            let paths = loaded.as_ref().ok().map(|d| d.folders.paths());
            let _ = page.update(cx, |page, cx| {
                match loaded {
                    Ok(mut data) => {
                        // Until the check answers, folders render as before
                        // (found when new), so nothing flickers.
                        if let Some(old) = &page.data {
                            data.folders.missing = old.folders.missing.clone();
                        }
                        let ids: Vec<KbId> = data.kb_flags.iter().map(|(id, _)| *id).collect();
                        page.kb_flags.set_store(data.kb_flags.clone());
                        page.data = Some(data);
                        page.gone = false;
                        page.read_kb_git(ids, cx);
                        if let Some(id) = page.kb_open {
                            page.load_kb_open(id, cx);
                        }
                    }
                    // Deleted (here or by another process): stay readable
                    // with the notice, keep the last data.
                    Err(acta_core::Error::NotFound(_)) => page.gone = true,
                    Err(err) => crate::log::line(&format!(
                        "page: load failed: {}",
                        crate::workspace::err_kind(&err)
                    )),
                }
                cx.notify();
            });
            // Existence on this computer, checked off the store.
            if let Some(paths) = paths {
                let missing = cx
                    .background_executor()
                    .spawn(async move { crate::workspace::missing_paths(paths) })
                    .await;
                let _ = page.update(cx, |page, cx| {
                    if let Some(data) = page.data.as_mut() {
                        data.folders.missing = missing;
                    }
                    page.read_git(cx);
                    cx.notify();
                });
            }
        });
    }

    // ---- writes ----

    fn project_id(&self) -> Option<acta_core::model::ProjectId> {
        self.data.as_ref().map(|d| d.project.0)
    }

    fn set_status(&mut self, status: Status, cx: &mut Context<Self>) {
        let Some(id) = self.topic() else { return };
        let Some(old) = self
            .data
            .as_ref()
            .and_then(|d| d.topic.as_ref())
            .map(|t| t.status)
        else {
            return;
        };
        self.status_menu = None;
        if old == status {
            cx.notify();
            return;
        }
        // Optimistic: the page shows it in this frame; the reload after the
        // write shows what was stored.
        if let Some(topic) = self.data.as_mut().and_then(|d| d.topic.as_mut()) {
            topic.status = status;
        }
        cx.notify();
        self.ws.update(cx, |ws, cx| ws.set_status(id, status, cx));
    }

    /// Open the due date picker on the Due row (`D`, `Enter` or a click on
    /// the row, "Set due date..."). `reveal`: scroll the row into view by
    /// the least amount (from the palette, the human asked).
    pub fn open_due_picker(&mut self, reveal: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(data) = &self.data else { return };
        let current = data.due_on;
        let today = due::today(cx);
        let picker = cx.new(|cx| DuePicker::new(today, current, window, cx));
        let sub = cx.subscribe_in(
            &picker,
            window,
            |page, _, event: &DuePickerEvent, window, cx| {
                let DuePickerEvent::Done(outcome) = *event;
                page.close_due_picker(outcome, window, cx);
            },
        );
        self.status_menu = None;
        self.due_picker = Some(picker);
        self._due_sub = Some(sub);
        self.reveal_due.set(reveal);
        cx.notify();
    }

    #[cfg(debug_assertions)]
    /// Debug builds' captures: type into the open picker, and press Enter.
    pub fn dev_type_due(
        &mut self,
        input: &str,
        enter: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(picker) = &self.due_picker {
            picker.update(cx, |p, cx| p.dev_type(input, enter, window, cx));
        }
    }

    /// `D`: on a focused row of the topic table, the picker of that topic,
    /// anchored to the row's title; anywhere else on the page, the page's
    /// own Due row (the focused element wins, components-records.md 5).
    pub fn open_due_here(&mut self, reveal: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some((row, topic)) = self.focused_row(window) else {
            self.open_due_picker(reveal, window, cx);
            return;
        };
        let current = self
            .data
            .as_ref()
            .and_then(|d| d.all_topics().into_iter().find(|t| t.id == topic))
            .and_then(|t| t.due_on);
        let today = due::today(cx);
        let picker = cx.new(|cx| DuePicker::new(today, current, window, cx));
        let sub = cx.subscribe_in(
            &picker,
            window,
            |page, _, event: &DuePickerEvent, window, cx| {
                let DuePickerEvent::Done(outcome) = *event;
                let Some(due) = page.table_due.take() else {
                    return;
                };
                if let Outcome::Set(date) = outcome {
                    page.ws
                        .update(cx, |ws, cx| ws.set_due(Owner::Topic(due.topic), date, cx));
                }
                if let Some(handle) = page.row_focus.get(due.row) {
                    window.focus(handle, cx);
                }
                cx.notify();
            },
        );
        self.table_due = Some(TableDue {
            row,
            topic,
            picker,
            anchor: AnchorBounds::default(),
            _sub: sub,
        });
        cx.notify();
    }

    #[cfg(debug_assertions)]
    /// Debug builds' captures: `D` on the table's first row.
    pub fn dev_due_on_first_row(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(first) = self.row_focus.first() {
            window.focus(first, cx);
        }
        self.open_due_here(false, window, cx);
    }

    /// The table row with keyboard focus, and its topic.
    fn focused_row(&self, window: &Window) -> Option<(usize, TopicId)> {
        let row = self.row_focus.iter().position(|h| h.is_focused(window))?;
        let topic = *self.row_ids.borrow().get(row)?;
        Some((row, topic))
    }

    /// What "Set due date..." and "Clear due date" act on from this page:
    /// a focused table row's topic, else the page's record; with its date.
    pub fn due_target(&self, window: &Window) -> Option<(Owner, Option<Date>)> {
        let data = self.data.as_ref()?;
        Some(match self.focused_row(window) {
            Some((_, topic)) => {
                let due = data
                    .all_topics()
                    .into_iter()
                    .find(|t| t.id == topic)
                    .and_then(|t| t.due_on);
                (Owner::Topic(topic), due)
            }
            None => (self.owner, data.due_on),
        })
    }

    /// Close the picker, write what it set, and put focus back on the row.
    fn close_due_picker(&mut self, outcome: Outcome, window: &mut Window, cx: &mut Context<Self>) {
        self.due_picker = None;
        self._due_sub = None;
        if let Outcome::Set(due) = outcome {
            self.set_due(due, cx);
        }
        window.focus(&self.due_focus, cx);
        cx.notify();
    }

    /// Set or clear the date: optimistic, one undo step, no toast.
    pub fn set_due(&mut self, due: Option<Date>, cx: &mut Context<Self>) {
        let Some(data) = self.data.as_mut() else {
            return;
        };
        if data.due_on == due {
            return;
        }
        data.due_on = due;
        if let Some(topic) = data.topic.as_mut() {
            topic.due_on = due;
        }
        let owner = self.owner;
        self.ws.update(cx, |ws, cx| ws.set_due(owner, due, cx));
        cx.notify();
    }

    /// Whether "More" (a project) or "Needs attention" (an epic) is open,
    /// as remembered (layout.md 4.7, record ids only).
    fn overview_open(&self, cx: &App) -> bool {
        let key = self.owner.to_string();
        settings::current(cx).overview_open.contains(&key)
    }

    /// "More" / "Less", or the epic's "Needs attention" folded or not.
    pub fn set_overview_open(&mut self, open: bool, cx: &mut Context<Self>) {
        let key = self.owner.to_string();
        settings::change(cx, |p| {
            p.overview_open.retain(|k| *k != key);
            if open {
                p.overview_open.push(key);
            }
        });
        cx.notify();
    }

    /// Whether the page shows an Overview whose "More" (or expansion) the
    /// palette can toggle, and whether it is open.
    pub fn overview_state(&self, cx: &App) -> Option<bool> {
        match self.owner {
            Owner::Project(_) | Owner::Epic(_) => Some(self.overview_open(cx)),
            Owner::Topic(_) => None,
        }
    }

    /// Open the list behind a figure of the Overview (components-records.md
    /// 5, Click-through): the table with its chip or its Overdue segment,
    /// scrolled to and with focus on its first row; the inbox; the
    /// knowledge page's stale memories.
    pub fn show_filter(&mut self, filter: OverviewFilter, cx: &mut Context<Self>) {
        let Some(project) = self.project_id() else {
            return;
        };
        match filter {
            OverviewFilter::Inbox => cx.emit(PageEvent::Open(Route::Inbox(project))),
            OverviewFilter::StaleMemories => cx.emit(PageEvent::KbChip(
                project,
                crate::kb_model::Chip::ToCheck,
                Some(KbKind::Memory),
            )),
            OverviewFilter::Overdue => {
                self.chip = None;
                self.table_filter = table::Filter::Overdue;
                self.table_sort = None;
                self.reveal_table_now(cx);
            }
            chip => {
                self.chip = Some(chip);
                self.table_sort = None;
                self.reveal_table_now(cx);
            }
        }
    }

    /// The Overview's late answers: the Questions page of this project or
    /// epic with Overdue selected.
    pub fn show_late_answers(&mut self, cx: &mut Context<Self>) {
        cx.emit(PageEvent::LateAnswers(self.owner));
    }

    /// Scroll to the table and focus its first row once the page reloaded
    /// (the human asked, so 6.5 rule 2 is not broken).
    fn reveal_table_now(&mut self, cx: &mut Context<Self>) {
        self.reveal_table.set(true);
        self.focus_first_row = true;
        self.reload(cx);
        cx.notify();
    }

    /// The chip's `x`, `Backspace` on it or `Esc` in the list: back to the
    /// segmented control at Open.
    fn clear_chip(&mut self, cx: &mut Context<Self>) {
        if self.chip.take().is_some() {
            self.table_filter = table::Filter::Open;
            self.reload(cx);
            cx.notify();
        }
    }

    pub fn edit_title(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(data) = &self.data else { return };
        let title = data.title.clone();
        let state = cx.new(|cx| InputState::new(window, cx).default_value(title));
        state.update(cx, |state, cx| state.focus(window, cx));
        let sub =
            cx.subscribe_in(
                &state,
                window,
                |page, _, event: &InputEvent, window, cx| match event {
                    InputEvent::PressEnter { .. } => page.commit_title(window, cx),
                    InputEvent::Blur => page.commit_title(window, cx),
                    _ => {}
                },
            );
        self._subs.push(sub);
        self.title_edit = Some(state);
        cx.notify();
    }

    /// Save the title. The editor stays, with the typed text, until the
    /// write succeeds; a failure leaves it open (interaction.md 6.4).
    fn commit_title(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving_title {
            return;
        }
        let Some(state) = self.title_edit.clone() else {
            return;
        };
        let new = state.read(cx).value().trim().to_owned();
        let Some(data) = self.data.as_ref() else {
            return;
        };
        if new.is_empty() || new == data.title {
            self.title_edit = None;
            window.focus(&self.focus, cx);
            cx.notify();
            return;
        }
        self.saving_title = true;
        let owner = self.owner;
        let task = self.ws.update(cx, |ws, cx| ws.rename(owner, new, cx));
        cx.spawn_in(window, async move |page, cx| {
            let saved = task.await.is_ok();
            let _ = page.update_in(cx, |page, window, cx| {
                page.saving_title = false;
                if saved {
                    page.title_edit = None;
                    window.focus(&page.focus, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn cancel_title(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.title_edit = None;
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn open_editor(&mut self, description: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(data) = &self.data else { return };
        let locale = settings::locale(cx);
        let (value, placeholder) = if description {
            (data.description.clone(), Key::DescriptionPlaceholder)
        } else {
            (
                data.topic
                    .as_ref()
                    .map(|t| t.state.clone())
                    .unwrap_or_default(),
                Key::StatePlaceholder,
            )
        };
        let state = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(3, 16)
                .placeholder(text(locale, placeholder))
                .default_value(value)
        });
        state.update(cx, |state, cx| state.focus(window, cx));
        crate::voice::attach(&state, window, cx);
        let sub = cx.subscribe_in(
            &state,
            window,
            move |page, _, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter {
                    secondary: true, ..
                } = event
                {
                    page.commit_editor(description, window, cx);
                }
            },
        );
        self._subs.push(sub);
        if description {
            self.description_edit = Some(state);
            // A description takes files on topics and epics (ADR 0012, 1).
            let owner = match self.owner {
                Owner::Topic(id) => Some(AttachOwner::Topic(id)),
                Owner::Epic(id) => Some(AttachOwner::Epic(id)),
                Owner::Project(_) => None,
            };
            self.description_tray = owner.map(|owner| self.new_tray(owner, cx));
        } else {
            self.state_edit = Some(state);
        }
        cx.notify();
    }

    fn new_tray(
        &mut self,
        owner: AttachOwner,
        cx: &mut Context<Self>,
    ) -> Entity<crate::attach_ui::Tray> {
        let ws = self.ws.clone();
        let held = self
            .data
            .as_ref()
            .and_then(|d| d.files.get(&owner).cloned())
            .unwrap_or_default();
        let tray = cx.new(|cx| crate::attach_ui::Tray::new(ws, Some(owner), held, cx));
        self._subs.push(cx.observe(&tray, |_, _, cx| cx.notify()));
        tray
    }

    /// An editor was saved: the files it added get their rows on `owner`.
    fn commit_tray(
        &self,
        tray: Option<Entity<crate::attach_ui::Tray>>,
        owner: AttachOwner,
        cx: &mut Context<Self>,
    ) {
        if let (Some(tray), Some(project)) = (tray, self.project_id()) {
            tray.update(cx, |tray, cx| tray.commit(owner, project, cx));
        }
    }

    /// Save the description or current state. The editor stays open with
    /// the typed text until the write succeeds; a failure keeps it.
    fn commit_editor(&mut self, description: bool, window: &mut Window, cx: &mut Context<Self>) {
        let editor = if description {
            self.description_edit.clone()
        } else {
            self.state_edit.clone()
        };
        let Some(editor) = editor else { return };
        let value = editor.read(cx).value().trim_end().to_owned();
        let owner = self.owner;
        if description {
            let files_owner = match owner {
                Owner::Topic(id) => Some(AttachOwner::Topic(id)),
                Owner::Epic(id) => Some(AttachOwner::Epic(id)),
                Owner::Project(_) => None,
            };
            if let Some(files_owner) = files_owner {
                let tray = self.description_tray.clone();
                self.commit_tray(tray, files_owner, cx);
            }
        }
        let task = self.ws.update(cx, |ws, cx| {
            ws.write(cx, Key::ErrorSave, move |store| {
                let me = Author::Human;
                match (owner, description) {
                    (Owner::Topic(id), true) => store.update_topic(
                        id,
                        TopicPatch {
                            description: Some(value),
                            ..TopicPatch::default()
                        },
                        me,
                    ),
                    (Owner::Topic(id), false) => store.update_topic(
                        id,
                        TopicPatch {
                            state: Some(value),
                            ..TopicPatch::default()
                        },
                        me,
                    ),
                    (Owner::Epic(id), _) => store.update_epic(
                        id,
                        EpicPatch {
                            description: Some(value),
                            ..EpicPatch::default()
                        },
                        me,
                    ),
                    (Owner::Project(id), _) => store.update_project(
                        id,
                        ProjectPatch {
                            description: Some(value),
                            ..ProjectPatch::default()
                        },
                        me,
                    ),
                }
            })
        });
        cx.spawn_in(window, async move |page, cx| {
            if task.await.is_ok() {
                let _ = page.update_in(cx, |page, window, cx| {
                    let open = if description {
                        &mut page.description_edit
                    } else {
                        &mut page.state_edit
                    };
                    // Only the editor that was saved closes.
                    if open.as_ref() == Some(&editor) {
                        *open = None;
                        if description {
                            page.description_tray = None;
                        }
                        window.focus(&page.focus, cx);
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// `Esc` in an editor: close it; its text is dropped from the editor
    /// but the stored value was never changed (drafts are request 15).
    fn cancel_editors(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // The files the editors added go with them (the record's stay).
        for tray in [self.description_tray.take(), self.entry_tray.take()]
            .into_iter()
            .flatten()
        {
            tray.update(cx, |tray, cx| tray.discard(cx));
        }
        let had = self.state_edit.take().is_some()
            | self.description_edit.take().is_some()
            | self.status_menu.take().is_some()
            | self.entry_menu.take().is_some()
            | self.entry_edit.take().is_some()
            | self.kb_new_menu.take().is_some();
        if had {
            window.focus(&self.focus, cx);
        }
        cx.notify();
    }

    /// Debug builds: the description editor, or the editor of the newest
    /// entry with files, for captures.
    #[cfg(debug_assertions)]
    pub fn dev_files_editor(&mut self, entry: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !entry {
            self.open_editor(true, window, cx);
            return;
        }
        let found = self.data.as_ref().and_then(|d| {
            d.entries
                .iter()
                .find(|e| d.files.contains_key(&AttachOwner::Entry(e.id)))
                .cloned()
        });
        if let Some(e) = found {
            self.start_entry_edit(e, window, cx);
        }
    }

    /// Debug builds: a menu open for captures (`status-menu`, or the
    /// `entry-menu` of the newest entry that is not a system event).
    #[cfg(debug_assertions)]
    pub fn dev_menu(&mut self, which: &str, window: &mut Window, cx: &mut Context<Self>) {
        match which {
            "status-menu" => self.status_menu = Some(0),
            "entry-menu" => {
                let entry = self.data.as_ref().and_then(|d| {
                    record::timeline_entries(&d.entries)
                        .into_iter()
                        .find(|e| e.kind != EntryKind::System)
                });
                if let Some(entry) = entry {
                    self.open_entry_menu(entry, window, cx);
                }
            }
            _ => {}
        }
        cx.notify();
    }

    // ---- the entry menu ----

    fn open_entry_menu(&mut self, entry: Entry, window: &mut Window, cx: &mut Context<Self>) {
        self.entry_menu = Some(EntryMenu {
            entry,
            kinds: false,
            highlight: 0,
        });
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Choose row `index` of the open entry menu.
    fn choose_entry_item(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(menu) = self.entry_menu.take() else {
            return;
        };
        let entry = menu.entry;
        if menu.kinds {
            if let Some(kind) = ENTRY_KINDS.get(index) {
                self.write_entry(&entry, entry.text.clone(), *kind, cx);
            }
            cx.notify();
            return;
        }
        match index {
            0 => self.start_entry_edit(entry, window, cx),
            1 => cx.write_to_clipboard(ClipboardItem::new_string(entry.text.clone())),
            2 => {
                let at = ENTRY_KINDS
                    .iter()
                    .position(|k| *k == entry.kind)
                    .unwrap_or(0);
                self.entry_menu = Some(EntryMenu {
                    entry,
                    kinds: true,
                    highlight: at,
                });
            }
            _ => self.delete_entry(&entry, cx),
        }
        cx.notify();
    }

    /// Edit: the body becomes the multi-line editor in place.
    fn start_entry_edit(&mut self, entry: Entry, window: &mut Window, cx: &mut Context<Self>) {
        let id = entry.id;
        let state = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(3, 16)
                .default_value(entry.text.clone())
        });
        state.update(cx, |state, cx| state.focus(window, cx));
        crate::voice::attach(&state, window, cx);
        let sub = cx.subscribe_in(
            &state,
            window,
            move |page, editor, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter {
                    secondary: true, ..
                } = event
                {
                    let text = editor.read(cx).value().trim_end().to_owned();
                    if !text.trim().is_empty() {
                        page.write_entry(&entry, text, entry.kind, cx);
                        let tray = page.entry_tray.take();
                        page.commit_tray(tray, AttachOwner::Entry(entry.id), cx);
                        page.entry_edit = None;
                        window.focus(&page.focus, cx);
                        cx.notify();
                    }
                }
            },
        );
        self._subs.push(sub);
        self.entry_edit = Some((id, state));
        self.entry_tray = Some(self.new_tray(AttachOwner::Entry(id), cx));
        self.reveal_editor.set(true);
        cx.notify();
    }

    /// Save an entry's text and type, undoably.
    fn write_entry(
        &mut self,
        entry: &Entry,
        text: String,
        kind: EntryKind,
        cx: &mut Context<Self>,
    ) {
        if text == entry.text && kind == entry.kind {
            return;
        }
        let id = entry.id;
        let back = (entry.text.clone(), entry.kind);
        let project = self.project_id();
        self.ws.update(cx, |ws, cx| {
            ws.undoable(cx, project, None, move |store| {
                store.update_entry(id, &text, kind, Author::Human)?;
                let now = store.entry(id)?;
                Ok((
                    Undo::Entry {
                        id,
                        back,
                        expect: (now.text, now.kind),
                    },
                    (),
                ))
            });
        });
    }

    fn delete_entry(&mut self, entry: &Entry, cx: &mut Context<Self>) {
        let record = Record::Entry(entry.id);
        let project = self.project_id();
        self.ws.update(cx, |ws, cx| ws.delete(record, project, cx));
    }

    fn restore_record(&mut self, cx: &mut Context<Self>) {
        let record = record_of(self.owner);
        let project = self.project_id();
        self.ws
            .update(cx, |ws, cx| ws.restore(record, project, None, cx).detach());
    }

    /// New document or memory owned by this page's record: the editor
    /// opens empty and the entry exists once saved.
    pub fn new_kb(&mut self, kind: KbKind, cx: &mut Context<Self>) {
        cx.emit(PageEvent::NewKb(crate::kb_page::Draft {
            owner: self.owner,
            kind,
        }));
    }

    // ---- keys ----

    // ---- blocks (interaction.md 6.1) ----

    /// "Logged as a decision": the entry is focused and brought into view,
    /// now or once the timeline is loaded. An entry the timeline does not
    /// show (another filter) shows it with every type first.
    pub fn focus_entry(&mut self, entry: EntryId, window: &mut Window, cx: &mut Context<Self>) {
        let block = record::Block::Entry(entry);
        let loaded = self
            .data
            .as_ref()
            .is_some_and(|d| d.entries.iter().any(|e| e.id == entry));
        if loaded && self.block_order.contains(&block) {
            self.focus_block(block, window, cx);
            return;
        }
        if self.data.is_some() && self.filter != EntryFilterKind::All {
            self.filter = EntryFilterKind::All;
            self.reload(cx);
        }
        self.pending_entry = Some(entry);
        cx.notify();
    }

    /// The handle of a block, made the first time it is drawn.
    fn block_handle(&mut self, block: record::Block, cx: &mut Context<Self>) -> FocusHandle {
        self.block_focus
            .entry(block)
            .or_insert_with(|| cx.focus_handle())
            .clone()
    }

    /// The block with the keyboard.
    fn focused_block(&self, window: &Window) -> Option<record::Block> {
        self.block_order.iter().copied().find(|b| {
            self.block_focus
                .get(b)
                .is_some_and(|h| h.is_focused(window))
        })
    }

    /// Focus a block and scroll it into view by the least amount.
    pub(super) fn focus_block(
        &mut self,
        block: record::Block,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let handle = self.block_handle(block, cx);
        window.focus(&handle, cx);
        self.reveal_block.set(Some(block));
        cx.notify();
    }

    /// A block's element: its focus handle, the ring for the keyboard, and
    /// the measure that brings it into view once stepped to.
    /// The handles are made before drawing, in `render`.
    pub(super) fn block_el(
        &self,
        block: record::Block,
        el: gpui_kit::Stateful<gpui_kit::Div>,
        t: &Tokens,
    ) -> gpui_kit::Stateful<gpui_kit::Div> {
        let handle = self.block_focus.get(&block).cloned();
        let reveal = std::rc::Rc::clone(&self.reveal_block);
        let scroll = self.scroll.clone();
        ui::focusable(el, t, false)
            .when_some(handle, |this, h| this.track_focus(&h))
            .relative()
            .child(ui::measure(move |bounds, window| {
                if reveal.get() == Some(block) {
                    reveal.set(None);
                    ui::scroll_into_view(&scroll, bounds, ui::Reveal::Whole);
                    window.refresh();
                }
            }))
    }

    /// `Up` / `Down` between blocks, `Enter` into the focused one. Whether
    /// the key was taken.
    fn block_keys(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let k = &event.keystroke;
        if k.modifiers.modified() {
            return false;
        }
        let at = self.focused_block(window);
        let on_page = self.focus.is_focused(window);
        match k.key.as_str() {
            "up" | "down" if at.is_some() || on_page => {
                if let Some(to) = record::step_block(&self.block_order, at, k.key == "up") {
                    self.focus_block(to, window, cx);
                }
            }
            "enter" => match at {
                Some(record::Block::Title) => self.edit_title(window, cx),
                Some(record::Block::State) => self.open_editor(false, window, cx),
                Some(record::Block::Description) => self.open_editor(true, window, cx),
                Some(record::Block::Questions) => self.enter_questions(window, cx),
                Some(record::Block::Folders) => self.enter_folders(window, cx),
                Some(record::Block::Entry(id)) => {
                    let entry = self
                        .data
                        .as_ref()
                        .and_then(|d| d.entries.iter().find(|e| e.id == id).cloned());
                    match entry {
                        Some(entry) => self.start_entry_edit(entry, window, cx),
                        None => return false,
                    }
                }
                None => return false,
            },
            _ => return false,
        }
        cx.stop_propagation();
        cx.notify();
        true
    }

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(menu) = self.entry_menu.as_mut() {
            if event.keystroke.modifiers.modified() {
                return;
            }
            let rows = if menu.kinds { ENTRY_KINDS.len() } else { 4 };
            match event.keystroke.key.as_str() {
                "up" => menu.highlight = menu.highlight.saturating_sub(1),
                "down" => menu.highlight = (menu.highlight + 1).min(rows - 1),
                "enter" | "space" => {
                    let at = menu.highlight;
                    self.choose_entry_item(at, window, cx);
                }
                "escape" => self.entry_menu = None,
                _ => return,
            }
            cx.stop_propagation();
            cx.notify();
            return;
        }
        if self.folder_keys(event, window, cx) {
            return;
        }
        if event.keystroke.key == "escape" && self.cancel_q_drag(cx) {
            cx.stop_propagation();
            return;
        }
        if self.status_menu.is_none()
            && self.focused_block(window) == Some(record::Block::Folders)
            && self.folder_row_keys(event, window, cx)
        {
            return;
        }
        if self.status_menu.is_none() && self.block_keys(event, window, cx) {
            return;
        }
        let Some(highlight) = self.status_menu else {
            if event.keystroke.key == "escape"
                && self.chip.is_some()
                && self.table_focus.contains_focused(window, cx)
            {
                self.clear_chip(cx);
                cx.stop_propagation();
            }
            return;
        };
        let key = event.keystroke.key.as_str();
        if event.keystroke.modifiers.modified() {
            return;
        }
        match key {
            "up" => self.status_menu = Some(highlight.saturating_sub(1)),
            "down" => self.status_menu = Some((highlight + 1).min(5)),
            "enter" | "space" => self.set_status(Status::ALL[highlight], cx),
            "escape" => self.status_menu = None,
            digit if digit.len() == 1 => {
                if let Some(n) = digit.chars().next().and_then(|c| c.to_digit(10))
                    && (1..=6).contains(&n)
                {
                    self.set_status(Status::ALL[n as usize - 1], cx);
                } else {
                    return;
                }
            }
            _ => return,
        }
        let _ = window;
        cx.stop_propagation();
        cx.notify();
    }
}

// ---- rendering ----

/// The meta line's text around the id (components-records.md 5): the
/// sentence of `meta.record` cut where the id goes, so the id can be its
/// own control in any language; without an id, all of `meta.updated`.
pub(crate) fn meta_parts(locale: Locale, with_id: bool, at: i64, by: Author) -> (String, String) {
    const MARK: &str = "\u{1}";
    let when = when::long(locale, at);
    let author = when::author(locale, by);
    let args = [
        ("id", Arg::Text(MARK)),
        ("when", Arg::Text(&when)),
        ("author", Arg::Text(&author)),
    ];
    if !with_id {
        return (String::new(), tr(locale, Key::MetaUpdated, &args[1..]));
    }
    let line = tr(locale, Key::MetaRecord, &args);
    match line.split_once(MARK) {
        Some((before, after)) => (before.to_owned(), after.to_owned()),
        None => (String::new(), line),
    }
}

fn entry_kind_meta(kind: EntryKind) -> (Icon, Key) {
    match kind {
        EntryKind::Progress | EntryKind::System => (Icon::GitCommitHorizontal, Key::EntryProgress),
        EntryKind::Decision => (Icon::Signpost, Key::EntryDecision),
        EntryKind::Discussion => (Icon::MessageSquare, Key::EntryDiscussion),
    }
}

impl RecordPage {
    fn render_title(&self, data: &Loaded, t: &Tokens, cx: &mut Context<Self>) -> AnyElement {
        if let Some(state) = &self.title_edit {
            // The editor keeps the title's own size (QA: it shrank to the
            // input's default while editing).
            return type_scale::TITLE
                .el()
                .border_b(px(1.))
                .border_color(t.border_strong)
                .on_action(
                    cx.listener(|page, _: &Escape, window, cx| page.cancel_title(window, cx)),
                )
                .child(ui::role_input(Input::new(state), type_scale::TITLE))
                .into_any_element();
        }
        let title = type_scale::TITLE
            .el()
            .id("page-title")
            .text_color(t.text_primary)
            .cursor_text()
            .rounded(px(RADIUS_SM))
            .on_click(cx.listener(|page, _, window, cx| page.edit_title(window, cx)))
            .child(data.title.clone());
        self.block_el(record::Block::Title, title, t)
            .into_any_element()
    }

    fn render_properties(
        &self,
        data: &Loaded,
        due_focused: bool,
        window: &Window,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut rows: Vec<(String, AnyElement)> = Vec::new();
        // An unset optional value has no row (components-records.md 5).
        let due_row = data.due_on.is_some();
        match self.owner {
            Owner::Topic(_) => {
                let topic = data.topic.as_ref();
                let status = topic.map(|t| t.status).unwrap_or(Status::Planned);
                let status_button = ui::ghost_value("prop-status", t)
                    .relative()
                    .child(ui::status_glyph(status, 16., t))
                    .child(text(locale, ui::status_key(status)))
                    .child(self.status_anchor.measure())
                    .on_click(cx.listener(move |page, _, window, cx| {
                        page.status_menu =
                            Some(Status::ALL.iter().position(|s| *s == status).unwrap_or(0));
                        window.focus(&page.focus, cx);
                        cx.notify();
                    }));
                let menu = self
                    .status_menu
                    .map(|highlight| self.render_status_menu(status, highlight, t, cx));
                rows.push((
                    text(locale, Key::PropStatus),
                    div().child(status_button).children(menu).into_any_element(),
                ));
                if due_row {
                    rows.push((
                        text(locale, Key::PropDue),
                        self.render_due(data, locale, t, cx),
                    ));
                }
                if let Some(report) = self.render_report(data, locale, t, cx) {
                    rows.push((text(locale, Key::PropReportTo), report));
                }
                let epic = match &data.epic {
                    Some((epic, name)) => {
                        let epic = *epic;
                        link(("prop-epic", 0usize), name.clone(), t)
                            .on_click(cx.listener(move |_, _, _, cx| {
                                cx.emit(PageEvent::Open(Route::Epic(epic)))
                            }))
                            .into_any_element()
                    }
                    None => div()
                        .text_color(t.text_tertiary)
                        .child(text(locale, Key::CommonNone))
                        .into_any_element(),
                };
                rows.push((text(locale, Key::PropEpic), epic));
            }
            Owner::Epic(_) | Owner::Project(_) => {
                rows.push((text(locale, Key::PropProgress), progress(data, locale, t)));
                if due_row {
                    rows.push((
                        text(locale, Key::PropDue),
                        self.render_due(data, locale, t, cx),
                    ));
                }
                if matches!(self.owner, Owner::Epic(_))
                    && let Some(report) = self.render_report(data, locale, t, cx)
                {
                    rows.push((text(locale, Key::PropReportTo), report));
                }
            }
        }
        rows.push((
            text(locale, Key::PropFolders),
            self.render_folders(data, locale, t, cx),
        ));
        let labels: Vec<&str> = rows.iter().map(|(l, _)| l.as_str()).collect();
        let width = ui::label_width(&labels, cx);
        // The add chips (components-records.md 5): only while the pointer
        // is over the block (its band included) or the picker they open is
        // open, drawn in the 24 band under the meta line, which is empty at
        // rest: they never move the page (components.md 5, revealed
        // controls).
        let report_chip = !matches!(self.owner, Owner::Project(_)) && data.report.to.is_none();
        let wanted = self.props_hover
            || self.due_picker.is_some()
            || due_focused
            || self.report_chip_wanted(window);
        let chips = ((!due_row || report_chip) && wanted)
            .then(|| self.render_add_chips(!due_row, report_chip, locale, t, cx));
        div()
            .id("properties")
            .flex()
            .flex_col()
            .mt(px(12.))
            .on_hover(cx.listener(|page, hovered: &bool, _, cx| {
                if page.props_hover != *hovered {
                    page.props_hover = *hovered;
                    cx.notify();
                }
            }))
            .children(
                rows.into_iter()
                    .map(|(label, value)| ui::property(label, width, value, t)),
            )
            .child(self.render_meta(data, locale, t, cx))
            .child(add_band(chips, width))
            .into_any_element()
    }

    /// The add chip of an unset Due (components-records.md 5): a compact
    /// ghost button, `plus` 14 and "Due date" in `type.small` tertiary; the
    /// picker hangs from it.
    fn render_add_chips(
        &self,
        due: bool,
        report: bool,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let hover = t.fill_hover;
        let chip = ui::focusable(type_scale::SMALL.el().id("prop-add-due"), t, false)
            .relative()
            .track_focus(&self.due_focus)
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.))
            .h(px(24.))
            .px(px(8.))
            .ml(px(-8.))
            .rounded(px(RADIUS_SM))
            .text_color(t.text_tertiary)
            .hover(move |s| s.bg(hover))
            .on_click(cx.listener(|page, _, window, cx| page.open_due_picker(false, window, cx)))
            .child(icon(Icon::Plus, 14., t.text_tertiary))
            .child(text(locale, Key::PropAddDue))
            .child(self.due_anchor.measure());
        let picker = self.due_picker.as_ref().and_then(|p| {
            self.due_anchor
                .below_at(p.clone(), crate::menu::Align::Start, 2)
        });
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .h(px(24.))
            .when(due, |this| this.child(chip).children(picker))
            .when(report, |this| {
                this.child(self.render_report_chip(locale, t, cx))
            })
            .into_any_element()
    }

    /// The meta line under the block (components-records.md 5): "T-42 ·
    /// Updated 2 h ago by Claude Code", `type.small` tertiary; the id in
    /// mono at 12 is a ghost control that copies it; the time has the full
    /// date as its tooltip. A project shows no id.
    fn render_meta(
        &self,
        data: &Loaded,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = match self.owner {
            Owner::Topic(id) => Some(id.to_string()),
            Owner::Epic(id) => Some(id.to_string()),
            Owner::Project(_) => None,
        };
        let (before, after) = meta_parts(locale, id.is_some(), data.updated_at, data.updated_by);
        let full = when::datetime(locale, data.updated_at);
        let mut line = type_scale::SMALL
            .el()
            .flex()
            .flex_row()
            .items_center()
            .min_h(px(20.))
            .mt(px(4.))
            .text_color(t.text_tertiary)
            .font_features(type_scale::tabular());
        if !before.is_empty() {
            line = line.child(before);
        }
        if let Some(id) = id {
            let shown = if self.id_copied {
                text(locale, Key::CommonCopied)
            } else {
                id.clone()
            };
            let hover = t.fill_hover;
            let role = if self.id_copied {
                type_scale::SMALL
            } else {
                type_scale::CODE_SMALL
            };
            line = line.child(
                ui::focusable(role.el().id("meta-id"), t, false)
                    .px(px(4.))
                    .mx(px(-4.))
                    .rounded(px(RADIUS_XS))
                    .text_color(t.text_tertiary)
                    .hover(move |s| s.bg(hover))
                    .tooltip(ui::tip(text(locale, Key::MetaCopyId)))
                    .on_click(cx.listener(move |page, _, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(id.clone()));
                        page.id_copied = true;
                        cx.notify();
                        cx.spawn(async move |page, cx| {
                            cx.background_executor()
                                .timer(std::time::Duration::from_millis(1500))
                                .await;
                            let _ = page.update(cx, |page, cx| {
                                page.id_copied = false;
                                cx.notify();
                            });
                        })
                        .detach();
                    }))
                    .child(shown),
            );
        }
        line.child(
            div()
                .id("meta-updated")
                .min_w_0()
                .truncate()
                .tooltip(ui::tip(full))
                .child(after),
        )
        .into_any_element()
    }

    /// The Due row (components-records.md 5), drawn only while a date is
    /// set: the date and its relative phrase in two runs; the picker hangs
    /// under it.
    fn render_due(
        &self,
        data: &Loaded,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let today = due::today(cx);
        let open = data.due_open();
        let state = due_state(data.due_on, today, open);
        let value = match data.due_on {
            None => div()
                .text_color(t.text_tertiary)
                .child(text(locale, Key::DueNone))
                .into_any_element(),
            Some(day) => {
                let phrase = due::phrase(locale, state)
                    .map(|(words, tone)| div().text_color(tone_color(tone, t)).child(words));
                div()
                    .flex()
                    .flex_row()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_color(if state == DueState::Closed {
                                t.text_secondary
                            } else {
                                t.text_primary
                            })
                            .child(due::date(locale, day)),
                    )
                    .children(phrase)
                    .into_any_element()
            }
        };
        let reveal = std::rc::Rc::clone(&self.reveal_due);
        let scroll = self.scroll.clone();
        let button = ui::ghost_value("prop-due", t)
            .relative()
            .track_focus(&self.due_focus)
            .on_click(cx.listener(|page, _, window, cx| page.open_due_picker(false, window, cx)))
            .on_key_down(cx.listener(|page, event: &KeyDownEvent, _, cx| {
                if matches!(event.keystroke.key.as_str(), "delete" | "backspace")
                    && !event.keystroke.modifiers.modified()
                {
                    page.set_due(None, cx);
                    cx.stop_propagation();
                }
            }))
            .child(value)
            .child(self.due_anchor.measure())
            .child(ui::measure(move |bounds, window| {
                if reveal.replace(false) {
                    ui::scroll_into_view(&scroll, bounds, ui::Reveal::Whole);
                    window.refresh();
                }
            }));
        let picker = self.due_picker.as_ref().and_then(|p| {
            self.due_anchor
                .below_at(p.clone(), crate::menu::Align::Start, 2)
        });
        div().child(button).children(picker).into_any_element()
    }

    fn render_status_menu(
        &self,
        current: Status,
        highlight: usize,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let locale = settings::locale(cx);
        let items = Status::ALL.into_iter().enumerate().map(|(i, status)| {
            let trailing = if status == current {
                menu::check(t)
            } else {
                menu::hint((i + 1).to_string(), t)
            };
            menu::item(
                ("status-item", i),
                Some(ui::status_glyph(status, 16., t).into_any_element()),
                text(locale, ui::status_key(status)),
                Some(trailing),
                i == highlight,
                t,
            )
            .on_click(cx.listener(move |page, _, _, cx| page.set_status(status, cx)))
        });
        self.status_anchor
            .below(
                menu::panel("status-menu", t)
                    .on_mouse_down_out(cx.listener(|page, _, _, cx| {
                        page.status_menu = None;
                        cx.notify();
                    }))
                    .children(items),
                menu::Align::Start,
            )
            .unwrap_or_else(|| div().into_any_element())
    }

    fn render_state(
        &self,
        data: &Loaded,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let topic = data.topic.as_ref();
        let meta = topic
            .and_then(|t| t.state_updated_at.zip(t.state_updated_by))
            .map(|(at, by)| {
                let when = when::long(locale, at);
                let author = when::author(locale, by);
                tr(
                    locale,
                    Key::StateUpdated,
                    &[("when", Arg::Text(&when)), ("author", Arg::Text(&author))],
                )
            });
        let body: AnyElement = if let Some(editor) = &self.state_edit {
            editor_box(editor, locale, t, cx)
        } else {
            let state = topic.map(|t| t.state.clone()).unwrap_or_default();
            if state.trim().is_empty() {
                type_scale::UI
                    .el()
                    .text_color(t.text_tertiary)
                    .child(text(locale, Key::StatePlaceholder))
                    .into_any_element()
            } else {
                markdown_view::short("state-body", state, t).into_any_element()
            }
        };
        let el = div()
            .id("current-state")
            .flex()
            .flex_col()
            .gap(px(8.))
            .mt(px(24.))
            .p(px(16.))
            .rounded(px(RADIUS_LG))
            .bg(t.bg_raised)
            .border(px(1.))
            .border_color(t.border_subtle)
            .when(self.state_edit.is_none(), |this| {
                this.cursor_text().on_click(
                    cx.listener(|page, _, window, cx| page.open_editor(false, window, cx)),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .child(ui::section_header(text(locale, Key::SectionState), t))
                    .children(
                        meta.map(|m| type_scale::SMALL.el().text_color(t.text_tertiary).child(m)),
                    ),
            )
            .child(body);
        if self.state_edit.is_some() {
            return el.into_any_element();
        }
        self.block_el(record::Block::State, el, t)
            .into_any_element()
    }

    fn render_description(
        &self,
        data: &Loaded,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let files_owner = match self.owner {
            Owner::Topic(id) => Some(AttachOwner::Topic(id)),
            Owner::Epic(id) => Some(AttachOwner::Epic(id)),
            Owner::Project(_) => None,
        };
        let strip = match (&self.description_edit, files_owner) {
            (None, Some(owner)) => data.files.get(&owner).and_then(|files| {
                crate::attach_ui::strip(
                    "description",
                    files,
                    crate::attach_ui::StripStyle::Full,
                    t,
                    cx,
                )
            }),
            _ => None,
        };
        let tray = self.description_tray.clone();
        let body: AnyElement = if let Some(editor) = &self.description_edit {
            editor_box_with(editor, tray.as_ref(), locale, t, cx)
        } else if data.description.trim().is_empty() {
            let placeholder = match self.owner {
                Owner::Topic(_) => Key::DescriptionPlaceholder,
                Owner::Epic(_) => Key::DescriptionPlaceholderEpic,
                Owner::Project(_) => Key::DescriptionPlaceholderProject,
            };
            type_scale::UI
                .el()
                .text_color(t.text_tertiary)
                .child(text(locale, placeholder))
                .into_any_element()
        } else {
            markdown_view::short("description-body", data.description.clone(), t).into_any_element()
        };
        let el = div()
            .id("description")
            .flex()
            .flex_col()
            .gap(px(4.))
            .mt(px(32.))
            .when(self.description_edit.is_none(), |this| {
                this.cursor_text()
                    .on_click(cx.listener(|page, _, window, cx| page.open_editor(true, window, cx)))
            })
            .child(
                div()
                    .h(px(28.))
                    .flex()
                    .items_center()
                    .child(ui::section_header(text(locale, Key::SectionDescription), t)),
            )
            .child(body)
            .children(strip.map(|s| div().mt(px(4.)).child(s)));
        if self.description_edit.is_some() {
            return el.into_any_element();
        }
        self.block_el(record::Block::Description, el, t)
            .into_any_element()
    }

    /// Git's flags of the section's entries, read off the UI thread.
    fn read_kb_git(&mut self, ids: Vec<KbId>, cx: &mut Context<Self>) {
        let task = crate::kbq_ui::list_git(&self.ws, ids, cx);
        self._kb_git = cx.spawn(async move |page, cx| {
            let flags = task.await;
            let _ = page.update(cx, |page, cx| {
                page.kb_flags.set_git(flags);
                cx.notify();
            });
        });
    }

    /// The unfolded memory's notice and body.
    fn load_kb_open(&mut self, id: KbId, cx: &mut Context<Self>) {
        let task = crate::kbq_ui::load_entry(&self.ws, id, cx);
        self._kb_open = cx.spawn(async move |page, cx| {
            if let Ok(q) = task.await {
                let _ = page.update(cx, |page, cx| {
                    if page.kb_open == Some(q.entry.id) {
                        page.kb_open_q = Some(q);
                    }
                    cx.notify();
                });
            }
        });
    }

    /// The focused knowledge row's entry, for the palette.
    pub fn kb_context(&self, window: &Window) -> Option<crate::commands::FocusedKb> {
        let i = self
            .kb_row_focus
            .borrow()
            .iter()
            .position(|h| h.is_focused(window))?;
        let (id, rev, owner, kind, state) = *self.kb_row_ids.borrow().get(i)?;
        let reasons = self.kb_flags.reasons(id);
        Some(crate::commands::FocusedKb {
            id,
            rev,
            owner,
            memory: kind == KbKind::Memory,
            live: state == acta_core::model::KbState::Live,
            conflict: reasons.iter().find_map(|r| match r {
                crate::kbq_model::Reason::Conflict(x) => Some(*x),
                _ => None,
            }),
            duplicate: reasons.iter().find_map(|r| match r {
                crate::kbq_model::Reason::Duplicate(x) => Some(*x),
                _ => None,
            }),
        })
    }

    /// "Mark as still true" on a row: its mark goes at once.
    pub fn verify_kb(&mut self, id: KbId, cx: &mut Context<Self>) {
        let owner = self
            .kb_row_ids
            .borrow()
            .iter()
            .find(|r| r.0 == id)
            .map(|r| r.2);
        self.kb_flags.mark_verified(id);
        crate::kbq_ui::verify(&self.ws, id, owner, cx);
        cx.notify();
    }

    fn render_kb_row(
        &self,
        entry: &KbSummary,
        index: usize,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = entry.id;
        let kind = entry.kind;
        let open = self.kb_open == Some(id);
        let unread = self.ws.read(cx).is_unread(acta_core::model::Record::Kb(id));
        let reasons = self.kb_flags.reasons(id);
        let signal = crate::kb_page::Signal::of(unread, &reasons, locale);
        let handle = {
            let mut focus = self.kb_row_focus.borrow_mut();
            while focus.len() <= index {
                focus.push(cx.focus_handle());
            }
            focus[index].clone()
        };
        {
            let mut ids = self.kb_row_ids.borrow_mut();
            ids.truncate(index);
            ids.push((id, entry.rev, entry.owner, kind, entry.state));
        }
        let row = crate::kb_page::kb_row(entry, None, signal, false, index, locale, t)
            .track_focus(&handle)
            .on_key_down(
                cx.listener(move |page, event: &gpui_kit::KeyDownEvent, _, cx| {
                    // `V`: Mark as still true (components-knowledge.md 5).
                    if !event.keystroke.modifiers.modified() && event.keystroke.key == "v" {
                        page.verify_kb(id, cx);
                        cx.stop_propagation();
                    }
                }),
            )
            .on_click(cx.listener(move |page, _, _, cx| match kind {
                KbKind::Document => cx.emit(PageEvent::Open(Route::Kb(id))),
                KbKind::Memory => {
                    page.kb_open = if page.kb_open == Some(id) {
                        page.kb_open_q = None;
                        None
                    } else {
                        // Unfolded is shown: the dot goes.
                        page.ws.update(cx, |ws, cx| {
                            ws.mark_seen(acta_core::model::Record::Kb(id), cx)
                        });
                        page.load_kb_open(id, cx);
                        Some(id)
                    };
                    cx.notify();
                }
            }));
        if !open {
            return row.into_any_element();
        }
        let ws = self.ws.clone();
        let unfolded = crate::kb_page::unfolded(
            id,
            index,
            self.kb_open_q.as_ref(),
            &reasons,
            &ws,
            locale,
            t,
            cx,
        );
        div()
            .flex()
            .flex_col()
            .child(row)
            .child(unfolded)
            .into_any_element()
    }

    fn render_knowledge(
        &self,
        data: &Loaded,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let kb = &data.kb;
        let own_count = kb.own.len();
        let mut section = div().flex().flex_col().mt(px(32.)).child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                .h(px(28.))
                .child(ui::section_header(text(locale, Key::SectionKnowledge), t))
                .child(
                    type_scale::SMALL
                        .el()
                        .text_color(t.text_tertiary)
                        .font_features(type_scale::tabular())
                        .child(number(locale, own_count as u64)),
                )
                .child(div().flex_1())
                .child(
                    div()
                        .relative()
                        .child(
                            ui::ghost_icon_button("kb-new", Icon::Plus, 24., false, false, true, t)
                                .when(self.kb_new_menu.is_none(), |this| {
                                    this.tooltip(ui::tip(format!(
                                        "{} / {}",
                                        text(locale, Key::KbNewDocument),
                                        text(locale, Key::KbNewMemory)
                                    )))
                                })
                                .on_click(cx.listener(|page, _, window, cx| {
                                    page.kb_new_menu = Some(0);
                                    window.focus(&page.focus, cx);
                                    cx.notify();
                                })),
                        )
                        .child(self.kb_new_anchor.measure())
                        .children(
                            self.kb_new_menu
                                .map(|h| self.render_kb_new_menu(h, locale, t, cx)),
                        ),
                ),
        );
        if kb.own.is_empty() {
            let empty = Key::KbEmpty;
            section = section.child(
                type_scale::UI
                    .el()
                    .mt(px(4.))
                    .text_color(t.text_tertiary)
                    .child(text(locale, empty)),
            );
        }
        let mut index = 0;
        for entry in kb.own.iter().take(kb_model::SECTION_LIMIT) {
            section = section.child(self.render_kb_row(entry, index, locale, t, cx));
            index += 1;
        }
        if own_count > kb_model::SECTION_LIMIT {
            let project = data.project.0;
            section = section.child(
                ui::button(
                    "kb-show-all",
                    tr(
                        locale,
                        Key::CommonShowAllN,
                        &[("n", Arg::Count(own_count as u64))],
                    ),
                    Variant::Ghost,
                    false,
                    t,
                )
                .on_click(cx.listener(move |_, _, _, cx| {
                    cx.emit(PageEvent::Open(Route::Knowledge(project)))
                })),
            );
        }
        let inherited = kb.inherited_count();
        if inherited > 0 {
            if self.kb_inherited_open {
                for (owner, entries) in &kb.inherited {
                    let name = entries
                        .first()
                        .map(|_| owner_label(*owner, data))
                        .unwrap_or_default();
                    let key = match owner {
                        Owner::Project(_) => Key::KbFromProject,
                        _ => Key::KbFromEpic,
                    };
                    section = section.child(
                        type_scale::SMALL
                            .el()
                            .mt(px(8.))
                            .text_color(t.text_tertiary)
                            .child(tr(locale, key, &[("name", Arg::Text(&name))])),
                    );
                    for entry in entries {
                        section = section.child(self.render_kb_row(entry, index, locale, t, cx));
                        index += 1;
                    }
                }
            } else {
                // From the project alone (an epic, or a topic outside any
                // epic), the row names it.
                let only_project = kb
                    .inherited
                    .iter()
                    .all(|(owner, _)| matches!(owner, Owner::Project(_)));
                let label = if only_project {
                    tr(
                        locale,
                        Key::KbMoreInheritedProject,
                        &[
                            ("n", Arg::Count(inherited as u64)),
                            ("name", Arg::Text(&data.project.1)),
                        ],
                    )
                } else {
                    tr(
                        locale,
                        Key::KbMoreInherited,
                        &[("n", Arg::Count(inherited as u64))],
                    )
                };
                section = section.child(div().ml(px(-12.)).child(
                    ui::button("kb-inherited", label, Variant::Ghost, false, t).on_click(
                        cx.listener(|page, _, _, cx| {
                            page.kb_inherited_open = true;
                            cx.notify();
                        }),
                    ),
                ));
            }
        }
        if data.kb_below > 0 {
            let key = match self.owner {
                Owner::Epic(_) => Key::KbMoreEpic,
                _ => Key::KbMoreProject,
            };
            let project = data.project.0;
            section = section.child(
                div().ml(px(-12.)).child(
                    ui::button(
                        "kb-below",
                        tr(locale, key, &[("n", Arg::Count(data.kb_below as u64))]),
                        Variant::Ghost,
                        false,
                        t,
                    )
                    .on_click(cx.listener(move |_, _, _, cx| {
                        cx.emit(PageEvent::Open(Route::Knowledge(project)))
                    })),
                ),
            );
        }
        section.into_any_element()
    }

    fn render_timeline(
        &self,
        data: &Loaded,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let filters = EntryFilterKind::ALL
            .into_iter()
            .enumerate()
            .map(|(i, filter)| {
                let key = match filter {
                    EntryFilterKind::All => Key::TimelineAll,
                    EntryFilterKind::Progress => Key::EntryProgress,
                    EntryFilterKind::Decisions => Key::EntryDecisions,
                    EntryFilterKind::Discussion => Key::EntryDiscussion,
                };
                ui::segment(
                    ("timeline-filter", i),
                    text(locale, key),
                    self.filter == filter,
                    t,
                )
                .on_click(cx.listener(move |page, _, _, cx| {
                    page.filter = filter;
                    page.reload(cx);
                }))
            })
            .collect::<Vec<_>>();
        let mut list = div().flex().flex_col().gap(px(32.)).mt(px(16.));
        let human = when::initial(&when::human_name(locale));
        let shown: Vec<Entry> = record::timeline_entries(&data.entries);
        // The empty state is about what is shown: a timeline of hidden
        // system events alone is empty too.
        if shown.iter().all(|e| e.kind == EntryKind::System) {
            list = list.child(
                type_scale::UI
                    .el()
                    .text_color(t.text_tertiary)
                    .child(text(locale, Key::TimelineEmpty)),
            );
        }
        for (i, entry) in shown.iter().enumerate() {
            list = list.child(self.render_entry(entry, i, &human, locale, t, cx));
        }
        div()
            .flex()
            .flex_col()
            .mt(px(32.))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(ui::section_header(text(locale, Key::SectionTimeline), t))
                    .child(ui::segmented(t).children(filters)),
            )
            .child(list)
            .into_any_element()
    }

    fn render_entry(
        &self,
        entry: &Entry,
        index: usize,
        human: &str,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if entry.kind == EntryKind::System {
            let names = self
                .data
                .as_ref()
                .map(|d| d.event_names.clone())
                .unwrap_or_default();
            let line = system_line(entry, &names, locale);
            // The gutter shows the glyph of the status it was set to.
            let glyph = event_status(entry)
                .map(|s| ui::status_glyph(s, 14., t))
                .unwrap_or_else(|| icon(Icon::CircleDot, 14., t.text_tertiary));
            return div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(12.))
                .child(div().w(px(20.)).flex().justify_center().child(glyph))
                .child(
                    type_scale::SMALL
                        .el()
                        .text_color(t.text_tertiary)
                        .child(line),
                )
                .into_any_element();
        }
        let (glyph, key) = entry_kind_meta(entry.kind);
        let decision = entry.kind == EntryKind::Decision;
        let name = when::author(locale, entry.author);
        let name_el = match entry.author {
            Author::Claude => type_scale::CODE.el().text_color(t.text_primary).child(name),
            Author::Human => type_scale::UI_STRONG
                .el()
                .text_color(t.text_primary)
                .child(name),
        };
        let entry_for_menu = entry.clone();
        let menu_open = self
            .entry_menu
            .as_ref()
            .is_some_and(|m| m.entry.id == entry.id);
        let menu = menu_open.then(|| self.render_entry_menu(locale, t, cx));
        let header = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .h(px(20.))
            .child(name_el)
            .child(icon(glyph, 14., t.text_secondary))
            .child(
                type_scale::SMALL
                    .el()
                    .text_color(if decision {
                        t.text_primary
                    } else {
                        t.text_secondary
                    })
                    .when(decision, |this| {
                        this.font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    })
                    .child(text(locale, key)),
            )
            .child(
                type_scale::SMALL
                    .el()
                    .font_features(type_scale::tabular())
                    .text_color(t.text_tertiary)
                    .child(when::long_alone(locale, entry.stamp.created_at)),
            )
            // An entry edited after it was written says so; the previous
            // wording is kept in an `edited` event (ADR 0005, append-only).
            .when(entry.stamp.updated_at != entry.stamp.created_at, |this| {
                let mark = if entry.stamp.updated_by != entry.author {
                    let name = when::author(locale, entry.stamp.updated_by);
                    tr(locale, Key::EntryEditedBy, &[("name", Arg::Text(&name))])
                } else {
                    text(locale, Key::EntryEdited)
                };
                this.child(
                    type_scale::SMALL
                        .el()
                        .text_color(t.text_tertiary)
                        .child(mark),
                )
            })
            .child(div().flex_1())
            .child(
                div()
                    .when(!menu_open, |this| {
                        this.invisible()
                            .group_hover(SharedString::from(format!("entry-{index}")), |s| {
                                s.visible()
                            })
                    })
                    .relative()
                    .child(
                        ui::ghost_icon_button(
                            format!("entry-menu-{index}"),
                            Icon::Ellipsis,
                            24.,
                            menu_open,
                            false,
                            true,
                            t,
                        )
                        .on_click(cx.listener(
                            move |page, _, window, cx| {
                                page.open_entry_menu(entry_for_menu.clone(), window, cx)
                            },
                        )),
                    )
                    .children(menu_open.then(|| self.entry_anchor.measure_keyed(entry.id.0 as u64)))
                    .children(menu),
            );
        let editing = self
            .entry_edit
            .as_ref()
            .filter(|(id, _)| *id == entry.id)
            .map(|(_, editor)| editor.clone());
        let body: AnyElement = match editing {
            // Scrolled into view once laid out, by the least amount
            // (an entry near the bottom opens its editor below the fold).
            Some(editor) => {
                let reveal = std::rc::Rc::clone(&self.reveal_editor);
                let scroll = self.scroll.clone();
                let tray = self.entry_tray.clone();
                div()
                    .relative()
                    .child(editor_box_with(&editor, tray.as_ref(), locale, t, cx))
                    .child(ui::measure(move |bounds, window| {
                        if reveal.replace(false) {
                            ui::scroll_into_view(&scroll, bounds, ui::Reveal::Whole);
                            window.refresh();
                        }
                    }))
                    .into_any_element()
            }
            None => {
                let strip = self
                    .data
                    .as_ref()
                    .and_then(|d| d.files.get(&AttachOwner::Entry(entry.id)))
                    .and_then(|files| {
                        crate::attach_ui::strip(
                            &format!("entry-{}", entry.id.0),
                            files,
                            crate::attach_ui::StripStyle::Full,
                            t,
                            cx,
                        )
                    });
                div()
                    .flex()
                    .flex_col()
                    .child(markdown_view::short(
                        ("entry", index),
                        entry.text.clone(),
                        t,
                    ))
                    .children(strip.map(|s| div().mt(px(8.)).child(s)))
                    .into_any_element()
            }
        };
        let content = div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .flex_1()
            .min_w_0()
            .child(header)
            .child(body);
        let entry_for_keys = entry.clone();
        let block = record::Block::Entry(entry.id);
        let handle = self.block_focus.get(&block).cloned();
        let reveal = std::rc::Rc::clone(&self.reveal_block);
        let scroll = self.scroll.clone();
        ui::focusable(div().id(("entry-row", index)), t, false)
            .when_some(handle, |this, h| this.track_focus(&h))
            .relative()
            .child(ui::measure(move |bounds, window| {
                if reveal.get() == Some(block) {
                    reveal.set(None);
                    ui::scroll_into_view(&scroll, bounds, ui::Reveal::Whole);
                    window.refresh();
                }
            }))
            .on_key_down(cx.listener(move |page, event: &KeyDownEvent, window, cx| {
                // Shift+F10 or the context-menu key: the entry menu.
                let k = &event.keystroke;
                if (k.key == "f10" && k.modifiers.shift) || k.key == "menu" {
                    page.open_entry_menu(entry_for_keys.clone(), window, cx);
                    cx.stop_propagation();
                }
            }))
            .rounded(px(RADIUS_LG))
            .group(SharedString::from(format!("entry-{index}")))
            .flex()
            .flex_row()
            .gap(px(12.))
            // The box hangs into the gutter by its padding and border, so
            // its author mark and text stay in the same columns as every
            // other entry's; in a narrow page it sits inside the column.
            .when(decision, |this| {
                this.px(px(16.))
                    .when(ui::decision_hangs(ui::side_space()), |this| {
                        this.mx(px(-ui::DECISION_HANG))
                    })
                    .py(px(12.))
                    .rounded(px(RADIUS_LG))
                    .border(px(1.))
                    .border_color(t.border_default)
            })
            .child(ui::author_mark(entry.author, human, t))
            .child(content)
            .into_any_element()
    }

    fn render_table(
        &self,
        data: &Loaded,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let all: Vec<TopicSummary> = data.all_topics();
        if all.is_empty() {
            return self.render_no_topics(locale, t, cx);
        }
        let today = due::today(cx);
        // The Due column and its two segments exist only while a listed
        // topic has a date.
        let dated = table::has_dates(&all);
        let current = self.table_filter.effective(dated);
        // Questions and Report to exist only while a listed topic uses
        // them; Report to goes under 640 of column, Questions and Entries
        // under 520 (components-records.md 5, Topic table).
        let width = ui::column_width();
        let (q_col, r_col) = crate::questions_model::table_columns(&all, &data.questions.per_topic);
        let q_col = q_col && width >= 520.;
        let r_col = r_col && width >= 640.;
        let entries_col = width >= 520.;
        let filters = table::Filter::shown(dated)
            .iter()
            .enumerate()
            .map(|(i, &filter)| {
                let key = match filter {
                    table::Filter::Open => Key::TableFilterOpen,
                    table::Filter::Overdue => Key::TableFilterOverdue,
                    table::Filter::Week => Key::TableFilterWeek,
                    table::Filter::All => Key::TableFilterAll,
                    table::Filter::Closed => Key::TableFilterClosed,
                };
                ui::segment(("table-filter", i), text(locale, key), current == filter, t).on_click(
                    cx.listener(move |page, _, _, cx| {
                        // The table keeps at least the height it had, so a
                        // shorter list never pulls the page (and the
                        // segment under the pointer) up (QA 2026-11-22).
                        page.table_floor = page.table_floor.max(page.table_height.get());
                        page.table_filter = filter;
                        cx.notify();
                    }),
                )
            })
            .collect::<Vec<_>>();
        let chip = data.chip.as_ref().filter(|(f, _)| Some(*f) == self.chip);
        let count = match chip {
            Some((_, ids)) => table::chip_rows(&all, ids, None).len(),
            None => all.iter().filter(|t| current.keeps(t, today)).count(),
        };
        let control = match chip {
            Some((filter, _)) => self.render_chip(*filter, locale, t, cx),
            None => ui::segmented(t).children(filters).into_any_element(),
        };
        let toolbar = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.))
            .h(px(28.))
            .child(control)
            .child(type_scale::SMALL.el().text_color(t.text_tertiary).child(tr(
                locale,
                Key::TableCount,
                &[("n", Arg::Count(count as u64))],
            )))
            .when(self.table_sort.is_some(), |this| {
                this.child(
                    ui::button(
                        "table-reset",
                        text(locale, Key::TableReset),
                        Variant::Ghost,
                        false,
                        t,
                    )
                    .on_click(cx.listener(|page, _, _, cx| {
                        page.table_sort = None;
                        cx.notify();
                    })),
                )
            })
            .child(div().flex_1())
            .child(
                ui::ghost_icon_button("table-new", Icon::Plus, 24., false, false, true, t)
                    .on_click(cx.listener(|_, _, window, cx| {
                        window.dispatch_action(Box::new(crate::shell::NewTopic), cx);
                    })),
            );
        let header_cell = |id: &'static str,
                           column: Column,
                           key: Option<Key>,
                           width: Option<f32>,
                           right: bool,
                           page: &Self,
                           cx: &mut Context<Self>| {
            let arrow = page.table_sort.filter(|s| s.column == column).map(|s| {
                icon(
                    if s.ascending {
                        Icon::ChevronUp
                    } else {
                        Icon::ChevronDown
                    },
                    12.,
                    t.text_tertiary,
                )
            });
            div()
                .id(id)
                .flex()
                .flex_row()
                .items_center()
                .gap(px(4.))
                .when(right, |this| this.justify_end())
                .map(|this| match width {
                    Some(w) => this.w(px(w)).flex_shrink_0(),
                    None => this.flex_1().min_w_0(),
                })
                .on_click(cx.listener(move |page, _, _, cx| {
                    page.table_sort = Some(table::next_sort(page.table_sort, column));
                    cx.notify();
                }))
                .children(key.map(|key| text(locale, key)))
                .children(arrow)
        };
        let header = type_scale::SMALL
            .el()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .h(px(28.))
            .px(px(8.))
            .mx(px(-8.))
            .mt(px(8.))
            .text_color(t.text_tertiary)
            .border_b(px(1.))
            .border_color(t.border_subtle)
            .child(header_cell(
                "th-status",
                Column::Status,
                None,
                Some(24.),
                false,
                self,
                cx,
            ))
            .child(header_cell(
                "th-title",
                Column::Title,
                Some(Key::TableTitle),
                None,
                false,
                self,
                cx,
            ))
            .when(dated, |this| {
                this.child(header_cell(
                    "th-due",
                    Column::Due,
                    Some(Key::TableDue),
                    Some(88.),
                    true,
                    self,
                    cx,
                ))
            })
            .when(q_col, |this| {
                this.child(
                    div()
                        .w(px(64.))
                        .flex_shrink_0()
                        .text_right()
                        .child(text(locale, Key::TableQuestions)),
                )
            })
            .when(r_col, |this| {
                this.child(
                    div()
                        .w(px(120.))
                        .flex_shrink_0()
                        .child(text(locale, Key::TableReportTo)),
                )
            })
            .when(entries_col, |this| {
                this.child(header_cell(
                    "th-entries",
                    Column::Entries,
                    Some(Key::TableEntries),
                    Some(56.),
                    true,
                    self,
                    cx,
                ))
            })
            .child(header_cell(
                "th-updated",
                Column::Updated,
                Some(Key::TableUpdated),
                Some(72.),
                true,
                self,
                cx,
            ));
        let mut body = div().flex().flex_col();
        let mut row_index: usize = 0;
        self.row_ids.borrow_mut().clear();
        let mut row = |topic: &TopicSummary, cx: &mut Context<Self>| {
            let id = topic.id;
            self.row_ids.borrow_mut().push(id);
            let handle = self.row_focus.get(row_index).cloned();
            // The row's picker hangs under its title (status 24 + gap 8 +
            // padding 8 from the row's edge).
            let picker = self
                .table_due
                .as_ref()
                .filter(|d| d.row == row_index && d.topic == id)
                .map(|d| {
                    div()
                        .absolute()
                        .top_0()
                        .left(px(40.))
                        .w(px(1.))
                        .h(px(32.))
                        .child(d.anchor.measure())
                        .children(
                            d.anchor
                                .below_at(d.picker.clone(), crate::menu::Align::Start, 2),
                        )
                });
            let open = topic.status.is_open();
            let due_cell = dated.then(|| {
                let cell = due::cell(locale, topic.due_on, today, open);
                div()
                    .id(("table-due", row_index))
                    .w(px(88.))
                    .flex_shrink_0()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_end()
                    .gap(px(4.))
                    .when_some(cell, |this, cell| {
                        let color = tone_color(cell.tone, t);
                        this.tooltip(ui::tip(cell.tip))
                            .when(cell.alert, |this| {
                                this.child(icon(Icon::ClockAlert, 12., color))
                            })
                            .child(
                                type_scale::SMALL
                                    .el()
                                    .font_features(type_scale::tabular())
                                    .text_color(color)
                                    .child(cell.text),
                            )
                    })
            });
            let questions_cell = q_col.then(|| {
                let (n, late) = data.questions.per_topic.get(&id).copied().unwrap_or((0, 0));
                let tip = if late > 0 {
                    tr(
                        locale,
                        Key::TableQuestionsTipOverdue,
                        &[
                            ("n", Arg::Count(n as u64)),
                            ("late", Arg::Count(late as u64)),
                        ],
                    )
                } else {
                    tr(
                        locale,
                        Key::TableQuestionsTip,
                        &[("n", Arg::Count(n as u64))],
                    )
                };
                type_scale::SMALL
                    .el()
                    .id(("table-questions", row_index))
                    .w(px(64.))
                    .flex_shrink_0()
                    .text_right()
                    .font_features(type_scale::tabular())
                    .text_color(if late > 0 {
                        t.text_secondary
                    } else {
                        t.text_tertiary
                    })
                    .when(n > 0, |this| {
                        this.tooltip(ui::tip(tip)).child(number(locale, n as u64))
                    })
            });
            let report_cell = r_col.then(|| {
                let name = topic.report_to.as_ref().map(|c| c.name.clone());
                type_scale::SMALL
                    .el()
                    .id(("table-report", row_index))
                    .w(px(120.))
                    .flex_shrink_0()
                    .truncate()
                    .text_color(if topic.reported {
                        t.text_tertiary
                    } else {
                        t.text_secondary
                    })
                    .when_some(name, |this, name| {
                        this.tooltip(ui::tip(name.clone())).child(name)
                    })
            });
            let r = ui::focusable(div().id(("table-row", row_index)), t, false)
                .when_some(handle, |this, handle| this.track_focus(&handle))
                .relative()
                .children(picker)
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                .h(px(32.))
                .px(px(8.))
                .mx(px(-8.))
                .rounded(px(RADIUS_SM))
                .hover({
                    let hover = t.fill_hover;
                    move |s| s.bg(hover)
                })
                .on_click(
                    cx.listener(move |_, _, _, cx| cx.emit(PageEvent::Open(Route::Topic(id)))),
                )
                .child(div().w(px(24.)).flex_shrink_0().child(ui::status_glyph(
                    topic.status,
                    14.,
                    t,
                )))
                .child(
                    type_scale::UI
                        .el()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_color(if open {
                            t.text_primary
                        } else {
                            t.text_tertiary
                        })
                        .child(topic.title.clone()),
                )
                .children(due_cell)
                .children(questions_cell)
                .children(report_cell)
                .when(entries_col, |this| {
                    this.child(
                        type_scale::SMALL
                            .el()
                            .w(px(56.))
                            .flex_shrink_0()
                            .text_right()
                            .font_features(type_scale::tabular())
                            .text_color(t.text_tertiary)
                            .child(number(locale, topic.entries.max(0) as u64)),
                    )
                })
                .child(
                    type_scale::SMALL
                        .el()
                        .w(px(72.))
                        .flex_shrink_0()
                        .text_right()
                        .font_features(type_scale::tabular())
                        .text_color(t.text_tertiary)
                        .child(when::compact(locale, topic.updated_at)),
                );
            row_index += 1;
            r
        };
        if let Some((_, ids)) = chip {
            // Under a chip the table is the chip's list, ungrouped.
            for topic in table::chip_rows(&all, ids, self.table_sort) {
                body = body.child(row(topic, cx));
            }
        } else if matches!(self.owner, Owner::Epic(_)) {
            for topic in table::rows(&data.topics, current, self.table_sort, today) {
                body = body.child(row(topic, cx));
            }
        } else if !table::grouped(current) {
            // Overdue and Next 7 days: one list by date across epics.
            for topic in table::rows(&all, current, self.table_sort, today) {
                body = body.child(row(topic, cx));
            }
        } else {
            for (epic, topics) in table::groups(&data.loose, &data.groups) {
                let shown = table::rows(topics, current, self.table_sort, today);
                // A group with nothing under the segment has no header.
                if shown.is_empty() {
                    continue;
                }
                let (done, total) = table::progress(topics);
                let name: SharedString = match epic {
                    Some((_, name)) => name.to_owned().into(),
                    None => text(locale, Key::TreeNoEpic).into(),
                };
                let group = div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .h(px(28.))
                    .px(px(8.))
                    .mx(px(-8.))
                    .mt(px(8.))
                    .child(ui::label(name, t))
                    .child(
                        type_scale::SMALL
                            .el()
                            .font_features(type_scale::tabular())
                            .text_color(t.text_tertiary)
                            .child(tr(
                                locale,
                                Key::TableGroupProgress,
                                &[
                                    ("done", Arg::Count(done as u64)),
                                    ("total", Arg::Count(total as u64)),
                                ],
                            )),
                    );
                body = body.child(group);
                for topic in shown {
                    body = body.child(row(topic, cx));
                }
            }
        }
        let reveal = std::rc::Rc::clone(&self.reveal_table);
        let scroll = self.scroll.clone();
        let height = std::rc::Rc::clone(&self.table_height);
        let body = body
            .relative()
            .min_h(px(self.table_floor))
            .child(ui::measure(move |bounds, _| {
                height.set(zoom::eff(bounds.size.height));
            }));
        div()
            .id("topic-table")
            .relative()
            .track_focus(&self.table_focus)
            .flex()
            .flex_col()
            .mt(px(32.))
            .child(toolbar)
            .child(header)
            .child(body)
            .child(ui::measure(move |bounds, window| {
                if reveal.replace(false) {
                    ui::scroll_into_view(&scroll, bounds, ui::Reveal::Top);
                    window.refresh();
                }
            }))
            .into_any_element()
    }

    /// The Overview (components-records.md 5, screens.md 8.20): on a project
    /// page the header, the attention lines that apply and "More"; on an
    /// epic page the collapsed "Needs attention", only when something
    /// applies. `None` where there is nothing to show.
    fn render_overview(
        &self,
        data: &Loaded,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let o = data.overview.as_ref()?;
        let open = self.overview_open(cx);
        let count = |n: usize| [("n", Arg::Count(n as u64))];
        // `None`: the late answers, which open the Questions page.
        let mut lines: Vec<(Icon, gpui_kit::Hsla, String, Option<OverviewFilter>)> = Vec::new();
        if o.blocked > 0 {
            let words = tr(locale, Key::OverviewBlocked, &count(o.blocked));
            lines.push((
                Icon::CirclePause,
                t.warning,
                words,
                Some(OverviewFilter::Blocked),
            ));
        }
        if o.overdue > 0 {
            let words = tr(locale, Key::OverviewOverdue, &count(o.overdue));
            lines.push((
                Icon::ClockAlert,
                t.warning,
                words,
                Some(OverviewFilter::Overdue),
            ));
        }
        // The fifth line, late answers (components-records.md 5).
        if o.answers_late > 0 {
            let words = tr(locale, Key::OverviewQuestionsLate, &count(o.answers_late));
            lines.push((Icon::CircleQuestionMark, t.warning, words, None));
        }
        if o.idle > 0 {
            let words = tr(locale, Key::OverviewIdle, &count(o.idle));
            lines.push((
                Icon::CircleDot,
                t.text_secondary,
                words,
                Some(OverviewFilter::Idle),
            ));
        }
        // The inbox is project-wide: never on an epic page.
        if matches!(self.owner, Owner::Project(_)) && o.inbox_needs_attention() {
            let days = o.inbox_oldest_days.unwrap_or_default().max(0) as u64;
            let words = tr(
                locale,
                Key::OverviewInbox,
                &[
                    ("n", Arg::Count(o.inbox as u64)),
                    ("days", Arg::Count(days)),
                ],
            );
            lines.push((
                Icon::Inbox,
                t.text_secondary,
                words,
                Some(OverviewFilter::Inbox),
            ));
        }
        let line_els = lines
            .into_iter()
            .enumerate()
            .map(|(i, (glyph, color, words, filter))| {
                overview_control(("ov-line", i), 32., t)
                    .on_click(cx.listener(move |page, _, _, cx| match filter {
                        Some(filter) => page.show_filter(filter, cx),
                        None => page.show_late_answers(cx),
                    }))
                    .child(
                        div()
                            .size(px(16.))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(icon(glyph, 14., color)),
                    )
                    .child(
                        type_scale::UI
                            .el()
                            .font_features(type_scale::tabular())
                            .text_color(t.text_primary)
                            .child(words),
                    )
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        match self.owner {
            Owner::Topic(_) => None,
            Owner::Epic(_) => {
                // Present only when a line applies; collapsed by default.
                if line_els.is_empty() {
                    return None;
                }
                let n = line_els.len();
                let header = ui::focusable(div().id("ov-attention"), t, false)
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .h(px(28.))
                    .px(px(8.))
                    .mx(px(-8.))
                    .rounded(px(RADIUS_SM))
                    .hover({
                        let hover = t.fill_hover;
                        move |s| s.bg(hover)
                    })
                    .on_click(cx.listener(move |page, _, _, cx| page.set_overview_open(!open, cx)))
                    .on_key_down(cx.listener(|page, event: &KeyDownEvent, _, cx| {
                        match event.keystroke.key.as_str() {
                            "right" => page.set_overview_open(true, cx),
                            "left" => page.set_overview_open(false, cx),
                            _ => return,
                        }
                        cx.stop_propagation();
                    }))
                    .child(icon(
                        if open {
                            Icon::ChevronDown
                        } else {
                            Icon::ChevronRight
                        },
                        14.,
                        t.text_secondary,
                    ))
                    .child(ui::section_header(text(locale, Key::OverviewAttention), t))
                    .child(
                        type_scale::SMALL
                            .el()
                            .font_features(type_scale::tabular())
                            .text_color(t.text_tertiary)
                            .child(number(locale, n as u64)),
                    );
                Some(
                    div()
                        .flex()
                        .flex_col()
                        .mt(px(24.))
                        .child(header)
                        .when(open, |this| {
                            this.child(div().flex().flex_col().mt(px(4.)).children(line_els))
                        })
                        .into_any_element(),
                )
            }
            Owner::Project(_) => {
                // Present while the project has a topic; its empty state
                // says what to do otherwise.
                if data.all_topics().is_empty() {
                    return None;
                }
                let calm = line_els.is_empty();
                let more = ui::focusable(div().id("ov-more"), t, false)
                    .flex()
                    .flex_row()
                    .flex_none()
                    .items_center()
                    .gap(px(4.))
                    .h(px(24.))
                    .px(px(8.))
                    .rounded(px(RADIUS_SM))
                    .text_color(t.text_secondary)
                    .hover({
                        let (hover, primary) = (t.fill_hover, t.text_primary);
                        move |s| s.bg(hover).text_color(primary)
                    })
                    .on_click(cx.listener(move |page, _, _, cx| page.set_overview_open(!open, cx)))
                    .child(type_scale::UI.el().child(text(
                        locale,
                        if open {
                            Key::OverviewLess
                        } else {
                            Key::OverviewMore
                        },
                    )))
                    .child(icon(
                        if open {
                            Icon::ChevronUp
                        } else {
                            Icon::ChevronDown
                        },
                        14.,
                        t.text_secondary,
                    ));
                let header = div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .h(px(28.))
                    .child(ui::section_header(text(locale, Key::OverviewTitle), t))
                    .when(calm, |this| {
                        this.child(
                            type_scale::SMALL
                                .el()
                                .text_color(t.text_tertiary)
                                .child(text(locale, Key::OverviewClear)),
                        )
                    })
                    .child(div().flex_1())
                    .child(more);
                let mut section = div()
                    .flex()
                    .flex_col()
                    .mt(px(24.))
                    .child(header)
                    .when(!calm, |this| {
                        this.child(div().flex().flex_col().mt(px(4.)).children(line_els))
                    });
                if open {
                    section = section.children(self.render_more(o, locale, t, cx));
                }
                Some(section.into_any_element())
            }
        }
    }

    /// Behind "More": Last 7 days, Epics and the memory line, each only
    /// when it has something to say.
    fn render_more(
        &self,
        o: &acta_core::store::Overview,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let block = |label: String, content: AnyElement| {
            div()
                .flex()
                .flex_col()
                .mt(px(16.))
                .child(
                    type_scale::SMALL
                        .el()
                        .h(px(16.))
                        .text_color(t.text_tertiary)
                        .child(label),
                )
                .child(div().mt(px(4.)).child(content))
                .into_any_element()
        };
        let mut blocks = Vec::new();
        // Last 7 days.
        let week: Vec<(Key, usize, OverviewFilter)> = [
            (Key::OverviewWeekDone, o.done, OverviewFilter::Done),
            (
                Key::OverviewWeekDecisions,
                o.decisions,
                OverviewFilter::Decided,
            ),
            (Key::OverviewWeekNew, o.created, OverviewFilter::Created),
        ]
        .into_iter()
        .filter(|(_, n, _)| *n > 0)
        .collect();
        if !week.is_empty() {
            let links = week.into_iter().enumerate().map(|(i, (key, n, filter))| {
                overview_control(("ov-week", i), 28., t)
                    .mx(px(0.))
                    .ml(if i == 0 { px(-8.) } else { px(0.) })
                    .on_click(cx.listener(move |page, _, _, cx| page.show_filter(filter, cx)))
                    .child(
                        type_scale::UI
                            .el()
                            .font_features(type_scale::tabular())
                            .text_color(t.text_primary)
                            .child(tr(locale, key, &[("n", Arg::Count(n as u64))])),
                    )
            });
            blocks.push(block(
                text(locale, Key::OverviewWeek),
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap(px(8.))
                    .children(links)
                    .into_any_element(),
            ));
        } else if let Some(at) = o.last_activity {
            let when = when::long(locale, at);
            blocks.push(block(
                text(locale, Key::OverviewWeek),
                type_scale::UI
                    .el()
                    .text_color(t.text_primary)
                    .child(tr(
                        locale,
                        Key::OverviewWeekNone,
                        &[("when", Arg::Text(&when))],
                    ))
                    .into_any_element(),
            ));
        }
        // Epics.
        if !o.epics.is_empty() || o.complete_epics > 0 {
            const SHOWN: usize = 8;
            let today = due::today(cx);
            let dated = o.epics.iter().any(|e| e.due_on.is_some());
            let mut rows = div().flex().flex_col();
            for (i, epic) in o.epics.iter().take(SHOWN).enumerate() {
                let (done, total) = (epic.progress.done, epic.progress.total);
                let fraction = if total > 0 {
                    done as f32 / total as f32
                } else {
                    0.
                };
                let name = match epic.epic {
                    Some(_) => epic.title.clone(),
                    None => text(locale, Key::TreeNoEpic),
                };
                let target = epic.epic;
                let cell = due::cell(locale, epic.due_on, today, epic.progress.is_open());
                rows = rows.child(
                    overview_control(("ov-epic", i), 28., t)
                        .on_click(cx.listener(move |page, _, _, cx| match target {
                            Some(id) => cx.emit(PageEvent::Open(Route::Epic(id))),
                            // "No epic": the table, at its "No epic" group
                            // (the first).
                            None => {
                                page.chip = None;
                                page.reveal_table_now(cx);
                            }
                        }))
                        .child(
                            type_scale::UI
                                .el()
                                .id(("ov-epic-name", i))
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_color(t.text_primary)
                                .tooltip(ui::tip(name.clone()))
                                .child(name),
                        )
                        .child(
                            div()
                                .w(px(120.))
                                .h(px(4.))
                                .flex_shrink_0()
                                .rounded(px(RADIUS_XS))
                                .bg(t.fill_selected)
                                .child(
                                    div()
                                        .h_full()
                                        .w(px(120. * fraction))
                                        .rounded(px(RADIUS_XS))
                                        .bg(t.text_secondary),
                                ),
                        )
                        .child(
                            type_scale::SMALL
                                .el()
                                .ml(px(4.))
                                .flex_shrink_0()
                                .whitespace_nowrap()
                                .font_features(type_scale::tabular())
                                .text_color(t.text_tertiary)
                                .child(tr(
                                    locale,
                                    Key::TableGroupProgress,
                                    &[
                                        ("done", Arg::Count(done as u64)),
                                        ("total", Arg::Count(total as u64)),
                                    ],
                                )),
                        )
                        .when(dated, |this| {
                            this.child(
                                div()
                                    .w(px(88.))
                                    .flex_shrink_0()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .justify_end()
                                    .gap(px(4.))
                                    .when_some(cell, |this, cell| {
                                        let color = tone_color(cell.tone, t);
                                        this.when(cell.alert, |this| {
                                            this.child(icon(Icon::ClockAlert, 12., color))
                                        })
                                        .child(
                                            type_scale::SMALL
                                                .el()
                                                .font_features(type_scale::tabular())
                                                .text_color(color)
                                                .child(cell.text),
                                        )
                                    }),
                            )
                        }),
                );
            }
            if o.epics.len() > SHOWN {
                let more = o.epics.len() - SHOWN;
                rows = rows.child(
                    overview_control("ov-epics-more", 28., t)
                        .on_click(cx.listener(|page, _, _, cx| {
                            page.chip = None;
                            page.reveal_table_now(cx);
                        }))
                        .child(type_scale::UI.el().text_color(t.text_primary).child(tr(
                            locale,
                            Key::OverviewEpicsMore,
                            &[("n", Arg::Count(more as u64))],
                        ))),
                );
            }
            if o.complete_epics > 0 {
                rows = rows.child(
                    type_scale::UI
                        .el()
                        .h(px(28.))
                        .flex()
                        .items_center()
                        .text_color(t.text_tertiary)
                        .child(tr(
                            locale,
                            Key::OverviewEpicsComplete,
                            &[("n", Arg::Count(o.complete_epics as u64))],
                        )),
                );
            }
            blocks.push(block(
                text(locale, Key::OverviewEpics),
                rows.into_any_element(),
            ));
        }
        // Knowledge.
        if o.stale_memories > 0 {
            let line =
                overview_control("ov-memories", 28., t)
                    .on_click(cx.listener(|page, _, _, cx| {
                        page.show_filter(OverviewFilter::StaleMemories, cx)
                    }))
                    .child(
                        type_scale::UI
                            .el()
                            .font_features(type_scale::tabular())
                            .text_color(t.text_primary)
                            .child(tr(
                                locale,
                                Key::OverviewToCheck,
                                &[("n", Arg::Count(o.stale_memories as u64))],
                            )),
                    );
            blocks.push(block(
                text(locale, Key::SectionKnowledge),
                line.into_any_element(),
            ));
        }
        blocks
    }

    /// The filter chip (components-records.md 5): in place of the segmented
    /// control while set; its `x`, or `Backspace` on it, clears it.
    fn render_chip(
        &self,
        filter: OverviewFilter,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let key = match filter {
            OverviewFilter::Blocked => Key::FilterBlocked,
            OverviewFilter::Idle => Key::FilterIdle,
            OverviewFilter::Done => Key::FilterDoneWeek,
            OverviewFilter::Decided => Key::FilterDecidedWeek,
            OverviewFilter::Created => Key::FilterCreatedWeek,
            // Not chips of the table: Overdue is its own segment, the
            // inbox and memories open their pages.
            _ => Key::FilterBlocked,
        };
        ui::focusable(div().id("table-chip"), t, false)
            .flex()
            .flex_row()
            .flex_none()
            .items_center()
            .gap(px(4.))
            .h(px(24.))
            .pl(px(8.))
            .pr(px(4.))
            .rounded(px(RADIUS_SM))
            .bg(t.fill_selected)
            .on_key_down(cx.listener(|page, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "backspace" {
                    page.clear_chip(cx);
                    cx.stop_propagation();
                }
            }))
            .child(
                type_scale::UI
                    .el()
                    .text_color(t.text_primary)
                    .child(text(locale, key)),
            )
            .child(
                ui::ghost_icon_button("table-chip-clear", Icon::X, 16., false, false, true, t)
                    .tooltip(ui::tip(text(locale, Key::FilterClear)))
                    .on_click(cx.listener(|page, _, _, cx| page.clear_chip(cx))),
            )
            .into_any_element()
    }

    /// No topics at all (components.md 5, Empty state): the heading, one
    /// sentence, then New topic with its key.
    fn render_no_topics(&self, locale: Locale, t: &Tokens, cx: &mut Context<Self>) -> AnyElement {
        let macos = cfg!(target_os = "macos");
        div()
            .flex()
            .flex_col()
            .mt(px(32.))
            .child(
                type_scale::HEADING
                    .el()
                    .text_color(t.text_primary)
                    .child(text(locale, Key::TopicEmptyHeading)),
            )
            .child(
                type_scale::UI
                    .el()
                    .mt(px(8.))
                    .max_w(px(480.))
                    .text_color(t.text_tertiary)
                    .child(text(locale, Key::TopicEmptyProject)),
            )
            .child(
                div().mt(px(16.)).flex().child(
                    ui::button(
                        "table-empty-new",
                        text(locale, Key::TopicNew),
                        Variant::Primary,
                        false,
                        t,
                    )
                    .child(ui::kbd_on_accent(
                        &[KeyName::Mod.label(locale, macos), "T"],
                        t,
                    ))
                    .on_click(cx.listener(|_, _, window, cx| {
                        window.dispatch_action(Box::new(crate::shell::NewTopic), cx);
                    })),
                ),
            )
            .into_any_element()
    }

    /// The entry menu, under its `ellipsis` button.
    fn render_entry_menu(&self, locale: Locale, t: &Tokens, cx: &mut Context<Self>) -> AnyElement {
        let Some(menu) = &self.entry_menu else {
            return div().into_any_element();
        };
        let mut panel =
            menu::panel("entry-menu", t).on_mouse_down_out(cx.listener(|page, _, _, cx| {
                page.entry_menu = None;
                cx.notify();
            }));
        if menu.kinds {
            for (i, kind) in ENTRY_KINDS.into_iter().enumerate() {
                let (glyph, key) = entry_kind_meta(kind);
                panel =
                    panel.child(
                        menu::item(
                            ("entry-kind", i),
                            Some(icon(glyph, 16., t.text_secondary).into_any_element()),
                            text(locale, key),
                            (kind == menu.entry.kind).then(|| menu::check(t)),
                            i == menu.highlight,
                            t,
                        )
                        .on_click(cx.listener(
                            move |page, _, window, cx| page.choose_entry_item(i, window, cx),
                        )),
                    );
            }
        } else {
            let items = [
                Key::CommonEdit,
                Key::EntryCopyMarkdown,
                Key::EntryChangeType,
                Key::CommonDelete,
            ];
            for (i, key) in items.into_iter().enumerate() {
                let danger = i == 3;
                if danger {
                    panel = panel.child(menu::separator(t));
                }
                panel =
                    panel.child(
                        menu::item(
                            ("entry-item", i),
                            None,
                            text(locale, key),
                            None,
                            i == menu.highlight,
                            t,
                        )
                        .when(danger, |this| this.text_color(t.danger))
                        .on_click(cx.listener(
                            move |page, _, window, cx| page.choose_entry_item(i, window, cx),
                        )),
                    );
            }
        }
        self.entry_anchor
            .below_keyed(menu.entry.id.0 as u64, panel, menu::Align::End, 1)
            .unwrap_or_else(|| div().into_any_element())
    }

    /// The `plus` menu of the knowledge section: New document, New memory,
    /// with their keys.
    fn render_kb_new_menu(
        &self,
        highlight: usize,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let macos = cfg!(target_os = "macos");
        let m = KeyName::Mod.label(locale, macos);
        let shift = KeyName::Shift.label(locale, macos);
        let rows = [
            (Key::KbNewDocument, Icon::FileText, "D", KbKind::Document),
            (Key::KbNewMemory, Icon::Pin, "M", KbKind::Memory),
        ];
        let mut panel =
            menu::panel("kb-new-menu", t).on_mouse_down_out(cx.listener(|page, _, _, cx| {
                page.kb_new_menu = None;
                cx.notify();
            }));
        for (i, (key, glyph, letter, kind)) in rows.into_iter().enumerate() {
            panel = panel.child(
                menu::item(
                    ("kb-new-item", i),
                    Some(icon(glyph, 16., t.text_secondary).into_any_element()),
                    text(locale, key),
                    Some(ui::kbd(&[m, shift, letter], t).into_any_element()),
                    i == highlight,
                    t,
                )
                .on_click(cx.listener(move |page, _, _, cx| {
                    page.kb_new_menu = None;
                    page.new_kb(kind, cx);
                })),
            );
        }
        self.kb_new_anchor
            .below(panel, menu::Align::End)
            .unwrap_or_else(|| div().into_any_element())
    }
}

/// The status a system event set, if it is a status change.
impl RecordPage {
    /// components-app.md 5, Claude Code sessions, Page line: "Claude Code
    /// is working here · since 14:05 · last: WAL on", and "Show terminal"
    /// when the session runs in Acta's terminal.
    fn render_claude_line(
        &self,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let live = crate::sessions::live(cx);
        let here = crate::sessions::here(live, self.owner)?;
        let on: Vec<_> = live
            .iter()
            .filter(|s| crate::sessions::place(s) == self.owner)
            .collect();
        let first = on.iter().min_by_key(|s| s.started_at)?;
        // Since and last activity are the line's tooltip, one per line
        // (clarity review 2026-11-22).
        let mut tip = vec![crate::sessions::since(locale, first.started_at)];
        if here.count == 1 {
            tip.extend(crate::sessions::last(locale, first));
        }
        let tip = tip.join("\n");
        let terminals = cx
            .try_global::<crate::sessions_view::TerminalTokens>()
            .map(|g| g.0.clone())
            .unwrap_or_default();
        let in_acta = on.iter().find_map(|s| {
            let token = s.launch.as_deref()?;
            terminals
                .iter()
                .find(|a| a.token == token)
                .map(|a| a.key.holder)
                .filter(|h| *h == crate::tools_model::Holder::Page(self.owner))
                .map(|_| self.owner)
        });
        Some(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                // Under the add chips' band (components-records.md 5).
                .mt(px(ADD_BAND + 8.))
                .min_w_0()
                .child(icon(
                    Icon::Bot,
                    14.,
                    crate::sessions::state_color(here.state, t),
                ))
                .child(
                    type_scale::SMALL
                        .el()
                        .id("claude-line")
                        .min_w_0()
                        .truncate()
                        .text_color(t.text_secondary)
                        .tooltip(ui::tip(tip))
                        .child(crate::sessions::here_text(locale, here)),
                )
                .when_some(in_acta, |this, owner| {
                    this.child(
                        ui::button(
                            "claude-show-terminal",
                            text(locale, Key::SessionsShowTerminal),
                            Variant::Ghost,
                            false,
                            t,
                        )
                        .h(px(24.))
                        .on_click(
                            cx.listener(move |_, _, _, cx| cx.emit(PageEvent::ShowTerminal(owner))),
                        ),
                    )
                })
                .into_any_element(),
        )
    }
}

fn event_status(entry: &Entry) -> Option<Status> {
    entry
        .event
        .as_ref()
        .filter(|e| e.kind == acta_core::model::EventKind::Status)
        .and_then(|e| e.new.as_deref())
        .and_then(|s| {
            Status::ALL
                .into_iter()
                .find(|st| format!("{st:?}").eq_ignore_ascii_case(s))
        })
}

fn owner_label(owner: Owner, data: &Loaded) -> String {
    match owner {
        Owner::Project(_) => data.project.1.clone(),
        Owner::Epic(_) => data.epic.as_ref().map(|e| e.1.clone()).unwrap_or_default(),
        Owner::Topic(_) => data.title.clone(),
    }
}

fn system_line(
    entry: &Entry,
    names: &std::collections::HashMap<String, String>,
    locale: Locale,
) -> String {
    let author = when::author(locale, entry.author);
    let when = when::long(locale, entry.stamp.created_at);
    // Report to events (request 19): the contact drawn with its name now.
    if let Some(event) = entry.event.as_ref().filter(|e| {
        matches!(
            e.kind,
            acta_core::model::EventKind::Report | acta_core::model::EventKind::Reported
        )
    }) {
        let name = |uuid: &Option<String>| {
            uuid.as_ref()
                .and_then(|u| names.get(u))
                .cloned()
                .unwrap_or_default()
        };
        let what = match (event.kind, &event.new) {
            (acta_core::model::EventKind::Report, Some(_)) => tr(
                locale,
                Key::EventReportSet,
                &[
                    ("author", Arg::Text(&author)),
                    ("name", Arg::Text(&name(&event.new))),
                ],
            ),
            (acta_core::model::EventKind::Report, None) => tr(
                locale,
                Key::EventReportCleared,
                &[("author", Arg::Text(&author))],
            ),
            (_, Some(_)) => tr(
                locale,
                Key::EventReported,
                &[
                    ("author", Arg::Text(&author)),
                    ("name", Arg::Text(&name(&event.new))),
                ],
            ),
            (_, None) => tr(
                locale,
                Key::EventReportUnmarked,
                &[("author", Arg::Text(&author))],
            ),
        };
        return format!("{what} \u{2013} {when}");
    }
    let status = event_status(entry);
    let due = entry
        .event
        .as_ref()
        .filter(|e| e.kind == acta_core::model::EventKind::Due);
    let what = match (status, due) {
        (Some(status), _) => {
            let name = text(locale, ui::status_key(status));
            tr(
                locale,
                Key::EventStatus,
                &[("author", Arg::Text(&author)), ("status", Arg::Text(&name))],
            )
        }
        (None, Some(event)) => match event.new.as_deref().and_then(Date::parse_iso) {
            Some(day) => {
                let date = due::date(locale, day);
                tr(
                    locale,
                    Key::EventDueSet,
                    &[("author", Arg::Text(&author)), ("date", Arg::Text(&date))],
                )
            }
            None => tr(
                locale,
                Key::EventDueCleared,
                &[("author", Arg::Text(&author))],
            ),
        },
        (None, None) => entry.text.clone(),
    };
    format!("{what} \u{2013} {when}")
}

/// A ghost control of the Overview: a row `height` high, padding 0 x 8,
/// hanging 8 into the gutter as list rows do, hover and pressed fills,
/// the focus ring.
fn overview_control(
    id: impl Into<gpui_kit::ElementId>,
    height: f32,
    t: &Tokens,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    let (hover, pressed) = (t.fill_hover, t.fill_pressed);
    ui::focusable(div().id(id.into()), t, false)
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.))
        .h(px(height))
        .px(px(8.))
        .mx(px(-8.))
        .rounded(px(RADIUS_SM))
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .active(move |s| s.bg(pressed))
}

/// The colour of a due phrase or cell.
fn tone_color(tone: Tone, t: &Tokens) -> gpui_kit::Hsla {
    match tone {
        Tone::Warning => t.warning,
        Tone::Secondary => t.text_secondary,
        Tone::Tertiary => t.text_tertiary,
    }
}

fn progress(data: &Loaded, locale: Locale, t: &Tokens) -> AnyElement {
    let (done, total) = data.progress();
    if total == 0 {
        // No topic to count: the words alone, no empty bar.
        return div()
            .text_color(t.text_tertiary)
            .child(text(locale, Key::ProgressNone))
            .into_any_element();
    }
    let fraction = if total > 0 {
        done as f32 / total as f32
    } else {
        0.
    };
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(12.))
        .child(tr(
            locale,
            Key::PropProgressValue,
            &[
                ("done", Arg::Count(done as u64)),
                ("total", Arg::Count(total as u64)),
            ],
        ))
        .child(
            div()
                .w(px(120.))
                .h(px(4.))
                .rounded(px(RADIUS_XS))
                .bg(t.fill_selected)
                .child(
                    div()
                        .h_full()
                        .w(px(120. * fraction))
                        .rounded(px(RADIUS_XS))
                        .bg(t.text_secondary),
                ),
        )
        .into_any_element()
}

/// A link in the breadcrumb style: `text.primary`, underlined on hover.
fn link(
    id: impl Into<gpui_kit::ElementId>,
    label: String,
    t: &Tokens,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    div()
        .id(id.into())
        .cursor_pointer()
        .text_color(t.accent)
        .hover(|s| s.underline())
        .child(label)
}

/// A multi-line editor in its box (docs/design/components.md 5), with the
/// footer hint.
fn editor_box(
    editor: &Entity<TextareaState>,
    locale: Locale,
    t: &Tokens,
    cx: &mut Context<RecordPage>,
) -> AnyElement {
    editor_box_with(editor, None, locale, t, cx)
}

/// [`editor_box`] for an editor that takes files (a description, an
/// entry): the paste hook, the strip between the text and the footer, the
/// paperclip left of the save hint, and the drop target.
fn editor_box_with(
    editor: &Entity<TextareaState>,
    tray: Option<&Entity<crate::attach_ui::Tray>>,
    locale: Locale,
    t: &Tokens,
    cx: &mut Context<RecordPage>,
) -> AnyElement {
    let macos = cfg!(target_os = "macos");
    let keys = format!(
        "{}+{}",
        KeyName::Mod.label(locale, macos),
        KeyName::Enter.label(locale, macos)
    );
    let mut textarea = Textarea::new(editor)
        .appearance(false)
        .text_size(px(type_scale::READING.size()))
        .line_height(px(type_scale::READING.line()));
    if let Some(tray) = tray {
        textarea = textarea.on_paste(crate::attach_ui::on_paste(tray));
        // The palette's "Attach files..." finds this editor's tray.
        crate::attach_ui::pair(editor, tray, cx);
    }
    let strip = tray.and_then(|tray| tray.update(cx, |tray, cx| tray.render_strip(cx)));
    let button = tray.map(|tray| crate::attach_ui::attach_button("editor", tray, cx));
    let body = div()
        .flex()
        .flex_col()
        .rounded(px(RADIUS_LG))
        .bg(t.bg_raised)
        .border(px(2.))
        .border_color(t.focus_ring)
        .px(px(16.))
        .py(px(12.))
        .on_action(cx.listener(|page, _: &Escape, window, cx| page.cancel_editors(window, cx)))
        .child(
            type_scale::READING
                .el()
                .mx(px(-10.))
                .my(px(-8.))
                .child(textarea),
        )
        .children(strip.map(|s| div().mt(px(16.)).child(s)))
        .child(
            div()
                .flex()
                .flex_row()
                .justify_end()
                .gap(px(8.))
                .h(px(28.))
                .items_center()
                .children(button)
                // Dictation (components-voice.md 5): the microphone after
                // the paperclip, left of the save hint, which yields while
                // a take shows its status.
                .children(crate::voice::ui::mic_for(editor, cx))
                .when(!crate::voice::ui::listening_for(editor, cx), |this| {
                    this.child(type_scale::SMALL.el().text_color(t.text_tertiary).child(tr(
                        locale,
                        Key::ComposerHint,
                        &[("keys", Arg::Text(&keys))],
                    )))
                }),
        );
    let boxed = match tray {
        Some(tray) => {
            crate::attach_ui::drop_zone("editor", tray, RADIUS_LG, body, cx).into_any_element()
        }
        None => body.into_any_element(),
    };
    // The dictation line sits under the editor's box.
    div()
        .flex()
        .flex_col()
        .child(boxed)
        .children(crate::voice::ui::line_for(editor, cx))
        .into_any_element()
}

impl Render for RecordPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        // The add chip stays while it has the keyboard (the picker closed
        // back onto it), as it does under the pointer.
        let due_focused = self.due_focus.is_focused(window);
        // A figure of the Overview was opened: focus the table's first row
        // once it is drawn.
        if self.focus_first_row && self.data.is_some() {
            self.focus_first_row = false;
            cx.on_next_frame(window, |page, window, cx| {
                if let Some(first) = page.row_focus.first() {
                    window.focus(first, cx);
                }
            });
        }
        // A focus handle for each table row (`D` on a row, focus after a
        // click-through).
        let rows = self.data.as_ref().map_or(0, |d| d.all_topics().len());
        while self.row_focus.len() < rows {
            self.row_focus.push(cx.focus_handle());
        }
        // The blocks `Up` / `Down` step through, in reading order, each
        // with its handle (made here: drawing takes `&self` in places).
        self.folder_handles(cx);
        if let Some(data) = &self.data {
            let shown = record::timeline_entries(&data.entries);
            let folders = data.folders.active().is_some();
            self.block_order = record::blocks(self.topic().is_some(), folders, &shown);
            for block in self.block_order.clone() {
                self.block_handle(block, cx);
            }
            // "Logged as a decision" opened this page: its entry, once shown.
            if let Some(entry) = self.pending_entry
                && self.block_order.contains(&record::Block::Entry(entry))
            {
                self.pending_entry = None;
                cx.on_next_frame(window, move |page, window, cx| {
                    page.focus_block(record::Block::Entry(entry), window, cx)
                });
            }
        }
        let mut column = ui::column();
        if self.gone {
            column = column.child(
                ui::notice(
                    Some((Icon::Info, t.text_secondary)),
                    text(locale, Key::DeletedNotice),
                    &t,
                )
                .child(
                    ui::button(
                        "deleted-restore",
                        text(locale, Key::DeletedRestore),
                        Variant::Ghost,
                        false,
                        &t,
                    )
                    .on_click(cx.listener(|page, _, _, cx| page.restore_record(cx))),
                ),
            );
        }
        if let Some(data) = self.data.clone() {
            column = column
                .child(self.render_title(&data, &t, cx))
                .child(self.render_properties(&data, due_focused, window, locale, &t, cx))
                .children(self.render_claude_line(locale, &t, cx));
            if self.topic().is_some() {
                column = column.child(self.render_state(&data, locale, &t, cx));
            }
            column = column
                .children(self.render_overview(&data, locale, &t, cx))
                .child(self.render_description(&data, locale, &t, cx));
            let questions = self.render_questions(&data, locale, &t, window, cx);
            column = column
                .child(questions)
                .child(self.render_knowledge(&data, locale, &t, cx));
            if self.topic().is_some() {
                column = column.child(self.render_timeline(&data, locale, &t, cx));
            } else {
                column = column.child(self.render_table(&data, locale, &t, cx));
            }
        }
        ui::page_scroll("record-page")
            .track_scroll(&self.scroll)
            // Scrolling away from a title being renamed saves it, as a
            // click elsewhere does (QA 2026-11-22).
            .on_scroll(cx.listener(|page, _, window, cx| {
                if page.title_edit.is_some() {
                    page.commit_title(window, cx);
                }
            }))
            .track_focus(&self.focus)
            .key_context("RecordPage")
            .on_action(cx.listener(|page, _: &OpenStatusMenu, window, cx| {
                if let Some(status) = page
                    .data
                    .as_ref()
                    .and_then(|d| d.topic.as_ref())
                    .map(|t| t.status)
                {
                    page.status_menu =
                        Some(Status::ALL.iter().position(|s| *s == status).unwrap_or(0));
                    window.focus(&page.focus, cx);
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|page, _: &EditTitle, window, cx| {
                // A focused question row edits itself (the focused element
                // wins, components-records.md 5).
                if !page.edit_focused_question(window, cx) {
                    page.edit_title(window, cx)
                }
            }))
            .on_action(cx.listener(|page, _: &OpenDuePicker, window, cx| {
                match page.focused_question(window) {
                    Some((_, id)) => page.open_question_date(id, window, cx),
                    None => page.open_due_here(false, window, cx),
                }
            }))
            .on_action(cx.listener(|page, _: &questions::NewQuestion, window, cx| {
                page.new_question(window, cx)
            }))
            .on_action(
                cx.listener(|page, _: &questions::OpenReportTo, window, cx| {
                    page.open_report_to(window, cx)
                }),
            )
            .on_action(cx.listener(|page, _: &Escape, window, cx| page.cancel_editors(window, cx)))
            .on_key_down(cx.listener(Self::on_key))
            .child(column)
    }
}

/// The 24 band under a properties block (components-records.md 5): part
/// of the block for hover, empty at rest, and taken back from the space
/// above the next section (`mb -24`), so showing the add chips in it,
/// lined up with the values' column, moves nothing.
pub fn add_band(chips: Option<AnyElement>, label_width: f32) -> gpui_kit::Div {
    div()
        .h(px(ADD_BAND))
        .mb(px(-ADD_BAND))
        .pl(px(label_width + 16.))
        .flex()
        .flex_row()
        .items_center()
        .children(chips)
}

/// The height of the add chips' band.
pub const ADD_BAND: f32 = 24.;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deleted_records_are_restored_as_the_same_record() {
        assert_eq!(
            record_of(Owner::Topic(TopicId(4))),
            Record::Topic(TopicId(4))
        );
        assert_eq!(
            record_of(Owner::Epic(acta_core::model::EpicId(2))),
            Record::Epic(acta_core::model::EpicId(2))
        );
    }

    #[test]
    fn decisions_are_the_outlined_kind_and_system_events_read_as_lines() {
        assert_eq!(entry_kind_meta(EntryKind::Decision).1, Key::EntryDecision);
        assert_eq!(
            entry_kind_meta(EntryKind::Progress).0,
            Icon::GitCommitHorizontal
        );
    }
}
