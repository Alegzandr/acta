//! Sample data for captures and manual checks (debug builds only, with
//! `ACTA_DEV_SEED=1` and an empty database). Sample data uses the name
//! "Alexandre" (CLAUDE.md).

use std::path::Path;

use acta_core::Store;
use acta_core::model::{Author, EntryKind, KbKind, Owner, Status};
use acta_core::store::{NewEntry, NewKb, NewTopic, TopicPatch};

/// One empty project, "Acta", with `folder` as its working folder: no
/// topics, no notes (the "File with Claude Code" check).
pub fn bare(store: &mut Store, folder: &Path) -> acta_core::Result<()> {
    if !store.projects()?.is_empty() {
        return Ok(());
    }
    let acta = store.create_project("Acta", "", Author::Human)?;
    store.add_folder(Owner::Project(acta), folder, Author::Human)?;
    Ok(())
}

/// Fill an empty store. `folder` becomes the project's working folder.
pub fn seed(store: &mut Store, folder: &Path) -> acta_core::Result<()> {
    if !store.projects()?.is_empty() {
        return Ok(());
    }
    let me = Author::Human;
    let claude = Author::Claude;
    let acta = store.create_project(
        "Acta",
        "A project tracker for Alexandre and a structured memory for Claude Code.",
        me,
    )?;
    store.add_folder(Owner::Project(acta), folder, me)?;
    let topic = |store: &mut Store, epic, title: &str, status| {
        store.create_topic(
            NewTopic {
                project: acta,
                epic,
                title: title.into(),
                description: String::new(),
                status,
            },
            me,
        )
    };
    topic(store, None, "Naming", Status::Draft)?;
    let core = store.create_epic(acta, "Core UI", "The workbench: tree, pages, palette.", me)?;
    topic(store, Some(core), "App shell", Status::Done)?;
    topic(store, Some(core), "Sidebar tree", Status::Active)?;
    topic(store, Some(core), "Command palette", Status::Planned)?;
    let terminal = store.create_epic(
        acta,
        "Terminal",
        "Everything that runs Claude Code inside Acta.",
        me,
    )?;
    let embed = topic(
        store,
        Some(terminal),
        "Embed the terminal bound to the project",
        Status::Active,
    )?;
    topic(
        store,
        Some(terminal),
        "Terminal scrollback",
        Status::Blocked,
    )?;
    topic(
        store,
        Some(terminal),
        "Links in terminal output",
        Status::Planned,
    )?;
    topic(store, Some(terminal), "Terminal spike", Status::Done)?;
    store.update_topic(
        embed,
        TopicPatch {
            description: Some(
                "Run Claude Code inside Acta, in the project directory, so the tracker and the agent share one window."
                    .into(),
            ),
            ..TopicPatch::default()
        },
        me,
    )?;
    store.log(
        embed,
        NewEntry {
            kind: EntryKind::Decision,
            text: "One terminal session per project. Navigating never restarts it; **Restart here** does.".into(),
            state: None,
            status: None,
        },
        me,
    )?;
    store.log(
        embed,
        NewEntry {
            kind: EntryKind::Progress,
            text: "Resize now reflows the grid; the last row is still cut when the panel is under 12 rows.".into(),
            state: Some(
                "Rendering and input work on Windows. Resize still drops the last row; the fix is in progress."
                    .into(),
            ),
            status: None,
        },
        claude,
    )?;
    store.kb_create(
        NewKb {
            owner: Owner::Topic(embed),
            kind: KbKind::Document,
            title: "Resize protocol".into(),
            summary: "How the grid reflows; read before touching it".into(),
            body: "## Goals\n\nThe grid must reflow on every resize without losing the scrollback position.\n\n- Width shrinks: soft-wrapped lines rewrap.\n- Height shrinks: rows move into scrollback.\n\n## Sequence\n\n```\nresize(cols, rows) -> pty.resize -> grid.reflow\n```\n".into(),
            from_file: false,
        },
        claude,
    )?;
    store.kb_create(
        NewKb {
            owner: Owner::Topic(embed),
            kind: KbKind::Memory,
            title: "ConPTY minimum".into(),
            summary: "Needs Windows 10 1809 or later".into(),
            body: "ConPTY is missing before Windows 10 1809. Acta refuses to start the terminal there and says why.".into(),
            from_file: false,
        },
        claude,
    )?;
    store.kb_create(
        NewKb {
            owner: Owner::Project(acta),
            kind: KbKind::Memory,
            title: "Sample data".into(),
            summary: "Sample and test data uses the name Alexandre".into(),
            body: "Every sample and test uses the name Alexandre.".into(),
            from_file: false,
        },
        claude,
    )?;
    store.add_note(
        acta,
        "Terminal should remember its width per project, not globally.",
        me,
    )?;
    store.add_note(
        acta,
        "Idea: weekly digest of decisions across all topics.",
        me,
    )?;
    store.create_project("Atlas", "", me)?;
    Ok(())
}

/// [`seed`], then the long tree of docs/design/layout.md 4.2.1 (an empty
/// epic, a project of 140 topics with long titles) for captures of the tree.
pub fn long(store: &mut Store, folder: &Path) -> acta_core::Result<()> {
    if !store.projects()?.is_empty() {
        return Ok(());
    }
    seed(store, folder)?;
    let me = Author::Human;
    let acta = store.projects()?[0].id;
    store.create_epic(acta, "Storage", "", me)?;
    let archive = store.create_project("Archive of Alexandre's experiments", "", me)?;
    let epic = store.create_epic(
        archive,
        "Migrations and long-running background jobs",
        "",
        me,
    )?;
    let statuses = [
        Status::Planned,
        Status::Active,
        Status::Draft,
        Status::Blocked,
        Status::Done,
        Status::Dropped,
    ];
    for i in 0..140 {
        store.create_topic(
            NewTopic {
                project: archive,
                epic: (i % 2 == 0).then_some(epic),
                title: format!("Experiment {i}: measure the reflow of very long titles"),
                description: String::new(),
                status: statuses[i % statuses.len()],
            },
            me,
        )?;
    }
    Ok(())
}
