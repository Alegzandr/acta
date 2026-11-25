# 0007 - Knowledge base: documents and agent memory

Status: accepted
Date: 2026-11-20

## Addendum (2026-11-25): deliverables are files in the folder
The user: "What Claude retains about the subject stays in Acta, but if I ask Claude to produce a document to present to my clients, I want to find it in the folder, not in Acta." The knowledge base holds what the work needs to remember (memories, reference documents). Anything produced for other people is written as an ordinary file in the working folder, where the human and git see it; the session logs an entry naming the path. The using-acta skill says so ("Deliverable"). Acta shows such files in place (Files view, Markdown rendering) and never copies them into the database.

## Context
The user wants documents attached to a project, an epic or a topic: a list Claude Code can pick from when it needs to, and that it can add to. This is the documentation side of a piece of work.

Acta already has two kinds of text on a topic. Documents must not blur with them:

| Kind | Answers | Shape |
|------|---------|-------|
| Current state | Where are we now? | A few lines, overwritten |
| Timeline | What happened and why? | Append-only entries, chronological |
| Documents | What stays true and is worth re-reading? | Titled, edited in place: specs, architecture notes, research, how-tos, reports |

The user then added: what Claude Code records as its memory must be readable by the human too, as a knowledge base at the scale of a project, an epic or a topic. Claude Code's built-in memory is a set of files outside the project that the human rarely sees; Acta replaces that blind spot with one shared, visible store.

## Decision
**One knowledge base per level.** Each project, epic and topic has a knowledge base. It holds entries of two kinds, the same entity underneath:

| Kind | What | Typical size | Typical author |
|------|------|--------------|----------------|
| Document | Long-form reference: spec, architecture, research, how-to, report | Pages | Either |
| Memory | One durable fact worth not relearning: a convention, a pitfall, a preference, a constraint, where something lives | A few lines | Claude Code |

Both have a title and a one-line summary, both are listed, searched, edited and versioned the same way. The kind only changes defaults: how it is displayed and how the agent is taught to use it.

**The human owns the agent's memory.** Every memory Claude Code writes is visible in the UI at the level it was filed, marked as written by Claude Code, and can be edited, moved to another level, or deleted by the human. The agent reads the corrected version from then on. Nothing the agent remembers about the project is hidden from the person it works for.

**Two directions of scope.**
- The agent looks up: on a topic it sees the topic's knowledge, then its epic's, then its project's.
- The human looks down: a project's knowledge base shows everything known in the project, filterable by level, kind and author; an epic's shows the epic and its topics.

**Filing rule taught by the skill.** File a memory at the narrowest level where it stays true: a quirk of one task on the topic, a rule for a feature area on the epic, a convention of the codebase on the project. One fact per memory. Before writing, search; update the existing entry instead of adding a near-duplicate. Durable knowledge goes to Acta rather than to Claude Code's own memory files, so there is one place to look.

**Recall.** The session briefing may list the titles of the few most relevant memories in scope within its token budget, and always gives the counts. Bodies are never injected; the agent opens what it needs.

**Model.** An entry has a kind (document or memory), a title, a one-line summary, a Markdown body, an author (human or Claude Code), timestamps, and exactly one owner: a project, an epic or a topic. It follows ADR 0006: stable UUID, soft delete, included in exports.

**Stored in the database.** Bodies live in the encrypted database, not as files on disk, so documents get the same encryption, snapshots, export and live refresh as everything else. A document can be created from a `.md` or `.txt` file and saved back to one.

**Text only in v1.** Markdown, no binary attachments (PDF, images). They would need a blob store, viewers, and a different answer for the agent. Revisit after 1.0.

**Revisions.** Every save keeps the previous version. A human or an agent can overwrite a document; neither can destroy what it said before. The UI shows history and restores a version.

**Inheritance.** Working on a topic, the agent and the human see that topic's documents, then its epic's, then its project's, each labelled with its level. A project-level document ("Architecture", "Conventions") is therefore within reach of every topic without being copied.

**Agent access, context economy first.** Nothing about documents is ever injected automatically. The agent pulls:
1. *List* - for the current scope: short id, title, one-line summary, level, approximate size in tokens, last update. No bodies. Paginated.
2. *Outline* - a document's headings with the size of each section.
3. *Read* - one section by heading, or a bounded range. Reading a whole document is allowed only under a size budget; above it the tool returns the outline and says so.
4. *Search* - full-text across the documents in scope, returning short excerpts with document id and heading.
5. *Write* - create a document (title and summary are required), replace one section, append a section, or replace the whole body. Section-level edits are the default path so an agent does not have to re-send a long document to change a paragraph.

The one-line summary is what makes the list useful: it is how the agent decides whether a document is worth opening. It is required on creation and the skill teaches the agent to write it well.

The session briefing injected at start mentions only that documents exist and how many ("3 documents in scope"), never their content.

**UI.** A Knowledge section on the project, epic and topic views, with documents and memories distinguishable at a glance: a compact list (title, summary, author mark, updated), a reading view set for long-form Markdown, an editor, history. Reachable from the command palette ("Open document...", "New document").

## Alternatives
- Separate systems for documents and memory: two lists, two sets of tools, and the agent has to guess which one holds the answer.
- Leaving memory in Claude Code's own files: invisible to the human, not scoped to epics or topics, lost on another machine.
- Linking to files in the repository instead of storing content: breaks encryption and export guarantees, and paths rot. A document can still mention a repo path in its text.
- One long description field per topic: no list to pick from, no summaries, forces the agent to read everything.
- Documents only on topics: project-wide references would be duplicated or lost.
- Binary attachments now: see above.

## Consequences
- Schema: `kb_entries` (with `kind`) and `kb_revisions` tables, bodies in the full-text index.
- MCP tool surface grows; ADR 0005 must absorb it while staying small, as three tools (`kb`, `kb_read`, `kb_write`) rather than one per verb.
- Revisions grow the database. Text is small; revisit with a retention rule only if measurements say so.
- Heading-based section addressing needs a precise rule for duplicate headings and for documents without headings (fall back to ranges).
- Milestones: schema, list, reading view and editor in M2; MCP and CLI access in M3, before dogfooding, so this project's own specs live in Acta.

## Evidence
None yet. Design decision; full-text indexing inside the encrypted database is proven in ADR 0003.
