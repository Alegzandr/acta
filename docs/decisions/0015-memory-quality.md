# 0015 - Memory quality

Status: accepted (2026-11-22, lead; to be proven by the phase 6 runs)
Date: 2026-11-22

## Context
The user endorsed a product critique on 2026-11-22: the agent's scoped memory must handle stale information, contradictions between memories, semantic duplicates, context changes, provenance, and concurrent edits by several sessions; "near-duplicate detection and optimistic revisions are already good bases, but memory quality will largely determine the product's quality."

What exists (read on `main` at 839ac61):

| Piece | Today | Where |
|-------|-------|-------|
| Entries | `kb_entries` (kind, title, summary 120, body; memory body 800), one owner (project, epic or topic), `created_by`, `updated_at`, `updated_by`, soft delete. | schema V1, ADR 0007 |
| Revisions | Every write keeps a `kb_revisions` row (author, `change`, `section`). `kb_update` takes `expected_rev` and refuses with `StaleRevision { current }` inside the write transaction; the agent gets "not saved: K-2 changed since rev 7; it is rev 8 now" with the fresh outline. The app's editor passes `expected_rev` too. | `store/kb.rs`, `agent/tools.rs` `kb_edit`, `kb_page.rs` |
| Near duplicates | New memories only: lower-cased words of 3+ letters, equal title word sets or Jaccard >= 0.6 on title or on summary against the memories of the target owner; refused once (one local value `kb.refused`), the identical call repeated saves anyway. No accent folding, no stop words, no stemming, no bodies, no ancestors or descendants, no edits, nothing for the human. | `agent/tools.rs` `near_duplicate` |
| Staleness | Overview line "N memories not updated for 90 days" (`STALE_MEMORY_DAYS`, by `updated_at`, which a move or a title change also bumps) and the Knowledge page chip. Nothing reaches the agent. | `store/overview.rs`, components-records.md 5 |
| Briefing | Up to 5 memory titles in scope, topic then epic then project, newest first, inside the 1,200-char session briefing. | `session_briefing` |
| Presence | One row per live session in `presence.db`: focus, state, last tool name, one line of the last `log`. | ADR 0005 amendment 2026-11-21 |
| Merge pattern | Contacts: "Merge into...", references moved in one transaction, the merged row soft-deleted with `merged_into`, one undo step; "Keep both" stored in `contact_distinct (a, b)`, a < b. | components-contacts.md, schema V5 |

Constraints: zero budget, local-first, no plaintext user data on disk or in process arguments, Claude Code on the user's subscription (no API key, no background model calls), agent output compact (`tools/list` under 7,000 chars, measured 6,491 after the contacts change; instructions under 800, measured 710; briefing 1,200; `kb` 1,800; acks 160). data.md 11 lists "file-change tracking" as not requested.

