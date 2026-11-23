//! Memory quality on screen (docs/design/components-knowledge.md 5, Memory
//! quality; ADR 0015): what an entry page loads (flags, history, sources,
//! presence), git's flags read off the UI thread, the acts (mark as still
//! true, no longer true, live again, this replaces it, keep both, merge,
//! find duplicates), each one undo step, and the notice, provenance line,
//! Branch and Sources rows and "also open" line shared by the entry page
//! and the unfolded memory.

use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use acta_core::agent::kbq::{Flag, GitFacts, git_facts};
use acta_core::date::Zone;
use acta_core::model::{
    Author, KbEntry, KbId, KbKind, KbLinkKind, KbPairKind, KbPairState, Owner, PageReq, ProjectId,
    Record, TopicId,
};
use acta_core::store::{KbFilter, KbLinkTarget, KbRevisionSummary, KbWriteCtx};

use crate::zoom::px;
use gpui_kit::{
    AnyElement, App, Context, Entity, EventEmitter, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Task, Window, div,
};

use crate::i18n::{Arg, Key, Locale, text, tr};
use crate::icons::Icon;
use crate::kbq_model::{self, Notice, NoticeAction, Provenance, Reason};
use crate::record_page::PageEvent;
use crate::route::Route;
use crate::settings;
use crate::theme::Tokens;
use crate::type_scale;
use crate::ui::{self, Variant};
use crate::when;
use crate::workspace::{Undo, Workspace, WriteError, kb_pair_state};

/// Git's flags of entries, and the default branches of a project's
/// folders.
pub type GitRead = (Vec<(KbId, Vec<Flag>)>, Vec<String>);

/// A source's id or path was copied: the page shows "Copied" on it.
pub type OnCopied = Rc<dyn Fn(usize, &mut App)>;

/// "Also open" looks this far back (ADR 0015, 6).
const ALSO_OPEN_MS: i64 = 10 * 60_000;

/// A source of an entry as its page shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceView {
    /// A topic, question, note or entry of this database: its id, its
    /// title, where a click goes.
    Record {
        id: String,
        title: String,
        route: Option<Route>,
    },
    /// A timeline entry: "{topic}, entry of {date}".
    Entry {
        id: String,
        topic: String,
        at: i64,
        route: Route,
    },
    /// A record this database lacks.
    Missing,
    Commit(String),
    File {
        path: String,
        folder: String,
    },
}

/// Another live Claude Code session touched this entry lately.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlsoOpen {
    pub topic: Option<TopicId>,
    pub at: i64,
    pub wrote: bool,
}

/// Everything the entry page and an unfolded memory need beyond the text.
#[derive(Clone, Debug)]
pub struct EntryQuality {
    pub entry: KbEntry,
    pub history: Vec<KbRevisionSummary>,
    pub store_flags: Vec<(KbId, Vec<Flag>)>,
    pub sources: Vec<SourceView>,
    pub successor: Option<String>,
    /// The other entry of each pair: title and summary.
    pub others: HashMap<KbId, (String, String)>,
    /// Where a closed topic's entry moves up to, and its name.
    pub above: Option<(Owner, String)>,
    pub facts: Option<GitFacts>,
    pub also_open: Option<AlsoOpen>,
    /// Live sessions' topics, by session id (the history's tooltip).
    pub sessions: HashMap<String, TopicId>,
    pub topic_titles: HashMap<TopicId, String>,
    /// The folder a branch memory belongs to, by name.
    pub branch_folder: Option<String>,
    /// The project's folders (their default branches are not shown).
    pub dirs: Vec<PathBuf>,
}

/// Store flags as [`Flag`]s.
pub fn store_flags_of(
    store: &acta_core::Store,
    ids: &[KbId],
    today: acta_core::date::Date,
) -> acta_core::Result<Vec<(KbId, Vec<Flag>)>> {
    Ok(store
        .kb_store_flags(ids, Zone::system(), today)?
        .into_iter()
        .map(|(id, f)| (id, f.into_iter().map(Flag::Store).collect()))
        .collect())
}

/// Git's flags of entries, and the default branches of `dirs`, read on a
/// background thread with the workspace's cache (never on the UI thread,
/// never holding the store).
pub fn read_git(
    ws: &Entity<Workspace>,
    facts: Vec<GitFacts>,
    dirs: Vec<PathBuf>,
    cx: &App,
) -> Task<GitRead> {
    let cache = ws.read(cx).kb_git.clone();
    cx.background_executor().spawn(async move {
        let mut flags = Vec::with_capacity(facts.len());
        if let Ok(mut cache) = cache.lock() {
            for f in &facts {
                flags.push((f.id, cache.flags(f)));
            }
        }
        let defaults = dirs
            .iter()
            .filter_map(|d| acta_core::agent::git::inspect(d))
            .filter_map(|i| i.default_branch)
            .collect();
        (flags, defaults)
    })
}

/// Git's flags of `ids` (a list): their facts from the store, then git.
pub fn list_git(ws: &Entity<Workspace>, ids: Vec<KbId>, cx: &App) -> Task<Vec<(KbId, Vec<Flag>)>> {
    let facts = ws.read(cx).db.run(cx, move |store| {
        Ok(ids
            .iter()
            .filter_map(|id| git_facts(store, *id))
            .collect::<Vec<_>>())
    });
    let cx_bg = cx.background_executor().clone();
    let cache = ws.read(cx).kb_git.clone();
    cx_bg.clone().spawn(async move {
        let Ok(facts) = facts.await else {
            return Vec::new();
        };
        if facts.is_empty() {
            return Vec::new();
        }
        cx_bg
            .spawn(async move {
                let mut out = Vec::with_capacity(facts.len());
                if let Ok(mut cache) = cache.lock() {
                    for f in &facts {
                        out.push((f.id, cache.flags(f)));
                    }
                }
                out
            })
            .await
    })
}

