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

/// [`seed`] with due dates and an Overview to show (components-records.md
/// 5, screens.md 8.14 and 8.20), relative to today: overdue topics (one
/// also written by Claude Code, one blocked), one due today, one in 2 days,
/// one in 30 days, a done topic with a past date, epics with dates, an
/// active topic idle for 12 days, an inbox 12 days old, a memory not
/// updated for 100 days; and a calm project, "Field notes".
#[cfg(debug_assertions)]
pub fn due(store: &mut Store, folder: &Path) -> acta_core::Result<()> {
    use acta_core::date::Zone;
    use acta_core::model::Record;
    if !store.projects()?.is_empty() {
        return Ok(());
    }
    seed(store, folder)?;
    let me = Author::Human;
    let today = Zone::system().today();
    let tree = store.tree(store.projects()?[0].id)?;
    let acta = tree.project.id;
    let find = |title: &str| {
        tree.loose_topics
            .iter()
            .chain(tree.epics.iter().flat_map(|(_, t)| t))
            .find(|t| t.title == title)
            .map(|t| t.id)
    };
    let dates = [
        ("Naming", -3),
        ("Sidebar tree", 0),
        ("Command palette", 2),
        ("App shell", -10),
        ("Embed the terminal bound to the project", -3),
        ("Terminal scrollback", -1),
        ("Links in terminal output", 30),
    ];
    for (title, days) in dates {
        if let Some(id) = find(title) {
            // Claude Code dates the embed topic: it stays unread.
            let by = if title.starts_with("Embed") {
                Author::Claude
            } else {
                me
            };
            store.set_due(Owner::Topic(id), Some(today.add_days(days)), by)?;
        }
    }
    for (epic, _) in &tree.epics {
        let days = if epic.title == "Terminal" { 27 } else { -2 };
        store.set_due(Owner::Epic(epic.id), Some(today.add_days(days)), me)?;
    }
    let idle = store.create_topic(
        NewTopic {
            project: acta,
            epic: None,
            title: "Release checklist".into(),
            description: String::new(),
            status: Status::Active,
        },
        me,
    )?;
    store.dev_backdate(Record::Topic(idle), 12)?;
    let inbox = store
        .inbox(acta, acta_core::model::PageReq::first(10))?
        .items;
    for (n, note) in inbox.into_iter().enumerate() {
        store.dev_backdate(Record::Note(note.id), if n == 0 { 12 } else { 3 })?;
    }
    let memory = store.kb_create(
        NewKb {
            owner: Owner::Project(acta),
            kind: KbKind::Memory,
            title: "Old build note".into(),
            summary: "Builds used to need a native Perl".into(),
            body: "Alexandre's machine needs a native Perl for vendored OpenSSL.".into(),
            from_file: false,
        },
        Author::Claude,
    )?;
    store.dev_backdate(Record::Kb(memory), 100)?;
    let calm = store.create_project("Field notes", "", me)?;
    for title in ["Interview Alexandre's team", "Sort the photos"] {
        store.create_topic(
            NewTopic {
                project: calm,
                epic: None,
                title: title.into(),
                description: String::new(),
                status: Status::Planned,
            },
            me,
        )?;
    }
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

/// [`seed`] with the folders of screens.md 8.11, built under `root` (a
/// scratch directory: it creates git repositories there): the project's
/// checkout `acta` (a clone one commit behind its remote, after a fetch)
/// and `acta-docs` (not found); the epic Terminal's `alacritty-fork` on
/// `acta-patches`; the topic "Embed the terminal" in its own worktree
/// `embed-terminal` on `topic/embed`, three files changed, Commit and
/// push, and a notes folder named "Design notes".
#[cfg(debug_assertions)]
pub fn folders(store: &mut Store, root: &Path) -> acta_core::Result<()> {
    use acta_core::model::CommitMode;
    if !store.projects()?.is_empty() {
        return Ok(());
    }
    let git = |dir: &Path, args: &[&str]| {
        let _ = acta_core::git_cmd::command(dir).args(args).output();
    };
    let identity = |dir: &Path| {
        git(dir, &["config", "user.name", "Alexandre"]);
        git(dir, &["config", "user.email", "alexandre@example.com"]);
        git(dir, &["config", "commit.gpgsign", "false"]);
    };
    let remote = root.join("remote.git");
    let seed_dir = root.join("seed");
    let acta = root.join("acta");
    for dir in [&remote, &seed_dir] {
        std::fs::create_dir_all(dir)?;
    }
    git(&remote, &["init", "--bare", "-q", "-b", "main"]);
    git(&seed_dir, &["init", "-q", "-b", "main"]);
    identity(&seed_dir);
    std::fs::write(seed_dir.join("README.md"), "Acta\n")?;
    git(&seed_dir, &["add", "README.md"]);
    git(&seed_dir, &["commit", "-q", "-m", "First"]);
    git(
        &seed_dir,
        &["remote", "add", "origin", &remote.to_string_lossy()],
    );
    git(&seed_dir, &["push", "-q", "-u", "origin", "main"]);
    git(root, &["clone", "-q", &remote.to_string_lossy(), "acta"]);
    identity(&acta);
    std::fs::write(seed_dir.join("NOTES.md"), "Later\n")?;
    git(&seed_dir, &["add", "NOTES.md"]);
    git(&seed_dir, &["commit", "-q", "-m", "Second"]);
    git(&seed_dir, &["push", "-q"]);
    git(&acta, &["fetch", "-q"]);
    let embed_dir = root.join("embed-terminal");
    git(
        &acta,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "topic/embed",
            &embed_dir.to_string_lossy(),
        ],
    );
    for (file, text) in [("a.rs", "a\n"), ("b.rs", "b\n"), ("README.md", "Acta!\n")] {
        std::fs::write(embed_dir.join(file), text)?;
    }
    let fork = root.join("alacritty-fork");
    std::fs::create_dir_all(&fork)?;
    git(&fork, &["init", "-q", "-b", "acta-patches"]);
    identity(&fork);
    git(&fork, &["commit", "-q", "--allow-empty", "-m", "Fork"]);
    let notes = root.join("notes").join("terminal");
    let docs = root.join("acta-docs");
    std::fs::create_dir_all(&notes)?;
    std::fs::create_dir_all(&docs)?;

    seed(store, &acta)?;
    let me = Author::Human;
    let tree = store.tree(store.projects()?[0].id)?;
    let project = Owner::Project(tree.project.id);
    store.add_folder(project, &docs, me)?;
    std::fs::remove_dir(&docs)?;
    let (terminal, topics) = tree
        .epics
        .iter()
        .find(|(e, _)| e.title == "Terminal")
        .expect("the seed's epic");
    store.add_folder(Owner::Epic(terminal.id), &fork, me)?;
    let embed = topics
        .iter()
        .find(|t| t.title.starts_with("Embed"))
        .expect("the seed's topic")
        .id;
    let wt = store.add_folder(Owner::Topic(embed), &embed_dir, me)?;
    let n = store.add_folder(Owner::Topic(embed), &notes, me)?;
    store.set_folder_name(n, "Design notes", me)?;
    store.set_folder_icon(n, "book-open", me)?;
    let _ =
        acta_core::claude_settings::set_commit(store, wt, CommitMode::Push, Some(true), false, me);
    Ok(())
}

