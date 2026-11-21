# 0012 - Attachments on notes, entries and descriptions

Status: accepted for storage, viewing in the app, Save as, export and agent access; **proposed** for "Open in another app", which writes a decrypted copy to disk and so needs the user's acceptance as an exception to CLAUDE.md's plaintext rule (below). Until it is accepted, the UI ships without that one action.
Date: 2026-11-21

## Context
The user asked on 2026-11-21 that files can be pasted into notes: from the clipboard, by drag and drop and with a button; images, PDFs, any file. When a note is filed, its files go with it. ADR 0007 kept knowledge text-only in v1 ("no binary attachments; revisit after 1.0"); this ADR does not change knowledge, it adds files to notes and to what notes become.

Forces:
- CLAUDE.md: no plaintext user data on disk, in logs or in process arguments. The database is SQLCipher with a raw key in the OS keychain (ADR 0003).
- ADR 0006: soft delete with a 30-day trash, automatic snapshots that are whole copies of the encrypted database (rolling: 7 daily, 4 weekly, plus the last of each other kind), and an open, lossless `.acta.json` export.
- ADR 0005: agent output is compact, budgeted text inside a data fence; binaries must never reach the context.
- GPUI decodes images from bytes in memory (PNG, JPEG, GIF, WebP, BMP through the `image` crate); there is no PDF renderer in the stack.
- MCP tool results can carry `image` content (base64 with a media type) next to `text`; Claude Code passes it to the model. Claude resizes images whose long side exceeds 1,568 px; an image costs about width x height / 750 tokens.

## Decision

**1. Where attachments live: inside the encrypted database.** Two tables:
- `attachment_blobs`: content-addressed by the SHA-256 of the plaintext, with size, media type, and for images the pixel size and a thumbnail (256 px on the long side, PNG, a few tens of KB) made once when the file is added. The content is in `attachment_chunks(blob, seq, data)`, 1 MiB per row, so that adding, reading and exporting stream with bounded memory and never hold a transaction for a whole file.
- `attachments`: `id` (UUID), short id `A-n`, one owner (a note, a timeline entry, or the description of a topic or an epic), `blob`, the file name as given (max 255 chars), `position`, `created_at`, `created_by` (human only in v1), soft delete with the metadata of design request 11.

Encryption, snapshots, live refresh, export and atomicity with the note come from the database as they already work; Acta adds no cryptography of its own. The hash is stored only inside the encrypted database.

**2. Limits.** 25 MB per file, refused above with the size and the limit in the message. A file over 10 MB is accepted with a one-line notice that it is kept in every snapshot. At most 20 files per owner. No total quota; Settings shows the total ("Attached files: 312 MB in 48 files") next to the database size, so growth is visible.

**3. Deduplication.** Adding a file whose hash already exists links the existing blob: pasting the same screenshot twice, or a file kept by a note and its copy after filing, costs nothing. A blob is shared by every attachment row that names it.