/// What an entry page shows beyond the text, read in one storage task.
pub fn load_entry(
    ws: &Entity<Workspace>,
    id: KbId,
    cx: &App,
) -> Task<acta_core::Result<EntryQuality>> {
    let today = crate::due::today(cx);
    ws.read(cx).db.run(cx, move |store| {
        let entry = store.kb_entry(id)?;
        let history = store
            .kb_history(id, PageReq::first(PageReq::MAX_LIMIT))?
            .items;
        let store_flags = store_flags_of(store, &[id], today)?;
        let folder_name = |f| {
            store
                .folder(f)
                .map(|f| f.display_name().to_owned())
                .unwrap_or_default()
        };
        let mut topic_titles = HashMap::new();
        let mut sources = Vec::new();
        for link in store.kb_links(id)? {
            let view = match (link.kind, &link.target) {
                (_, KbLinkTarget::Missing) => SourceView::Missing,
                (KbLinkKind::Commit, KbLinkTarget::Text(sha)) => SourceView::Commit(sha.clone()),
                (KbLinkKind::File, KbLinkTarget::Text(path)) => SourceView::File {
                    path: path.clone(),
                    folder: link.folder.map(folder_name).unwrap_or_default(),
                },
                (_, KbLinkTarget::Text(text)) => SourceView::Record {
                    id: String::new(),
                    title: text.clone(),
                    route: None,
                },
                (_, KbLinkTarget::Record(record)) => record_view(store, *record),
            };
            sources.push(view);
        }
        let successor = entry
            .superseded_by
            .and_then(|s| store.kb_entry(s).ok())
            .map(|e| e.title);
        let mut others = HashMap::new();
        for pair in store.kb_pairs_of(id)? {
            let other = pair.other(id);
            if let Ok(o) = store.kb_entry(other) {
                others.insert(other, (o.title, o.summary));
            }
        }
        let above = match entry.owner {
            Owner::Topic(t) => {
                let topic = store.topic(t)?;
                match topic.epic {
                    Some(e) => Some((Owner::Epic(e), store.epic(e)?.title)),
                    None => Some((
                        Owner::Project(topic.project),
                        store.project(topic.project)?.title,
                    )),
                }
            }
            _ => None,
        };
        let facts = git_facts(store, id);
        let now = acta_core::time::now_ms();
        let mut sessions = HashMap::new();
        let mut also_open = None;
        if let Ok(mut presence) = store.presence() {
            for s in presence.live(now).unwrap_or_default() {
                if let Some(Owner::Topic(t)) = s.focus {
                    sessions.insert(s.id, t);
                }
            }
            if let Some((session, focus, at)) = presence
                .kb_touches_elsewhere(None, id, now - ALSO_OPEN_MS, now)
                .unwrap_or_default()
                .into_iter()
                .next()
            {
                let wrote = history.iter().any(|h| {
                    h.session.as_deref() == Some(session.as_str())
                        && h.created_at >= at - ALSO_OPEN_MS
                });
                also_open = Some(AlsoOpen {
                    topic: match focus {
                        Some(Owner::Topic(t)) => Some(t),
                        _ => None,
                    },
                    at,
                    wrote,
                });
            }
        }
        let mut want: Vec<TopicId> = sessions.values().copied().collect();
        want.extend(entry.created_topic);
        want.extend(also_open.as_ref().and_then(|a| a.topic));
        for t in want {
            if let Ok(topic) = store.topic(t) {
                topic_titles.insert(t, topic.title);
            }
        }
        let branch_folder = entry
            .context_branch
            .as_ref()
            .and(entry.context_folder)
            .map(folder_name);
        let dirs = store
            .all_folders()?
            .into_iter()
            .filter(|f| f.project == entry.project)
            .map(|f| PathBuf::from(f.path))
            .collect();
        Ok(EntryQuality {
            entry,
            history,
            store_flags,
            sources,
            successor,
            others,
            above,
            facts,
            also_open,
            sessions,
            topic_titles,
            branch_folder,
            dirs,
        })
    })
}

fn record_view(store: &acta_core::Store, record: Record) -> SourceView {
    let missing = || SourceView::Missing;
    match record {
        Record::Topic(t) => match store.topic(t) {
            Ok(topic) => SourceView::Record {
                id: t.to_string(),
                title: topic.title,
                route: Some(Route::Topic(t)),
            },
            Err(_) => missing(),
        },
        Record::Entry(e) => match store.entry(e) {
            Ok(entry) => SourceView::Entry {
                id: match entry.seq {
                    Some(n) => format!("{}#{n}", entry.topic),
                    None => entry.topic.to_string(),
                },
                topic: store
                    .topic(entry.topic)
                    .map(|t| t.title)
                    .unwrap_or_default(),
                at: entry.stamp.created_at,
                route: Route::Topic(entry.topic),
            },
            Err(_) => missing(),
        },
        Record::Question(q) => match store.question(q) {
            Ok(question) => SourceView::Record {
                id: q.to_string(),
                title: question.text,
                route: Some(Route::Questions(question.owner)),
            },
            Err(_) => missing(),
        },
        Record::Note(n) => match store.note(n) {
            Ok(note) => SourceView::Record {
                id: n.to_string(),
                title: note.text.lines().next().unwrap_or_default().to_owned(),
                route: Some(Route::Inbox(note.project)),
            },
            Err(_) => missing(),
        },
        Record::Kb(k) => match store.kb_entry(k) {
            Ok(e) => SourceView::Record {
                id: k.to_string(),
                title: e.title,
                route: Some(Route::Kb(k)),
            },
            Err(_) => missing(),
        },
        _ => missing(),
    }
}

