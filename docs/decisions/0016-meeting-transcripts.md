# 0016 - Meeting transcripts

Status: accepted (2026-11-25, user's choices; to be specified by the product-designer before it is built).

## Context
The user: "I'd like a second way to feed topics and epics, in addition to notes: meeting transcripts." Asked where they come from, the user chose pasted text and imported files (not recording inside Acta). Asked what happens next, the user chose that Claude Code makes use of them, as it does with notes.

## Decision
1. **A transcript is a record on a topic or an epic** (not on a project, like Report to): title, meeting date, participants (contacts, ADR on contacts), source (pasted, or the imported file's name), and the text. Stored encrypted like notes; searchable (V8 search covers it); in the trash like the rest; exported in backups.
2. **Input, two ways, both by the human only:**
   - Paste: "Add a transcript" opens a box to paste the text; Acta normalises it (speaker lines, timestamps kept as plain prefixes).
   - Import: drag or choose a file. Text formats are parsed in Rust: WebVTT (`.vtt`, Teams/Zoom/Meet), SubRip (`.srt`), `.txt`, `.md`, `.docx` (text only). Speaker names and times are kept; cue numbering and styling are dropped. The original file is not kept (only the text).
   - Audio or video files are out of scope for now (no recording, no transcription of media); the user did not ask for them.
3. **Claude Code uses them like notes.** A new transcript shows as "not processed" on its page. The user asks Claude Code (or a slash command `/acta:transcript`) to process it: Claude Code reads it through MCP (paged, in the data fence) and files, in the transcript's topic or epic or the right topics below it: decisions (entries), questions asked to people (questions with contacts and due dates), actions (topics or next steps), updates to the current state, and durable memories. Each result names its source (`refs` with the transcript's id), and the transcript is marked processed with the list of what came out of it. The human reviews; nothing is deleted from the transcript.
4. **Who writes what.** Only the human adds a transcript (it is typed content in the sense of the "notes only" rule: pasted or imported, never edited by Claude Code). Claude Code reads it and writes elsewhere; it never creates, edits or deletes transcripts. MCP gets read-only `transcript` access (list and paged read) and a `processed` mark.
5. **Context economy.** Transcripts are long: MCP returns a summary line first (date, participants, length, processed or not) and pages of the text on request; never the whole text in one answer.

## Consequences
- Schema V9: a `transcript` table (encrypted DB) and FTS coverage.
- New design specs: the "Add a transcript" dialog, the transcript page (text with speaker column, processed state and its outputs), the topic and epic sections listing transcripts, catalogue rows.
- Parsers for VTT, SRT and DOCX text with tests on real-shaped samples (Teams, Zoom, Meet exports) using the name "Alexandre".
- A using-acta skill section on processing a transcript; tools/list budget stays under 7,000.