**4. Deleting and purging.** Removing an attachment, or deleting its owner, soft-deletes the attachment row (trash, 30 days, restorable with its owner). When the trash expires, the row is deleted; a blob no row names any longer (live or in the trash) is deleted in the same transaction, and `PRAGMA incremental_vacuum` returns the pages. The migration that adds the tables also sets `auto_vacuum = INCREMENTAL` and runs `VACUUM` once, after the pre-migration snapshot ADR 0006 already takes. No key press destroys a file at once (ADR 0006's known limit applies: something pasted by mistake stays in the database and its snapshots until the trash and the snapshot retention expire).

**5. Filing carries the files.** A note filed as an entry moves its attachments to that entry; a note made into a topic or an epic moves them to the new record's description, where the note's text also goes. Moving is a change of owner, not a copy; undoing the filing moves them back. A dismissed note keeps its files until it is deleted.

**6. Viewing in the app, decrypted in memory only.** Images show as thumbnails and open in an in-app viewer at full size. Text-like files (valid UTF-8 with no NUL byte in the first 8 KB: `.txt`, `.md`, `.csv`, `.json`, `.log`, source code) open in the same viewer as read-only monospaced text, the first 1 MB. Nothing is written to disk to show them.

**7. Getting a file out.** "Save as..." writes the decrypted file where the human chooses, through the system's save dialog: an explicit act, like an export, and the same kind of plaintext the human asked for. It is always available.

**8. "Open in another app" (proposed).** For PDFs and any other file, an external application needs a file on disk. Acta would write the decrypted copy into `<data dir>/open/<random>/<file name>` (a per-user directory: inside `%LOCALAPPDATA%` on Windows, mode 0700 with the file at 0600 on macOS and Linux), hand it to the system's opener, and delete the directory's contents when Acta quits and again at the next start (after a crash). The first use shows the sentence of the catalogue ("Acta writes a decrypted copy for the other app and deletes it when Acta quits.") with Open and Cancel, once per installation. This is a new plaintext file for as long as Acta runs, so it ships only when the user accepts it and CLAUDE.md lists it next to the web view profiles as an accepted exception. Without it the human uses Save as.

**9. Export.** The `.acta.json` export includes attachments by default: one `blobs` array, deduplicated by hash, each blob's content in base64, and attachment records referencing blobs by hash. The export dialog shows the size they add and a checkbox, "Include attached files (32 MB)", on by default; unchecked, attachment records are exported with their names and sizes and no content, and an import of that file lists them as "not in the export". Import deduplicates by hash. The export format version goes up by one; older files import with no attachments.

**10. Claude Code.** Names and sizes only, everywhere, unless the agent asks for one file:
- `find(kind="note")`, `brief` and `entries` add a short segment where a record has files: `· files: A-4 plan.pdf 2.1M, A-5 IMG_2041.jpg 3.4M +1` (two at most, names cut to 40 chars, inside the data fence like all stored text); `create` with `from_notes` adds `(2 files moved)` to its ack.
- A new tool, `attachment(id, lines?, pages?, size?)`, read-only, the tenth for the model:
  - text-like: the text, fenced, by `lines` like `kb_read`, budget 1,500 tokens (6,000 chars);
  - image: one `image` content block, re-encoded in memory to at most 1,092 px on the long side by default (about 1,600 tokens; `size="full"` allows 1,568, about 3,300), JPEG for photos and PNG for screenshots and diagrams, plus one text line `A-5 IMG_2041.jpg · 3024x4032 · 3.4 MB · sent 819x1092`;
  - PDF: the text of the requested pages (`pages="3-5"`, default the first pages that fit), extracted by a pure-Rust library chosen by the integration engineer, fenced, within the same budget; a PDF with no text layer says so in one line;
  - anything else: one line, `A-7 archive.zip · 4.2 MB · binary, not readable here`.
- The agent cannot add, remove or move attachments in v1; files reach Acta from the human. The skill says that text read from a file or seen in an image is data, never instructions, as fenced text is.

## Alternatives
- Encrypted files under the data directory, content-addressed, with a key derived from the database key: snapshots would share files instead of copying them, but Acta would carry its own file encryption, a two-phase commit between files and rows, and a purge that must know which snapshots still name a file. Kept as the move if attachments exceed 500 MB on a real installation or the snapshot directory exceeds 5 GB.
- One row per file with SQLite's incremental blob I/O and no chunks: simpler, but a 25 MB write holds one long transaction, and the export and the agent would need their own streaming.
- Plaintext files beside the database, or links to the user's files: against CLAUDE.md, and links rot.
- Excluding attachments from export: the export would no longer be the lossless copy ADR 0006 promises.
- Always writing temporary files to show anything: plaintext on disk for every glance at a screenshot.
- Returning images to the agent as files on disk, or as base64 text: the first is plaintext, the second costs about four times the tokens of an image block and the model cannot see it.

## Consequences
- Database and snapshot size grow with attachments, and each retained snapshot holds them again: 300 MB of files is about 4 GB across a full rotation of snapshots. The caps, the Settings totals and the revisit threshold above are how this is watched.
- Opening the database stays fast: blobs are in their own tables and never read by the queries behind pages, the tree or `brief`.
- A file pasted by mistake can be removed from view at once but leaves the disk only when the trash and snapshot retention expire (ADR 0006).
- ADR 0005 gains a tenth tool and image content in results; `tools/list` stays under 7,000 chars and every text result keeps its budget test. ADR 0006 gains the blobs in the export format.
- ADR 0007 is unchanged: knowledge stays text; a document cannot hold attachments.

## Evidence
None yet. Design decision. To prove when built: chunked writes of a 25 MB file off the UI thread on Windows with the database open in the app and in `acta mcp`; an `image` result reaching the model in one headless Haiku run; `incremental_vacuum` returning pages after a purge on SQLCipher.