// ---- the acts, each one undo step ----

/// The undo stack of an entry: its project's.
fn stack(ws: &Entity<Workspace>, owner: Option<Owner>, cx: &App) -> Option<ProjectId> {
    owner.and_then(|o| ws.read(cx).project_of(o))
}

/// "Mark as still true" (`V`): the file sources' blob ids at `HEAD` read
/// off the UI thread, then one write; no toast (the mark going is the
/// feedback). The caller hides the mark at once.
pub fn verify(ws: &Entity<Workspace>, id: KbId, owner: Option<Owner>, cx: &mut App) {
    let project = stack(ws, owner, cx);
    let cache = ws.read(cx).kb_git.clone();
    let read = ws.read(cx).db.run(cx, move |store| {
        Ok((store.kb_entry(id)?.rev, git_facts(store, id)))
    });
    let ws = ws.clone();
    cx.spawn(async move |cx| {
        let Ok((rev, facts)) = read.await else {
            return;
        };
        let blobs = match facts {
            Some(f) => {
                cx.background_executor()
                    .spawn(async move {
                        cache
                            .lock()
                            .map(|mut c| c.verify_blobs(&f))
                            .unwrap_or_default()
                    })
                    .await
            }
            None => Vec::new(),
        };
        ws.update(cx, |ws, cx| {
            ws.undoable(cx, project, None, move |store| {
                let trace = store.kb_verify(
                    id,
                    Some(rev),
                    &blobs,
                    &KbWriteCtx::default(),
                    Author::Human,
                )?;
                Ok((Undo::KbVerify(Box::new(trace)), ()))
            });
        });
    })
    .detach();
}

/// "Mark as no longer true": no reason asked; it leaves the live list,
/// often away from the reader's eyes, so it toasts with Undo.
pub fn retire(
    ws: &Entity<Workspace>,
    id: KbId,
    rev: Option<i64>,
    kind: KbKind,
    owner: Option<Owner>,
    cx: &mut App,
) {
    let project = stack(ws, owner, cx);
    let locale = settings::locale(cx);
    let message = text(
        locale,
        kbq_model::by_kind(kind, Key::KbRetiredToastDocument, Key::KbRetiredToastMemory),
    );
    ws.update(cx, |ws, cx| {
        ws.undoable(cx, project, Some(message), move |store| {
            // The revision the page showed: a text changed since is not
            // retired unseen (refused like a stale edit).
            let trace = store.kb_retire(id, rev, "", &KbWriteCtx::default(), Author::Human)?;
            Ok((Undo::KbState(Box::new(trace)), ()))
        });
    });
}

/// "Make live again": no toast (the notice going is the feedback).
pub fn make_live(ws: &Entity<Workspace>, id: KbId, owner: Option<Owner>, cx: &mut App) {
    let project = stack(ws, owner, cx);
    ws.update(cx, |ws, cx| {
        ws.undoable(cx, project, None, move |store| {
            let trace = store.kb_make_live(id, Author::Human)?;
            Ok((Undo::KbState(Box::new(trace)), ()))
        });
    });
}

/// "This replaces it": `this` supersedes `old`; the other entry changes
/// off screen, so it toasts with Undo.
pub fn replace(ws: &Entity<Workspace>, this: KbId, old: KbId, owner: Option<Owner>, cx: &mut App) {
    let project = stack(ws, owner, cx);
    let locale = settings::locale(cx);
    let message = tr(
        locale,
        Key::KbReplacedToast,
        &[("id", Arg::Text(&old.to_string()))],
    );
    ws.update(cx, |ws, cx| {
        ws.undoable(cx, project, Some(message), move |store| {
            let mut traces = store.kb_replace_with(this, &[old], Author::Human)?;
            let trace = traces
                .pop()
                .ok_or_else(|| acta_core::Error::Invalid(format!("{old} not superseded")))?;
            Ok((Undo::KbState(Box::new(trace)), ()))
        });
    });
}

/// "Keep both": the pair is distinct and never warns again; no toast.
pub fn keep_both(
    ws: &Entity<Workspace>,
    this: KbId,
    other: KbId,
    kind: KbPairKind,
    owner: Option<Owner>,
    cx: &mut App,
) {
    let project = stack(ws, owner, cx);
    ws.update(cx, |ws, cx| {
        ws.undoable(cx, project, None, move |store| {
            let back = kb_pair_state(store, this, other, kind)?;
            store.kb_set_pair_state(this, other, kind, KbPairState::Distinct, Author::Human)?;
            Ok((
                Undo::KbPair {
                    a: this,
                    b: other,
                    kind,
                    back,
                    expect: KbPairState::Distinct,
                },
                (),
            ))
        });
    });
}

