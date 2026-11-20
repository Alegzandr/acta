# 0007 - Documents

Status: accepted
Date: 2026-11-20

## Context
The user wants documents attached to a project, an epic or a topic: a list Claude Code can pick from when it needs to, and that it can add to. This is the documentation side of a piece of work.

Acta already has two kinds of text on a topic. Documents must not blur with them:

| Kind | Answers | Shape |
|------|---------|-------|
| Current state | Where are we now? | A few lines, overwritten |
| Timeline | What happened and why? | Append-only entries, chronological |
| Documents | What stays true and is worth re-reading? | Titled, edited in place: specs, architecture notes, research, how-tos, reports |

## Decision
**Model.** A document has a title, a one-line summary, a Markdown body, an author (human or Claude Code), timestamps, and exactly one owner: a project, an epic or a topic. It follows ADR 0006: stable UUID, soft delete, included in exports.

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

**UI.** A Documents section on the project, epic and topic views: a compact list (title, summary, author mark, updated), a reading view set for long-form Markdown, an editor, history. Reachable from the command palette ("Open document...", "New document").

## Alternatives
- Linking to files in the repository instead of storing content: breaks encryption and export guarantees, and paths rot. A document can still mention a repo path in its text.
- One long description field per topic: no list to pick from, no summaries, forces the agent to read everything.
- Documents only on topics: project-wide references would be duplicated or lost.
- Binary attachments now: see above.

## Consequences
- Schema: `documents` and `document_revisions` tables, bodies in the full-text index.
- MCP tool surface grows; ADR 0005 must absorb it while staying small, likely as two or three tools (`docs` list/search, `doc_read`, `doc_write`) rather than one per verb.
- Revisions grow the database. Text is small; revisit with a retention rule only if measurements say so.
- Heading-based section addressing needs a precise rule for duplicate headings and for documents without headings (fall back to ranges).
- Milestones: schema, list, reading view and editor in M2; MCP and CLI access in M3, before dogfooding, so this project's own specs live in Acta.

## Evidence
None yet. Design decision; full-text indexing inside the encrypted database is proven in ADR 0003.