/// Questions, Report to, contacts and the profile (screens.md 8.21 to
/// 8.24, 8.28 to 8.30), on the sample data: two Maries at nested levels
/// and one in another project, a contact in the trash still named, an
/// overdue question, a 300-character one, a French "le service RH", a
/// done topic not reported, a note with a person, a 500-character address
/// note.
#[cfg(debug_assertions)]
pub fn questions(store: &mut Store, folder: &Path) -> acta_core::Result<()> {
    use acta_core::date::Zone;
    use acta_core::model::{ContactLevel, Record};
    use acta_core::store::{ContactPatch, NewQuestion};
    if !store.projects()?.is_empty() {
        return Ok(());
    }
    seed(store, folder)?;
    let me = Author::Human;
    let claude = Author::Claude;
    let today = Zone::system().today();
    store.set_profile(Some("Alexandre"), Some("Product designer"))?;
    let tree = store.tree(store.projects()?[0].id)?;
    let acta = tree.project.id;
    let find = |title: &str| {
        tree.loose_topics
            .iter()
            .chain(tree.epics.iter().flat_map(|(_, t)| t))
            .find(|t| t.title == title)
            .map(|t| t.id)
    };
    let terminal = tree
        .epics
        .iter()
        .find(|(e, _)| e.title == "Terminal")
        .map(|(e, _)| e.id);
    let atlas = store.create_project("Atlas", "", me)?;
    let project = ContactLevel::Project(acta);
    let detail = |store: &mut Store, id, job: Option<&str>, company: Option<&str>| {
        store.update_contact(
            id,
            ContactPatch {
                job_title: job.map(|j| Some(j.to_owned())),
                company: company.map(|c| Some(c.to_owned())),
                ..ContactPatch::default()
            },
            me,
        )
    };
    let marie = store.create_contact("Marie", project, claude)?;
    detail(store, marie, Some("Head of legal"), Some("Acme"))?;
    store.update_contact(
        marie,
        ContactPatch {
            address_note: Some(Some(
                "Vouvoiement. Madame Dubois in writing, Marie in person. Short messages, no \
                 attachments; she reads on her phone between meetings, so put the question in \
                 the first line and the context after it. She prefers one message with three \
                 questions to three messages. Never before 9:00 or after 18:00, and not on \
                 Wednesdays, when she is at the other office. If something is urgent, say so in \
                 the subject and call the legal desk rather than her mobile. Copy the legal team \
                 on anything about contracts or data leaving the EU."
                    .into(),
            )),
            ..ContactPatch::default()
        },
        me,
    )?;
    let marie_city = store.create_contact("Marie", ContactLevel::All, me)?;
    detail(store, marie_city, Some("Head of legal"), Some("City hall"))?;
    store.create_contact("Marie", ContactLevel::Project(atlas), me)?;
    let legal = store.create_contact("Legal team", project, me)?;
    let marius = store.create_contact("Marius", project, me)?;
    detail(store, marius, Some("Accounting"), None)?;
    let rh = store.create_contact("le service RH", project, me)?;
    let accountant =
        store.create_contact("Alexandre\u{2019}s accountant", ContactLevel::All, me)?;
    store.create_contact("Paul", project, claude)?;
    let (design, sami) = match terminal {
        Some(e) => {
            let design =
                store.create_contact("Design review group", ContactLevel::Epic(acta, e), me)?;
            let sami = store.create_contact("Sami", ContactLevel::Epic(acta, e), me)?;
            detail(store, sami, Some("Windows packaging"), None)?;
            (Some(design), Some(sami))
        }
        None => (None, None),
    };
    let supplier = store.create_contact("Old supplier", project, me)?;
    let embed = find("Embed the terminal bound to the project");
    let ask = |store: &mut Store, owner, text: &str, to, days: Option<i64>, by| {
        store.ask(
            NewQuestion {
                owner,
                text: text.into(),
                asked_to: to,
                expected_on: days.map(|d| today.add_days(d)),
            },
            by,
        )
    };
    if let Some(t) = embed {
        let owner = Owner::Topic(t);
        ask(
            store,
            owner,
            "Can we keep the v1 export format until March, or must both formats ship together?",
            Some(marie),
            Some(-3),
            me,
        )?;
        ask(
            store,
            owner,
            "Which Windows versions must the installer support, given that the terminal relies on \
             ConPTY, that some of our users are still on older builds of Windows 10 at work, and \
             that the packaging team would rather test fewer versions well than many versions \
             badly; is it acceptable to state Windows 10 1809 as the floor in the docs?",
            Some(legal),
            None,
            claude,
        )?;
        let eu = ask(
            store,
            owner,
            "Can the export file leave the EU?",
            Some(legal),
            None,
            me,
        )?;
        store.answer(
            eu,
            Some("Inside the EEA only; ask again for anything else."),
            true,
            me,
        )?;
        let key = ask(
            store,
            owner,
            "Do we need a second signing key?",
            None,
            None,
            claude,
        )?;
        store.drop_question(key, claude)?;
        ask(
            store,
            owner,
            "Is the HR onboarding checklist final?",
            Some(rh),
            Some(2),
            me,
        )?;
        store.set_report_to(Record::Topic(t), Some(marie), me)?;
    }
    if let Some(e) = terminal {
        ask(
            store,
            Owner::Epic(e),
            "Who reviews the terminal colours for contrast?",
            design,
            None,
            me,
        )?;
        ask(
            store,
            Owner::Epic(e),
            "Is ConPTY acceptable as the minimum on Windows?",
            sami,
            Some(-1),
            claude,
        )?;
        store.set_report_to(Record::Epic(e), Some(legal), me)?;
    }
    ask(
        store,
        Owner::Project(acta),
        "Do we keep a free tier on the website?",
        Some(accountant),
        Some(13),
        me,
    )?;
    if let Some(t) = find("Terminal spike") {
        store.set_report_to(Record::Topic(t), Some(marie), me)?;
    }
    if let Some(t) = find("App shell") {
        store.set_report_to(Record::Topic(t), Some(marie_city), me)?;
        store.set_reported(Record::Topic(t), true, me)?;
    }
    if let Some(t) = find("Sidebar tree") {
        store.set_report_to(Record::Topic(t), Some(supplier), me)?;
    }
    store.delete(Record::Contact(supplier), me)?;
    let note = store.add_note(acta, "Ask about the review room for Thursday", me)?;
    store.set_report_to(Record::Note(note), Some(marie), me)?;
    Ok(())
}