/// Merge `other` into `survivor` (one step, "Merged into K-4." with
/// Undo). The caller opens the survivor in its editor once it is done.
pub fn merge(
    ws: &Entity<Workspace>,
    survivor: KbId,
    other: KbId,
    owner: Option<Owner>,
    cx: &mut App,
) -> Task<Result<(), WriteError>> {
    let project = stack(ws, owner, cx);
    let locale = settings::locale(cx);
    let message = tr(
        locale,
        Key::KbMerged,
        &[("id", Arg::Text(&survivor.to_string()))],
    );
    ws.update(cx, |ws, cx| {
        ws.write_undoable(cx, project, Some(message), move |store| {
            let trace = store.kb_merge(survivor, other, Author::Human)?;
            Ok((Undo::KbMerge(Box::new(trace)), ()))
        })
    })
}

/// "Move to epic {name}" from a closed topic's notice: one step, toasted.
pub fn move_up(
    ws: &Entity<Workspace>,
    id: KbId,
    kind: KbKind,
    to: Owner,
    name: &str,
    cx: &mut App,
) {
    let project = stack(ws, Some(to), cx);
    let locale = settings::locale(cx);
    let key = match (to, kind) {
        (Owner::Project(_), KbKind::Document) => Key::KbMovedProjectDocument,
        (Owner::Project(_), KbKind::Memory) => Key::KbMovedProjectMemory,
        (_, KbKind::Document) => Key::KbMovedEpicDocument,
        (_, KbKind::Memory) => Key::KbMovedEpicMemory,
    };
    let message = tr(locale, key, &[("name", Arg::Text(name))]);
    ws.update(cx, |ws, cx| {
        ws.undoable(cx, project, Some(message), move |store| {
            let back = store.kb_entry(id)?.owner;
            store.kb_move(id, to, Author::Human)?;
            store.kb_recheck(id, Author::Human)?;
            Ok((
                Undo::KbOwner {
                    id,
                    back,
                    expect: to,
                },
                (),
            ))
        });
    });
}

/// "Find duplicate memories": the scan off the UI thread, one toast at
/// the end ("Found 3 possible duplicates." with Show, or none found).
/// `running` is set while it runs (the command is not listed then).
pub fn find_duplicates(ws: &Entity<Workspace>, project: ProjectId, cx: &mut App) {
    if ws.read(cx).kb_scanning {
        return;
    }
    ws.update(cx, |ws, _| ws.kb_scanning = true);
    let ws = ws.clone();
    cx.spawn(async move |cx| {
        // Page by page: the store is free for other writes between pages.
        let mut pairs = std::collections::HashSet::new();
        let mut offset = Some(0);
        let mut found = Ok(());
        while let Some(at) = offset {
            let task = ws.update(cx, |ws, cx| {
                ws.db.run(cx, move |store| {
                    store.kb_scan_duplicates_page(
                        project,
                        at,
                        acta_core::store::SCAN_PAGE,
                        Author::Human,
                    )
                })
            });
            match task.await {
                Ok((page, next)) => {
                    pairs.extend(page);
                    offset = next;
                }
                Err(err) => {
                    found = Err(err);
                    break;
                }
            }
        }
        let found = found.map(|()| pairs.len());
        ws.update(cx, |ws, cx| {
            ws.kb_scanning = false;
            let locale = settings::locale(cx);
            match found {
                Ok(0) => ws.toast(
                    text(locale, Key::KbScanNone),
                    crate::workspace::ToastKind::Done,
                    cx,
                ),
                Ok(n) => {
                    let message = tr(locale, Key::KbScanFound, &[("n", Arg::Count(n as u64))]);
                    ws.toast_with(
                        message,
                        crate::workspace::ToastKind::Done,
                        Some(crate::workspace::ToastAction::ShowToCheck(project)),
                        cx,
                    );
                    ws.reload(cx);
                }
                Err(_) => ws.toast_error(Key::ErrorSave, cx),
            }
        });
    })
    .detach();
}

/// The live memories sharing a scope with `id` (its level, the levels
/// above, the levels below): id, title, summary.
pub fn merge_candidates(
    ws: &Entity<Workspace>,
    id: KbId,
    cx: &App,
) -> Task<acta_core::Result<Vec<(KbId, String, String)>>> {
    let tree = ws.read(cx).projects.clone();
    ws.read(cx).db.run(cx, move |store| {
        let me = store.kb_entry(id)?;
        let epic_of = |t: TopicId| {
            tree.iter()
                .flat_map(|p| p.epics.iter())
                .find(|(_, ts)| ts.iter().any(|x| x.id == t))
                .map(|(e, _)| e.id)
        };
        let all = store.kb_rollup(
            Owner::Project(me.project),
            &KbFilter {
                kind: Some(KbKind::Memory),
                ..KbFilter::default()
            },
            PageReq::first(PageReq::MAX_LIMIT),
        )?;
        Ok(all
            .items
            .into_iter()
            .filter(|k| k.id != id && kbq_model::shares_scope(me.owner, k.owner, epic_of))
            .map(|k| (k.id, k.title, k.summary))
            .collect())
    })
}

/// What happens once a merge is written: the survivor opens in its editor.
pub type OnMerged = Rc<dyn Fn(KbId, &mut Window, &mut App)>;