Claude Code: this decision relies on no Claude Code behaviour beyond the facts ADR 0005 verified (2026-11-20 and 21: tool search loads schemas on demand, descriptions and instructions cut at 4,096 chars, `SessionStart` `additionalContext`, `mcp_tool` hooks, `CLAUDE_CODE_SESSION_ID` and `CLAUDE_PROJECT_DIR` in the server's environment). The one hook idea that would need new facts (a `PostToolUse` push of "K-4 changed elsewhere") is left as a candidate, below, until its docs are checked and a run proves it.

## Decision

**Principle.** Acta detects cheaply and lexically, at the moment of writing or reading; the agent, which is already a language model, judges. Acta never deletes, rewrites or hides a memory on its own: every signal is a flag computed at read time or a warning at write time, and every state change (superseded, retired, merged) is an explicit act of the agent or the human, reversible and kept in history.

### 1. Staleness

**Verified date.** A memory or document gets `verified_at` / `verified_by`, distinct from `updated_at`. It is set when someone asserts the content is true: creation, a body or section write, a restore, and the explicit verify action. A move, a title or summary edit, or a kind change does not set it. The migration backfills it with `updated_at` / `updated_by` (the best value there is).

**Decay signals**, computed by one domain function `kb_flags(entries, ctx, today)` and never stored:

| Flag | Rule | Computed by |
|------|------|-------------|
| `old` | Memories only: `verified_at` on a local day at least 90 days before today (`STALE_MEMORY_DAYS`, unchanged). Documents do not age by date (data.md request 17's reasoning). | store |
| `topic closed` | The owner is a topic that is Done or Dropped. | store |
| `conflict` / `duplicate` | An open pair in `kb_pairs` (sections 2 and 3). | store |
| `folder removed` | `context_folder_id` names a deleted folder (section 4). | store |
| `branch gone` / `branch merged` | `context_branch` (section 4) no longer exists in the folder's repository, or is an ancestor of its default branch. | git, at read time |
| `file gone` / `file changed` | A file reference (section 5) no longer exists at `HEAD`, or its blob id differs from the one recorded when the memory was last verified. Outside git: missing, or modified after `verified_at`. | git, at read time |

This is not file-change tracking: nothing watches files. The git signals are read in one `git ls-tree -z HEAD -- <paths>` per folder (2 s timeout, as the folder view), plus `agent::git::inspect` for refs, only for the entries on the page being rendered, cached per (folder, `HEAD` oid) in the server's and the app's memory. data.md's "not requested" line is amended to say so.

**Agent.** A flag adds one marker to a line, inside the fence, at most 30 chars: `?old 140d`, `?topic done`, `?file changed`, `?file gone`, `?branch merged`, `?conflict K-9`, `?dup K-4`. In `kb` lists the marker ends the meta segment (`K-19 mem topic  Title · 08-14 from T-12 ?old 140d - summary`); the date shown for a memory is its verified date. The session briefing marks flagged titles with `?` after the id (`K-17? Credential Manager entry name`) and appends, only when non-zero and within the existing budget, `2 memories to check (kb shows why).` `kb_read` adds the flags in full to its header. The skill teaches: a `?` memory is a lead, not a fact; check it against the code before relying on it, then verify, fix, or retire it.

**Verify.** `kb_write(id, rev, op="verify")`: no text, sets `verified_at` and re-records the blob ids of its file references, writes a revision row with change `verified` but no new body (cheap, and the history shows who vouched when). Ack: `ok K-4 verified (rev 3)`. A stale `rev` is refused like any edit: you cannot vouch for a text you did not read.

**Human.** The Knowledge page's "Not updated for 90 days" chip becomes "To check" (every flag; copy and layout are the product designer's), each row showing its reason in the tooltip and a "Mark as still true" action (`V`). The Overview line counts the store-computable flags (`old`, `topic closed`, open pairs, `folder removed`) so it stays inside its 5 ms budget; git flags appear on the Knowledge page once read in the background. Closing a topic that owns memories adds to the `log`/`update` ack: `T-42 owns 3 memories: promote lasting ones with kb_write(id, rev, level="epic"|"project")` (once per close, 100 chars), the moment a topic-level fact is about to fall out of every scope.

**Never auto-delete.** No flag hides an entry, from the agent or the human. Retiring is section 2's explicit act.

### 2. Contradictions

**Detection at write time**, for memories (created, body or title or summary edited, or moved), against the live memories the writer and the target scope share: the target owner, its ancestors, and its descendants (a project-level fact can contradict a topic quirk). Candidates come from one FTS5 query over `kb_fts` with the new title, summary and body's content words OR-ed, filtered to that scope, top 20 by bm25; then the checker of section 3 scores topical overlap. A pair is a **possible conflict** when overlap is at least 0.35 and one of these differs:
- polarity: a negation or prohibition on one side only (en: not, no, never, don't, avoid, instead of, no longer, deprecated; fr: ne ... pas, jamais, plus, éviter, au lieu de, obsolète);
- an antonym pair from a fixed list of about 40 (enable/disable, on/off, true/false, allow/deny, always/never, required/optional, before/after, sync/async, keep/remove, and their French forms);
- a value: the same anchor word followed by a different number, version, flag (`--x`), code span or path.

A small local model is **not** justified now: it would add a download of 25 to 500 MB, an inference runtime, CI cost and a second engine for a judgement the agent is already making, with recall gains nobody has measured. Revisit with section 8's metrics: if missed duplicates or conflicts found by the human stay above 20% of the human's merges after a month of dogfooding, spike a sentence-embedding model through the ADR 0014 downloader.

**Reply.** A possible conflict refuses the write once (as near duplicates are refused today), naming at most two candidates with their ids, revisions and summaries fenced:

```text
not saved: possible conflict with K-9 rev 2 (project, verified 10-02)
[acta data ...] K-9 Tests run in parallel - cargo test runs all tests concurrently; ... [end acta data ...]
Reconcile: replace it (supersedes="K-9"), fix it (kb_write id="K-9" rev=2), or, if both hold, say in this summary when it applies and repeat the call.
```

About 90 tokens. The three ways out:
- **Supersede.** `supersedes: "K-9"` on the new write (one to three ids). In one transaction the new entry is saved and each named one goes to state `superseded` with `superseded_by`, and its pair is resolved. Ack: `ok K-21 memory · topic T-42 · rev 1 · supersedes K-9`.
- **Fix the old one** with an ordinary edit; the old text stays in history.
- **Keep both**: repeating the identical call saves it and records every named pair as `distinct`, so it never warns again for that pair. The refusal asks for a scope note in the summary first, which is how "both hold" stays legible.

**Superseded and retired.** States `live` (default), `superseded` (has a successor) and `retired` (no longer true, no successor). `kb_write(id, rev, op="retire", text="why")` retires one (reason at most 200 chars, kept as the revision's change note). Neither state appears in briefings, `kb` lists or `kb` search; the `kb` header counts them (`· 3 retired`), and `kb_read` of one still answers, headed `K-9 superseded by K-21 on 11-20`, so old references resolve. The human sees them on the Knowledge page under a "Superseded and retired" filter, and "Make live again" restores either in one undo step. A retire is not a delete: delete stays human only (ADR 0005).

Edits by the human are checked the same way but never refused: the editor shows a non-blocking notice ("Possibly contradicts K-9") with Open, Replace it, Keep both.

### 3. Semantic duplicates

**Normalisation** (one function shared by duplicates, conflicts and the scan): Unicode NFKD with combining marks removed, lower case, split on non-alphanumerics, a stop-word list for English and French (about 150 words), a light suffix strip for both languages (plural `s`/`es`/`x`, `ing`, `ed`, `ment`, `tion`), tokens of 2+ characters (keeps `ci`, `db`). Code spans, flags and paths are kept as whole tokens.

**Scores**, exact (no MinHash: with at most a few thousand memories per project and an FTS prefilter of 20 candidates, exact sets cost microseconds; MinHash is for collections a thousand times larger):
- `t`: Jaccard of character trigrams of the normalised titles;
- `w`: Jaccard of content tokens of title + summary + body (bodies are at most 800 chars);
- `s = max(t, w, 0.5 t + 0.5 w)`.

| Result | Rule | Agent | Human |
|--------|------|-------|-------|
| Same | Normalised title, summary and body all equal to a live memory in the shared scope | Refused every time: `not saved: same as K-4 (rev 3)`. Nothing to keep. | Notice, never refused. |
| Duplicate | `s >= 0.70`, no conflict signal | Refused once, naming up to two (today's behaviour, extended to ancestors, descendants and edits); `supersedes` merges, a repeat keeps both and records `distinct`. | Notice "Possible duplicate of K-4": Merge into it..., Keep both. |
| Related | `0.45 <= s < 0.70` | Saved; the ack ends `· related: K-4` (one id, 20 chars), so the agent can look without a refusal. | Nothing. |
| Conflict | Section 2 | Refused once. | Notice. |

The thresholds are constants, not settings, fixed by a labelled fixture of at least 80 pairs (half English, half French; Alexandre as the sample person) with targets: duplicates precision 0.8 and recall 0.8, conflicts precision 0.6 (a false alarm costs one repeat) and recall 0.6. The current 0.6 Jaccard and the one-slot `kb.refused` value are replaced; the refusal key becomes the target owner, the normalised title and the candidate ids.

**Merge** (the contacts pattern). The agent merges with `supersedes` on an edit of the survivor: `kb_write(id="K-4", rev=3, op="replace_body", text=<merged>, supersedes="K-11")`. The human uses "Merge into..." on a memory (row menu, notice, palette): the survivor gets a new revision whose body is its text, a blank line and the other's body (the editor opens on it, so the person can tidy it; the 800-char limit applies to typing, not to the merge), the other goes to `superseded`, file references and provenance links are copied to the survivor (the other keeps its own), the result lives at the wider of the two levels, and the pair is resolved; one undo step, toast "Merged into K-4. Undo". **Unmerge** after the undo window is "Make live again" on the superseded one; the survivor's own history restores its earlier body if wanted. Nothing else references a memory, so unlike contacts there is nothing to move back.

**Scan.** New memories are checked at write time; existing ones once, by `acta kb check [--project P-n]` and the palette command "Find duplicate memories" (each memory against its prefilter, about a second for 1,000 memories, off the UI thread). Found pairs go to `kb_pairs` as open; the scan never changes an entry.

### 4. Context changes

A memory's **context** is its level (owner) plus two optional narrowings:
- **Branch**: `level: "branch"` files a memory on the focus topic, true only on the session's current branch in its current folder (`context_branch`, `context_folder_id`, read from `CLAUDE_PROJECT_DIR` through the folder records and `agent::git::inspect`). For "on this branch the schema is at V7" facts. Refused outside a git folder or on the default branch (`level=branch needs a feature branch; use topic`).
- **Folder**: a memory whose file references (section 5) all sit in one folder records that folder as `context_folder_id`.

When the context changes, the memory is **flagged, never deleted or moved**: folder removed, branch merged or gone (section 1), topic Done or Dropped. A topic moved to another epic keeps its memories, which still say the same thing about the same topic; epic-level memories that topic used to see are not flagged.

**Filtering by current context.** The briefing and `kb` lists hide branch memories of other branches and count them in the header (`· 2 on other branches`); `kb(query)` searches them too, marked `?branch feature/x`. The briefing ranks memories in scope: matching branch first, then memories whose folder is the session's folder, then by level (topic, epic, project), then by verified date, newest first; superseded and retired are left out. When the 5-title budget is short, flagged titles give way last among equals (a stale fact you are about to trip on is worth seeing).

### 5. Provenance

Recorded automatically on every agent write, free of parameters: author (`created_by`, `updated_by`, already there), the Claude Code session id (`CLAUDE_CODE_SESSION_ID`, opaque), the session's focus topic (`created_topic_id`), the branch, and timestamps; on revisions too (`session`, `branch`), so "which session changed this" is answerable for every revision. Human writes record the author only.

Explicit sources: `refs` (one string, at most three items, comma-separated) on create and edit:
- an Acta id: `T-12`, `T-12#14` (an entry), `Q-3`, `N-5`, `K-2`;
- a commit: `commit:9e92ba4` (7 to 40 hex, checked to exist in the session's folder);
- a file: a path relative to the session's repository root (checked to exist; its blob id at `HEAD` recorded; it then drives the `file` flags of section 1).

Stored in `kb_links`; record references by UUID, so they survive export and import. An unknown id, missing commit or file is refused with the item named.

**Agent sees** one short segment, ids only: `from T-12 3d` in `kb` lines (the focus topic at creation, or the first record reference; the age is since creation); `kb_read`'s header gives the whole set: `from T-12#14, Q-3 · claude · 11-19 · branch feature/kb · files: src/store/kb.rs` (branch and paths are stored text: inside the fence, under the id line).

**Human sees** everything on the memory's meta row and the reading view's properties: who wrote and verified it and when, from which session (its start time and topic; presence rows are gone a day later, so the session is shown as "a Claude Code session on T-12, 19 Nov 14:05"), the source records as links, the commit, the files with their flags.

All stored text (paths, branch names, the retire reason) reaches the agent inside the per-response fence, as titles and bodies do today. Nothing here is written outside the encrypted database except section 6's ids in `presence.db`, which is the same SQLCipher file family.

### 6. Concurrent sessions

Optimistic revisions stay the mechanism. What changes is what the second writer is told and how often it must start over.

**Conflict reply.** A stale `rev` now names who changed what, in one block under 1,600 chars:
- memory (at most 800 chars): `not saved: K-4 changed since rev 7: rev 8 by another Claude Code session on T-51, 40 s ago (body replaced). Current text:` and the fenced body, so the agent reconciles in one call without a re-read;
- document: `... rev 9 by Alexandre 3 min ago: sections 2 and 5 changed (+12 -3 lines)` and the fresh outline, with `section 3 (yours) unchanged` when it holds.

**Merge assistance** (section-level three-way merge, the only automatic one): when the edit is `append_section`, or `replace_section` whose target section's text at the read revision is found exactly once and byte-identical in the current one (matched by heading path and content, not by number), the server applies it on the current revision and says so: `ok K-2 rev 10 · section 3 replaced · merged over rev 9 (Alexandre changed sections 2 and 5)`. Anything else, `replace_body` and title or summary changes included, is refused as above. The app's editor uses the same function: a stale save whose sections do not overlap is merged with a toast; one that overlaps shows a notice with "Compare" (Changes view of the history) and "Save mine on top" (theirs stays in history).

**Presence-aware hints.** The server records, in its own `presence.db` row, the last three knowledge ids it read or wrote with their revision and time (`kb_recent`; ids and numbers only). `kb_read` and the stale-rev reply then add one line when another live session touched the same entry in the last 10 minutes: `K-4 is also open in a session on T-51 (read 2 min ago)`. The app shows the same on the entry ("Claude Code is working with this memory on T-51") from the presence watcher it already runs.

**Candidate, not decided**: pushing "K-4 changed in another session" into the first session's context through the existing `PostToolUse` `mcp_tool` hook (which answers empty today). It needs the current hooks documentation checked for `additionalContext` on that event and one run proving it; until then the first session learns at its next write.

### 7. Tool surface and budgets

No new tool. Changes, all on `kb_write` and `kb`:

| Change | Cost in `tools/list` |
|--------|------|
| `op` gains `verify`, `retire` | about 15 chars |
| `level` gains `branch` | about 8 |
| `supersedes` (string: K ids) | about 60 |
| `refs` (string: ids, `commit:sha`, paths; max 3) | about 75 |
| `kb` output only (markers, counts) | 0 |

About 160 chars. Paid for in the same change by trims, measured before and after with the existing `tools/list` test, so the total does not grow: `kb_write`'s description drops "instead of your own memory files" (the instructions say it, about 35), `create`'s description is shortened (about 60), and the `section`/`heading` field descriptions merge into the `op` description (about 70). The test's 7,000 limit stays; the target after this change is at most 6,500. Instructions (800) are unchanged; the `using-acta` skill gains a "Memory quality" block of at most 10 lines (verify, reconcile, `supersedes`, `refs`, branch level, never trust a `?` memory blindly).

Output budgets stay: `kb` 1,800 (markers count against it; lines are cut at item boundaries as today), briefing 1,200 (the "to check" line and `?` marks give way after titles), acks 160 (the `related:` and `supersedes` segments fit), refusals 600 for duplicates and conflicts, 1,600 for stale revisions, `kb_read` 6,000. Tests assert each on a seeded database with pathological sizes (three 120-char refs paths, 40-char branch names, flags on every line).

CLI: `acta kb verify K-4 --rev 3`, `acta kb retire K-4 --rev 3` (reason on stdin), `acta kb write` takes `supersedes` and `refs` in its stdin JSON (paths are stored text: never in arguments), `acta kb check`, `acta kb quality`.

### 8. Metrics

Local, computed on demand, nothing leaves the machine. A local table `kb_quality_log` (event, ids, session, time; never exported, pruned after 180 days) records what history cannot: warnings issued (duplicate, conflict, related), their outcome (superseded, fixed, kept both, abandoned: no write within the session), stale-rev refusals and automatic merges. `acta kb quality [--days 30]` prints six lines from it and from the entries:

1. **Freshness**: share of live memories with no flag; count per flag.
2. **Human correction rate**: human edits, retires, deletes and merges of Claude Code memories per 100 Claude Code memory writes. The headline number: lower is better.
3. **Missed duplicates**: human merges and supersedes of pairs the checker never flagged, as a share of all human merges (detector recall; section 2's revisit trigger).
4. **Warning precision**: share of duplicate and conflict warnings the agent overrode with "keep both" (noise).
5. **Verification**: verifies per agent session, and the share of `?` memories read that were then verified, fixed or retired in the same session (does the agent act on flags).
6. **Concurrency**: stale-rev refusals, automatic merges, and refusals followed by a successful write within 2 calls.

The lead runs it on P-1 (Acta tracks itself) before and two weeks after each phase ships and records the numbers in this ADR's Evidence.

### 9. Migration

Schema 7, additive (schema.rs rules):
- `kb_entries`: `verified_at INTEGER`, `verified_by TEXT` (backfilled from `updated_at`, `updated_by`), `state TEXT NOT NULL DEFAULT 'live' CHECK (state IN ('live', 'superseded', 'retired'))`, `superseded_by INTEGER REFERENCES kb_entries(id)`, `context_branch TEXT`, `context_folder_id INTEGER REFERENCES folders(id)`, `created_session TEXT`, `created_topic_id INTEGER REFERENCES topics(id)`; index `kb_verified ON kb_entries(project_id, kind, verified_at) WHERE deleted_at IS NULL AND state = 'live'` (the Overview's count).
- `kb_revisions`: `session TEXT`, `branch TEXT`.
- `kb_links` (id, uuid, `entry_id`, `kind` in topic, entry, question, note, kb, commit, file; `target_uuid` for records, `target_text` for a commit or path, `folder_id`, `blob` (git blob id at the last verification), `created_at`, `created_by`).
- `kb_pairs` (`a`, `b` with a < b, `kind` duplicate or conflict, `state` open, distinct or resolved, `score REAL`, `created_at`, `created_by`, `resolved_at`, `resolved_by`; primary key `(a, b, kind)`).
- `kb_quality_log` (local).
- `presence.db`: `kb_recent TEXT` on the session row (its own additive step).

Export format version + 1: entries carry the new fields, `kb_links` and `kb_pairs` by UUID; session ids are not exported (local, meaningless elsewhere); a previous-version file imports with `verified_at = updated_at`, state live, no links or pairs. Tests for both directions (ADR 0006).

## Alternatives
- Local embedding model now: a download, a runtime and CI cost for an unmeasured gain; the agent already judges semantics. Revisit on the metric above.
- Asking Claude (`claude -p`) to deduplicate in the background: spends the user's plan quota without them asking, adds minutes of latency, and needs a process Acta would start on its own.
- Time-to-live or automatic expiry: deletes true facts that are simply old, against "never auto-delete".
- Storing flags: they go stale themselves; computing them at read time is cheap and always right.
- A file watcher or hashing the repository: that is the file-change tracking data.md rules out; reading `HEAD` for the few referenced paths on the page is enough.
- A separate `kb_check` or `kb_merge` tool: grows the surface; `op` values and two parameters carry the same actions.
- Refusing conflicts permanently until reconciled: blocks the agent on false alarms; one refusal then "keep both" is the existing, proven pattern.
- Auto-merging whole bodies (line-level three-way merge): silent wrong merges in prose; sections are the unit agents and humans already address.
- Locks or leases on entries: a crashed session would hold them; presence hints give the information without the failure mode.
- MinHash or locality-sensitive hashing: built for millions of items; exact Jaccard on 20 prefiltered candidates is faster and simpler.
- Confidence scores per memory: one more number nobody can calibrate; flags with a reason are actionable.

## Consequences
- Every memory write runs one FTS query and up to 20 set comparisons: target under 2 ms with 2,000 memories in scope, measured in a test (timing reported, not asserted).
- `kb` lists and the briefing may run one `git ls-tree` per folder (2 s timeout, cached per `HEAD`); a slow or absent git means no git flags, never a slower or failed answer. The session-start briefing reads git flags only from the cache the server warmed at registration, so `acta hook session-start` keeps its 50 ms target; it shows store flags only when the cache is cold.
- The fixture corpus becomes the contract for the checker: any threshold or word-list change re-runs it and reports precision and recall.
- `supersedes` and `retire` give the agent a way to hide a memory from itself. The human sees every one (filter, unread dot, history) and restores in one step; metric 2 watches whether it is abused.
- The Overview copy and the Knowledge page additions need the product designer (components-knowledge.md, components-records.md 5, catalogue); this ADR fixes behaviour, not layout.
- Revisit: the embedding model (section 2 trigger); the `PostToolUse` push (section 6); `old` for documents if dogfooding shows stale documents misleading the agent.

## Amendment
- 2026-11-22 (phase 2, accepted by the lead): the thresholds of section 3 are the ones the fixture fixed, not the provisional ones: duplicate `s >= 0.45`, related `0.30 <= s < 0.45`, conflict overlap `s >= 0.30`. Polarity is a signal only when one side alone negates and the negation governs, within its clause, a content word the other side says (four tokens after an English marker; also two before a French one or a prohibition); "same" also requires no conflict signal (stop-word removal otherwise hides "not").

## Evidence
- 2026-11-22, phase 2 (`integration-engineer/memory-quality`): the labelled fixture (`acta-core/src/kb_quality/fixture.rs`, 88 pairs, 44 English and 44 French, labelled before the first run). With the provisional thresholds of section 3 (duplicate 0.70, related 0.45) duplicate recall was 0.29: paraphrases scored `s` 0.38 to 0.90, related and distinct pairs at most 0.38. With the constants of the amendment below: duplicates precision 0.96, recall 0.89; conflicts precision 0.91, recall 0.88. Misses: two duplicates whose "do not bump it" reads as a conflict, a conflict whose side says both "activer" and "désactivés", two clippy-policy conflicts with no lexical signal, one French duplicate at 0.44. `kb_check` over 2,000 memories: 7.6 ms per write in a debug build (release not measured).

Before phase 2: none, a design decision. Proofs planned in PLAN.md "Memory quality": the labelled fixture's precision and recall, the stdio end-to-end tests, budgets, and at most four headless Haiku runs (conflict, stale file, concurrent edit, duplicate) whose transcripts' tool calls are checked for the expected reconcile.