/// "Merge into...": the palette as a picker of `candidates`; `Enter`
/// merges `id` into the chosen one, which survives whatever the ids.
pub fn merge_picker(
    locale: Locale,
    ws: &Entity<Workspace>,
    id: KbId,
    owner: Option<Owner>,
    candidates: Vec<(KbId, String, String)>,
    on_merged: OnMerged,
) -> (crate::palette::Picker, crate::palette::OnPick) {
    let choices = candidates
        .iter()
        .map(|(_, title, summary)| crate::palette::Choice {
            title: title.clone(),
            detail: summary.clone(),
            lead: crate::palette::Lead::Icon(Icon::Pin),
            trailing: String::new(),
            refusal: None,
        })
        .collect();
    let ws = ws.clone();
    let on_pick = crate::palette::OnPick::new(move |i, window, cx| {
        let Some((survivor, _, _)) = candidates.get(i).cloned() else {
            return;
        };
        let task = merge(&ws, survivor, id, owner, cx);
        let on_merged = on_merged.clone();
        let handle = window.window_handle();
        cx.spawn(async move |cx| {
            if task.await.is_ok() {
                let _ = handle.update(cx, |_, window, cx| on_merged(survivor, window, cx));
            }
        })
        .detach();
    });
    (
        crate::palette::Picker {
            label: text(locale, Key::KbMergeInto),
            choices,
        },
        on_pick,
    )
}

// ---- shared pieces of the entry page and the unfolded memory ----

/// The tooltip of a flagged row: its reasons, one per line.
pub fn reason_lines(reasons: &[Reason], locale: Locale) -> Vec<String> {
    reasons.iter().map(|r| r.line(locale)).collect()
}

/// The facts the notice of an entry needs.
#[derive(Clone, Debug)]
pub struct NoticeFacts {
    pub id: KbId,
    /// The revision the page shows.
    pub rev: i64,
    pub kind: KbKind,
    pub owner: Owner,
    pub notice: Notice,
    pub successor: Option<String>,
    pub others: HashMap<KbId, (String, String)>,
    pub above_name: Option<String>,
}

impl NoticeFacts {
    /// The notice of a loaded entry with these reasons, if one applies.
    pub fn of(q: &EntryQuality, reasons: &[Reason]) -> Option<Self> {
        let notice = kbq_model::notice(
            &q.entry,
            &q.history,
            reasons,
            q.above.as_ref().map(|(o, _)| *o),
        )?;
        Some(NoticeFacts {
            id: q.entry.id,
            rev: q.entry.rev,
            kind: q.entry.kind,
            owner: q.entry.owner,
            notice,
            successor: q.successor.clone(),
            others: q.others.clone(),
            above_name: q.above.as_ref().map(|(_, n)| n.clone()),
        })
    }
}

/// An id or a title as a link (`accent`), opening `route`.
fn link<T: EventEmitter<PageEvent>>(
    id: impl Into<gpui_kit::ElementId>,
    label: String,
    tip: String,
    route: Route,
    t: &Tokens,
    cx: &mut Context<T>,
) -> AnyElement {
    div()
        .id(id)
        .flex_shrink_0()
        .cursor_pointer()
        .text_color(t.accent)
        .hover(|s| s.underline())
        .tooltip(ui::tip(tip))
        .on_click(cx.listener(move |_, _, _, cx| cx.emit(PageEvent::Open(route))))
        .child(label)
        .into_any_element()
}

/// A message with one link inside it: the text before `{slot}`, the link,
/// the text after (the catalogue string stays whole; only the value is a
/// control).
fn with_link(message: &str, marker: &str, link: AnyElement) -> gpui_kit::Div {
    let (before, after) = message.split_once(marker).unwrap_or((message, ""));
    div()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .child(before.to_owned())
        .child(link)
        .child(after.to_owned())
}

/// The notice of an entry (components-knowledge.md 5, Notice): one at a
/// time, its ghost actions right-aligned. `on_merge` runs once a merge is
/// written (the survivor opens in its editor).
pub fn notice_el<T: EventEmitter<PageEvent> + 'static>(
    f: &NoticeFacts,
    ws: &Entity<Workspace>,
    on_merged: OnMerged,
    locale: Locale,
    t: &Tokens,
    cx: &mut Context<T>,
) -> AnyElement {
    const MARK: &str = "\u{1}";
    let id = f.id;
    let rev = f.rev;
    let owner = Some(f.owner);
    let kind = f.kind;
    let button = |key: &'static str, label: String| {
        ui::button(key, label, Variant::Ghost, false, t).flex_shrink_0()
    };
    let other_tip = |other: KbId| {
        f.others
            .get(&other)
            .map(|(title, summary)| format!("{title}\n{summary}"))
            .unwrap_or_default()
    };
    let info = Some((Icon::Info, t.text_secondary));
    let message = |el: gpui_kit::Div| type_scale::UI.el().text_color(t.text_primary).child(el);
    let shell = |glyph: Option<(Icon, gpui_kit::Hsla)>| ui::notice_frame(glyph, t);
    match &f.notice {
        Notice::Superseded { by, at } => {
            let key = kbq_model::by_kind(
                kind,
                Key::KbNoticeSupersededDocument,
                Key::KbNoticeSupersededMemory,
            );
            let date = when::date(locale, *at);
            let text = tr(
                locale,
                key,
                &[("title", Arg::Text(MARK)), ("date", Arg::Text(&date))],
            );
            let title = f.successor.clone().unwrap_or_else(|| by.to_string());
            let l = link(
                "kbq-successor",
                title,
                by.to_string(),
                Route::Kb(*by),
                t,
                cx,
            );
            let ws = ws.clone();
            shell(info)
                .child(
                    message(with_link(&text, MARK, l))
                        .flex_1()
                        .min_w_0()
                        .py(px(4.)),
                )
                .child(
                    button("kbq-make-live", text_of(locale, Key::KbMakeLive))
                        .on_click(move |_, _, cx| make_live(&ws, id, owner, cx)),
                )
                .into_any_element()
        }
        Notice::Retired { by, at, reason } => {
            let key = kbq_model::by_kind(
                kind,
                Key::KbNoticeRetiredDocument,
                Key::KbNoticeRetiredMemory,
            );
            let line = tr(
                locale,
                key,
                &[
                    ("author", Arg::Text(&when::author(locale, *by))),
                    ("date", Arg::Text(&when::date(locale, *at))),
                ],
            );
            let ws = ws.clone();
            shell(info)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .py(px(4.))
                        .flex()
                        .flex_col()
                        .child(type_scale::UI.el().text_color(t.text_primary).child(line))
                        .children(
                            reason
                                .clone()
                                .map(|r| type_scale::UI.el().text_color(t.text_secondary).child(r)),
                        ),
                )
                .child(
                    button("kbq-make-live", text_of(locale, Key::KbMakeLive))
                        .on_click(move |_, _, cx| make_live(&ws, id, owner, cx)),
                )
                .into_any_element()
        }
        Notice::Conflict(other) => {
            let text = tr(locale, Key::KbNoticeConflict, &[("id", Arg::Text(MARK))]);
            let l = link(
                "kbq-other",
                other.to_string(),
                other_tip(*other),
                Route::Kb(*other),
                t,
                cx,
            );
            let other = *other;
            let (ws1, ws2) = (ws.clone(), ws.clone());
            shell(Some((Icon::TriangleAlert, t.warning)))
                .child(
                    message(with_link(&text, MARK, l))
                        .flex_1()
                        .min_w_0()
                        .py(px(4.)),
                )
                .child(
                    button("kbq-replace", text_of(locale, Key::KbThisReplaces))
                        .on_click(move |_, _, cx| replace(&ws1, id, other, owner, cx)),
                )
                .child(
                    button("kbq-keep", text_of(locale, Key::ContactKeepBoth)).on_click(
                        move |_, _, cx| keep_both(&ws2, id, other, KbPairKind::Conflict, owner, cx),
                    ),
                )
                .into_any_element()
        }
        Notice::Duplicate(other) => {
            let text = tr(locale, Key::KbNoticeDuplicate, &[("id", Arg::Text(MARK))]);
            let l = link(
                "kbq-other",
                other.to_string(),
                other_tip(*other),
                Route::Kb(*other),
                t,
                cx,
            );
            let other = *other;
            let (survivor, merged) = kbq_model::merge_direction(id, other);
            let (ws1, ws2) = (ws.clone(), ws.clone());
            shell(info)
                .child(
                    message(with_link(&text, MARK, l))
                        .flex_1()
                        .min_w_0()
                        .py(px(4.)),
                )
                .child(
                    button("kbq-merge", kbq_model::merge_label(locale, id, other)).on_click(
                        move |_, window, cx| {
                            let task = merge(&ws1, survivor, merged, owner, cx);
                            let on_merged = on_merged.clone();
                            let handle = window.window_handle();
                            cx.spawn(async move |cx| {
                                if task.await.is_ok() {
                                    let _ = handle.update(cx, |_, window, cx| {
                                        on_merged(survivor, window, cx)
                                    });
                                }
                            })
                            .detach();
                        },
                    ),
                )
                .child(
                    button("kbq-keep", text_of(locale, Key::ContactKeepBoth)).on_click(
                        move |_, _, cx| {
                            keep_both(&ws2, id, other, KbPairKind::Duplicate, owner, cx)
                        },
                    ),
                )
                .into_any_element()
        }
        Notice::Reasons(reasons, action) => {
            let ws = ws.clone();
            let action_el = match action {
                NoticeAction::Verify => button("kbq-verify", text_of(locale, Key::KbVerify))
                    .on_click(move |_, _, cx| verify(&ws, id, owner, cx)),
                NoticeAction::MoveUp(to) => {
                    let to = *to;
                    let name = f.above_name.clone().unwrap_or_default();
                    let key = match to {
                        Owner::Project(_) => Key::KbMoveProject,
                        _ => Key::KbMoveEpic,
                    };
                    let label = tr(locale, key, &[("name", Arg::Text(&name))]);
                    button("kbq-move-up", label)
                        .on_click(move |_, _, cx| move_up(&ws, id, kind, to, &name, cx))
                }
                NoticeAction::Retire => button("kbq-retire", text_of(locale, Key::KbRetire))
                    .on_click(move |_, _, cx| retire(&ws, id, Some(rev), kind, owner, cx)),
            };
            shell(info)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .py(px(4.))
                        .flex()
                        .flex_col()
                        .children(reasons.iter().map(|r| {
                            type_scale::UI
                                .el()
                                .text_color(t.text_primary)
                                .child(r.line(locale))
                        })),
                )
                .child(action_el)
                .into_any_element()
        }
    }
}

fn text_of(locale: Locale, key: Key) -> String {
    text(locale, key)
}

/// The provenance line under the meta line: `type.small` `text.tertiary`,
/// segments joined by " · ", each a whole catalogue string.
pub fn provenance_el<T: EventEmitter<PageEvent> + 'static>(
    q: &EntryQuality,
    defaults: &[String],
    locale: Locale,
    t: &Tokens,
    cx: &mut Context<T>,
) -> Option<AnyElement> {
    let segments = kbq_model::provenance(&q.entry, &q.history, defaults);
    if segments.is_empty() {
        return None;
    }
    let kind = q.entry.kind;
    let mut row = type_scale::SMALL
        .el()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .mt(px(4.))
        .min_h(px(20.))
        .text_color(t.text_tertiary);
    for (i, seg) in segments.into_iter().enumerate() {
        if i > 0 {
            row = row.child("\u{a0}\u{b7}\u{a0}");
        }
        let el: AnyElement = match seg {
            Provenance::Written { by, at } => {
                let line = tr(
                    locale,
                    kbq_model::by_kind(kind, Key::KbWrittenDocument, Key::KbWrittenMemory),
                    &[
                        ("author", Arg::Text(&when::author(locale, by))),
                        ("when", Arg::Text(&when::long(locale, at))),
                    ],
                );
                let mut tip = Vec::new();
                if by == Author::Claude {
                    tip.push(session_line(q.entry.created_topic, locale));
                }
                tip.push(when::datetime(locale, at));
                div()
                    .id("kbq-written")
                    .tooltip(ui::tip(tip.join("\n")))
                    .child(line)
                    .into_any_element()
            }
            Provenance::From(topic) => {
                let text = tr(locale, Key::KbProvFrom, &[("id", Arg::Text("\u{1}"))]);
                let title = q.topic_titles.get(&topic).cloned().unwrap_or_default();
                let l = link(
                    "kbq-from",
                    topic.to_string(),
                    title,
                    Route::Topic(topic),
                    t,
                    cx,
                );
                with_link(&text, "\u{1}", l).into_any_element()
            }
            Provenance::Branch(b) => {
                let text = tr(locale, Key::KbProvBranch, &[("branch", Arg::Text("\u{1}"))]);
                let code = type_scale::CODE_SMALL.el().child(b).into_any_element();
                with_link(&text, "\u{1}", code).into_any_element()
            }
            Provenance::Checked { by, at } => tr(
                locale,
                kbq_model::by_kind(kind, Key::KbProvCheckedDocument, Key::KbProvCheckedMemory),
                &[
                    ("when", Arg::Text(&when::long(locale, at))),
                    ("author", Arg::Text(&when::author(locale, by))),
                ],
            )
            .into_any_element(),
        };
        row = row.child(el);
    }
    Some(row.into_any_element())
}

/// "A Claude Code session on T-12" (or any session without a topic).
pub fn session_line(topic: Option<TopicId>, locale: Locale) -> String {
    match topic {
        Some(t) => tr(
            locale,
            Key::KbSessionOn,
            &[("topic", Arg::Text(&t.to_string()))],
        ),
        None => text(locale, Key::KbSessionAny),
    }
}

/// "Claude Code is working with this memory on T-51", while another live
/// session touched it in the last 10 minutes; no action.
pub fn also_open_el(q: &EntryQuality, locale: Locale, t: &Tokens) -> Option<AnyElement> {
    let a = q.also_open.as_ref()?;
    let kind = q.entry.kind;
    let line = match (a.topic, kind) {
        (Some(topic), _) => tr(
            locale,
            kbq_model::by_kind(kind, Key::KbAlsoOpenDocument, Key::KbAlsoOpenMemory),
            &[("topic", Arg::Text(&topic.to_string()))],
        ),
        (None, _) => text(
            locale,
            kbq_model::by_kind(kind, Key::KbAlsoOpenDocumentAny, Key::KbAlsoOpenMemoryAny),
        ),
    };
    let ago = when::long(locale, a.at);
    let tip = tr(
        locale,
        if a.wrote {
            Key::KbAlsoOpenWrote
        } else {
            Key::KbAlsoOpenRead
        },
        &[("when", Arg::Text(&ago))],
    );
    Some(
        div()
            .id("kbq-also-open")
            .mt(px(4.))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .min_h(px(20.))
            .tooltip(ui::tip(tip))
            .child(crate::icons::icon(Icon::Bot, 14., t.text_secondary))
            .child(
                type_scale::SMALL
                    .el()
                    .text_color(t.text_secondary)
                    .child(line),
            )
            .into_any_element(),
    )
}

/// The Branch row's value: the branch, then "merged" or "no longer exists".
pub fn branch_value(
    q: &EntryQuality,
    reasons: &[Reason],
    locale: Locale,
    t: &Tokens,
) -> Option<AnyElement> {
    let branch = q.entry.context_branch.clone()?;
    let state = reasons.iter().find_map(|r| match r {
        Reason::BranchMerged(_) => Some(Key::KbBranchMerged),
        Reason::BranchGone(_) => Some(Key::KbBranchGone),
        _ => None,
    });
    let tip = tr(
        locale,
        Key::KbBranchTip,
        &[(
            "folder",
            Arg::Text(q.branch_folder.as_deref().unwrap_or("")),
        )],
    );
    Some(
        div()
            .id("kbq-branch")
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .tooltip(ui::tip(tip))
            .child(
                type_scale::CODE_SMALL
                    .el()
                    .text_color(t.text_primary)
                    .child(branch),
            )
            .children(state.map(|k| {
                type_scale::SMALL
                    .el()
                    .text_color(t.text_secondary)
                    .child(text(locale, k))
            }))
            .into_any_element(),
    )
}

/// The Sources rows (up to three, 28 each, in the order given).
pub fn sources_el<T: EventEmitter<PageEvent> + 'static>(
    q: &EntryQuality,
    reasons_git: &[Flag],
    copied: Option<usize>,
    on_copied: OnCopied,
    locale: Locale,
    t: &Tokens,
    cx: &mut Context<T>,
) -> Option<AnyElement> {
    if q.sources.is_empty() {
        return None;
    }
    let mut col = div().flex().flex_col().min_w_0();
    for (i, source) in q.sources.iter().take(3).enumerate() {
        let row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .min_h(px(28.))
            .min_w_0();
        let el = match source {
            SourceView::Record { id, title, route } => {
                let title_el: AnyElement = match route {
                    Some(r) => link(("kbq-src", i), title.clone(), title.clone(), *r, t, cx),
                    None => div().child(title.clone()).into_any_element(),
                };
                row.child(
                    type_scale::CODE_SMALL
                        .el()
                        .flex_shrink_0()
                        .text_color(t.text_tertiary)
                        .child(id.clone()),
                )
                .child(div().min_w_0().truncate().child(title_el))
            }
            SourceView::Entry {
                id,
                topic,
                at,
                route,
            } => {
                let label = tr(
                    locale,
                    Key::KbSourceEntry,
                    &[
                        ("topic", Arg::Text(topic)),
                        ("date", Arg::Text(&when::date(locale, *at))),
                    ],
                );
                row.child(
                    type_scale::CODE_SMALL
                        .el()
                        .flex_shrink_0()
                        .text_color(t.text_tertiary)
                        .child(id.clone()),
                )
                .child(div().min_w_0().truncate().child(link(
                    ("kbq-src", i),
                    label.clone(),
                    label,
                    *route,
                    t,
                    cx,
                )))
            }
            SourceView::Missing => row.child(
                type_scale::UI
                    .el()
                    .text_color(t.text_tertiary)
                    .child(text(locale, Key::KbSourceMissing)),
            ),
            SourceView::Commit(sha) => {
                let short: String = sha.chars().take(7).collect();
                let label = tr(locale, Key::KbSourceCommit, &[("sha", Arg::Text("\u{1}"))]);
                let full = sha.clone();
                let on_copied = on_copied.clone();
                let shown: AnyElement = if copied == Some(i) {
                    text(locale, Key::CommonCopied).into_any_element()
                } else {
                    with_link(
                        &label,
                        "\u{1}",
                        type_scale::CODE_SMALL.el().child(short).into_any_element(),
                    )
                    .into_any_element()
                };
                row.child(
                    ui::focusable(div().id(("kbq-commit", i)), t, false)
                        .px(px(4.))
                        .mx(px(-4.))
                        .rounded(px(crate::theme::RADIUS_XS))
                        .cursor_pointer()
                        .hover({
                            let hover = t.fill_hover;
                            move |s| s.bg(hover)
                        })
                        .tooltip(ui::tip(text(locale, Key::KbSourceCopyCommit)))
                        .on_click(move |_, _, cx| {
                            cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(
                                full.clone(),
                            ));
                            on_copied(i, cx);
                        })
                        .child(shown),
                )
            }
            SourceView::File { path, folder } => {
                let state = reasons_git.iter().find_map(|f| match f {
                    Flag::FileChanged(p) if p == path => Some(Key::KbSourceChanged),
                    Flag::FileGone(p) if p == path => Some(Key::KbSourceGone),
                    _ => None,
                });
                let full = path.clone();
                let on_copied = on_copied.clone();
                let tip = tr(
                    locale,
                    Key::KbSourceCopyPath,
                    &[("folder", Arg::Text(folder))],
                );
                let shown: AnyElement = if copied == Some(i) {
                    type_scale::SMALL
                        .el()
                        .child(text(locale, Key::CommonCopied))
                        .into_any_element()
                } else {
                    type_scale::CODE_SMALL
                        .el()
                        .text_color(t.text_primary)
                        .child(middle_truncate(path, 64))
                        .into_any_element()
                };
                row.child(
                    ui::focusable(div().id(("kbq-file", i)), t, false)
                        .min_w_0()
                        .px(px(4.))
                        .mx(px(-4.))
                        .rounded(px(crate::theme::RADIUS_XS))
                        .cursor_pointer()
                        .hover({
                            let hover = t.fill_hover;
                            move |s| s.bg(hover)
                        })
                        .tooltip(ui::tip(tip))
                        .on_click(move |_, _, cx| {
                            cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(
                                full.clone(),
                            ));
                            on_copied(i, cx);
                        })
                        .child(shown),
                )
                .children(state.map(|k| {
                    type_scale::SMALL
                        .el()
                        .flex_shrink_0()
                        .text_color(t.text_secondary)
                        .child(text(locale, k))
                }))
            }
        };
        col = col.child(el);
    }
    Some(col.into_any_element())
}

/// A path cut in the middle when longer than `max` characters.
pub fn middle_truncate(path: &str, max: usize) -> String {
    let chars: Vec<char> = path.chars().collect();
    if chars.len() <= max || max < 5 {
        return path.to_owned();
    }
    let keep = max - 1;
    let head = keep / 2;
    let tail = keep - head;
    let mut out: String = chars[..head].iter().collect();
    out.push('\u{2026}');
    out.extend(&chars[chars.len() - tail..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_paths_are_cut_in_the_middle() {
        assert_eq!(middle_truncate("src/kb.rs", 64), "src/kb.rs");
        let cut = middle_truncate(
            "crates/acta-app/src/terminal/shell_detection_on_windows.rs",
            24,
        );
        assert_eq!(cut.chars().count(), 24);
        assert!(cut.starts_with("crates/acta") && cut.ends_with("windows.rs"));
        assert!(cut.contains('\u{2026}'));
    }
}
