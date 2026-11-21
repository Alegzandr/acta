# 0005 - Claude Code integration

Status: accepted (install path, `mcp_tool` hooks on SessionStart/SubagentStart and macOS/Linux to be proven in M3)
Date: 2026-11-20

## Context
Acta must give Claude Code a scoped memory (topic, or epic when coordinating) through an MCP server and a CLI in the app binary, plus hooks, a skill and a one-step setup. Responses must never bloat the context. ADR 0006 keeps export/import/snapshots out of MCP; ADR 0007 adds one knowledge base per project, epic and topic (entries of kind document or memory), pulled on demand, and makes Acta the place for the agent's durable memory.

Facts checked on 2026-11-20 against Claude Code 2.1.329 (Windows 11) and the current docs:

| Fact | Source |
|------|--------|
| `rmcp` 3.11.0 (2026-11-17, MSRV 1.88, a release roughly every week) implements MCP revision 2026-07-28 and older. `#[tool_router]`, `#[tool]`, `#[tool_handler(name, version, instructions)]`; schemas from `schemars` (draft 2020-12). | crates.io, rust-sdk README |
| Claude Code probes `server/discover` (2026-07-28), then falls back to `initialize` with 2025-11-25. | spike |
| Tool search is on by default: only tool names and server `instructions` load at session start; schemas load on demand. Instructions and upfront descriptions are cut at 4,096 chars. `_meta["anthropic/alwaysLoad"]: true` loads one tool upfront. | docs/mcp, spike |
| MCP tools prompt for permission in default mode even with `readOnlyHint: true`; `readOnlyHint` only enables parallel calls and safe retry. Allow rule for a plugin server: `mcp__plugin_<plugin>_<server>__*`. | spike, docs/permissions |
| When a result carries `structuredContent`, the model receives only that JSON; the text block is dropped. | spike |
| Output: warning at 10,000 tokens, saved to a file above 25,000 tokens or 50,000 chars. | docs/mcp |
| A stdio server gets `CLAUDE_PROJECT_DIR` (the launch directory, not the git root) and `CLAUDE_CODE_SESSION_ID` (fixed at spawn, not updated on `/clear`), and inherits the shell environment. `tools/call` carries only `claudecode/toolUseId`: calls from subagents share the parent connection and cannot be told apart. | docs/env-vars, spike |
| Hooks: 33 events. Common stdin fields `session_id`, `transcript_path`, `cwd`, `hook_event_name`, `agent_id`, `agent_type`. Context is injected with `{"hookSpecificOutput":{"hookEventName":..,"additionalContext":..}}`, capped at 10,000 chars. Exec form (`command` + `args`) spawns the executable with no shell; shell form uses Git Bash on Windows, PowerShell without it. | docs/hooks, spike |
| `mcp_tool` hooks call a tool on a connected server (about 5 ms, no process). They are skipped for `SessionStart` at launch, run after `/clear` and compaction. Their output reaches the model only as hook JSON, not as plain text. | docs/hooks, spike |
| `PreCompact` stdout becomes instructions for the compaction summary. `Stop` fires every turn; `SessionEnd` has 1.5 s and cannot inject. | docs/hooks |
| Custom slash commands are merged into skills. `SKILL.md` = YAML frontmatter + Markdown; `description` + `when_to_use` cut at 1,536 chars; after compaction the first 5,000 tokens of an invoked skill are re-attached. Plugin skills are `/plugin:skill`. | docs/skills |
| Claude Code's auto memory (Markdown files under `~/.claude/projects/<repo>/memory/`, first 200 lines or 25 KB of `MEMORY.md` loaded every session, shared by all worktrees of a repository, not loaded into subagents) is on by default. It can be turned off per project with `"autoMemoryEnabled": false` in project or local settings, or globally with `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1`. `autoMemoryDirectory` only moves the files; it cannot point at an MCP server. | docs/memory |
| A plugin bundles skills, hooks (`hooks/hooks.json`) and MCP servers (`.mcp.json`). A plugin listed by relative path in a marketplace added from a local directory loads in place at every session start, whatever its version. A plugin cannot ship permission rules. `--strict-mcp-config` drops plugin servers. | docs/plugins, spike |

## Decision

### 1. Setup: a generated local plugin, plus one allow rule
`acta setup` (also a button in onboarding) does three things, each idempotent:

1. Writes the plugin tree from files embedded in the binary to `<data dir>/claude/` (`%LOCALAPPDATA%\Acta`, `~/Library/Application Support/Acta`, `~/.local/share/acta`): `.claude-plugin/marketplace.json` (marketplace `acta`), `plugins/acta/{.claude-plugin/plugin.json, .mcp.json, hooks/hooks.json, skills/}`. The absolute path of the Acta executable is written into `.mcp.json` and `hooks.json`, so nothing depends on `PATH`.
2. Runs `claude plugin marketplace add <dir>` and `claude plugin install acta@acta` (user scope). Claude Code owns its own settings writes.
3. Adds the single string `mcp__plugin_acta_acta__*` to `permissions.allow` in `~/.claude/settings.json` (parse, append if absent, atomic write, backup). Without it every tool prompts. `--no-allow` skips it.

`acta setup --remove` runs `claude plugin uninstall acta@acta`, `claude plugin marketplace remove acta`, removes that one string and the directory. `acta setup --check` reports each step. **Auto memory.** It cannot be redirected to Acta, and instructions alone would leave two memories competing, with the built-in one prompted from the system prompt. So `acta link` (and the UI when a root is linked) offers, on by default and stated plainly, to write `"autoMemoryEnabled": false` into that repository's `.claude/settings.local.json` (per user, not committed), and `acta link --remove` takes it out. The skill carries the routing rule either way, for users who decline. An existing `MEMORY.md` is not migrated automatically; `acta kb import <file> --kind memory` loads it for the human to split and file.

On every app update the app rewrites the plugin tree; because the plugin loads in place, the next session picks it up with no `claude` call and no version bump.

### 2. Server
`acta mcp`: stdio, `rmcp`, tools only. No resources, prompts, elicitation or `list_changed`. Results are plain text, never `structuredContent` and no `outputSchema`. Read tools carry `readOnlyHint`, writes `idempotentHint: false`. `brief` and `log` are `alwaysLoad`; the rest are found by tool search, so names are plain words and each description starts with when to call it. `instructions` stays under 800 chars:

> Acta is this project's tracker and your working memory: topics (T-n) in epics (E-n), each with a current state, a timeline and a knowledge base (K-n: documents and memories). The session briefing names your focus topic. Call `brief` when unsure where things stand. After a meaningful step or a decision call `log`, and refresh the state in the same call. Save durable facts with `kb_write` instead of your own memory files; check `kb` before working on an unfamiliar area. Tools default to the focus topic; pass an id to address another. Subagents always pass ids. Do not copy code or long output into Acta.

### 3. Tools (9 for the model, 1 for hooks)
Three families of three: orientation and timeline (`brief`, `entries`, `log`), structure (`update`, `find`, `create`), knowledge (`kb`, `kb_read`, `kb_write`).
Ids are short per-installation aliases (ADR 0006): `T-42`, `E-7`, `N-13`, `K-2` (knowledge entry of either kind), timeline entry `T-42#14`. Costs are for the examples, at 4 chars per token.

| Tool | Parameters | Returns | Budget |
|------|------------|---------|--------|
| `brief` | `id?` (T/E, default focus), `focus?: bool` | Topic: state + last 3 entries + counts. Epic: one line per topic. No focus: project overview. `focus: true` makes `id` the session focus. | 300 tok |
| `entries` | `topic?`, `type?`, `before?`, `limit=5` (max 20), `id?` | Timeline page, newest first, 240 chars per entry; `id` returns one entry in full. | 500 / 1,500 |
| `log` | `type` (progress, decision, discussion), `text` (max 1,500 chars), `state?` (max 600), `status?`, `topic?` | One-line ack. | 40 |
| `update` | `id` (T/E/N), `title?`, `description?`, `state?`, `status?`, `epic?` | One-line ack. The previous state is kept in history. | 40 |
| `find` | `query?`, `kind?` (topic, epic, note, entry), `status?`, `epic?`, `limit=8`, `cursor?` | One line per hit. `kind: note` without a query is the inbox, 200 chars per note. | 500 |
| `create` | `items[]` (max 10): `kind` (topic, epic, note), `title`, `description?`, `epic?`, `from_notes?` | Ack with new ids; `from_notes` marks those notes converted and links them. | 80 |
| `kb` | `query?`, `kind?` (document, memory), `id?` (scope, default focus), `cursor?` | Knowledge in scope, topic then epic then project, 10 per page: id, kind, level, size, date, summary. With `query`: 5 excerpts of 160 chars with entry and section. | 450 |
| `kb_read` | `id`, `section?`, `lines?` | No selector: whole body if under budget (every memory is), else the outline. `section` is the outline number. | 1,500 |
| `kb_write` | New: `kind`, `title`, `summary` (max 120 chars), `text`, `level` (topic, epic, project; default topic) or `owner` id. Existing: `id`, `rev`, `op` (replace_body, replace_section, append_section), `section?`, `heading?`, `text`, `summary?`, `level?` | One-line ack with id and revision. A memory body is capped at 800 chars. A stale `rev` is refused with the fresh outline. A new memory whose title or summary closely matches an existing one in scope is refused once, naming it. | 40 |
| `hook` | `event`, `session`, `agent?` | Hook JSON. Described as internal; not for the model. | 300 |

```text
brief                                                                        ~150 tok
T-42 Encrypted storage spike · doing · E-3 Storage (2/5 done) · focus
State (11-19, claude): SQLCipher via bundled rusqlite works on Windows; key in Credential Manager. Next: WAL and a 3-process contention test.
#14 11-19 progress claude    WAL on; GUI and CLI write concurrently, 1k iterations pass
#13 11-19 decision claude    Raw 32-byte key, no KDF: open 2 ms instead of 310 ms
#12 11-18 discussion Alexandre  Prefers one file over one database per project
14 entries · knowledge in scope: 3 documents, 11 memories · inbox 4

entries(before=13, limit=2)                                                  ~70 tok
T-42 entries 11-12 of 14 · older: before=11
#12 11-18 discussion Alexandre  Prefers one file over one database per project
#11 11-18 progress claude    Compared sqlx and rusqlite on build time and... [+310 chars, id="T-42#11"]

log(type="progress", text="...", state="...")                                ~12 tok
ok T-42#15 progress · state updated

find(query="keychain")                                                       ~60 tok
T-42 doing  Encrypted storage spike - WAL on; contention test next
T-42#13 decision  ...key in the OS keychain, raw 32 bytes, no KDF...
N-9 note  Ask whether the keychain prompt on macOS is acceptable at every launch
3 hits

create(items=[{kind:"epic",title:"Notes inbox"},{kind:"topic",epic:"E-4",title:"Quick capture",from_notes:["N-3","N-5"]}])   ~20 tok
ok E-4 · T-51 (notes N-3 N-5 converted)

kb                                                                           ~130 tok
Knowledge in scope of T-42: 3 documents, 11 memories · 1-4 of 14 · more: cursor=4
K-7  doc topic    Storage spike results · ~900 tok · 11-19 - Open and write latency of each SQLCipher option, with the numbers behind the raw-key choice.
K-19 mem topic    Contention test needs --test-threads=1 · 11-19 - The 3-process test deadlocks when cargo runs tests in parallel.
K-4  doc epic     Storage requirements · ~1.4k tok · 11-15 - What storage must guarantee: encryption, multi-process access, snapshots.
K-2  doc project  Architecture · ~3.2k tok · 11-12 - Crates, processes and data flow; read before changing structure.

kb_read(id="K-2")                                                            ~70 tok
K-2 Architecture · doc · rev 7 · ~3.2k tok, over the 1.5k budget; outline:
1 Overview 210 · 2 Crates 640 · 3 Processes 520 · 3.1 MCP server 300 · 4 Data flow 880 · 5 Open questions 650
Read one with section=<n>.

kb_write(kind="memory", level="project", title="Clippy runs with -D warnings", summary="CI fails on any clippy warning; run it before proposing a commit.", text="...")   ~12 tok
ok K-21 memory · project · rev 1

kb_write(id="K-2", rev=7, op="replace_section", section=3, text="...")       ~15 tok
ok K-2 rev 8 · section 3 replaced (520 -> 410 tok)
```

Sections are addressed by outline number, valid for the revision the agent read; that settles duplicate headings. An entry without headings is read by `lines`. Every write keeps the previous revision, and the human can edit, move or delete anything the agent filed (ADR 0007).

Left out: delete of anything (human only, in the UI), reorder, cross-project access, export/import/snapshots (ADR 0006), knowledge history and restore, a separate tool family per kind (one family with a `kind` filter, so the agent never guesses where an answer lives), bulk reads of any kind, MCP resources and prompts (skills do the job, and `@` mentions would paste whole bodies).

**Budget enforcement.** One renderer in the domain layer produces every agent-facing text and takes a hard character budget; lists are cut at item boundaries with `+N more` and a cursor. No response exceeds 2,000 tokens (8,000 chars). Writes over a field limit are refused with the limit and the actual size. Tests: for every tool with default parameters, on a seeded database containing pathological sizes, assert `chars <= budget`; the stdio end-to-end test asserts the same on real `tools/call` results and that `tools/list` stays under 7,000 chars and `instructions` under 800.

### 4. CLI
The CLI calls the same domain functions and the same renderer; `--json` is for scripts.

| CLI only | CLI and MCP | MCP only |
|----------|-------------|----------|
| `setup`, `link [project]`, `mcp`, `hook <event>`, `export`, `import`, `snapshot`, `kb import/export <file>` | `brief`, `entries`, `log`, `update`, `find`, `create`, `kb`, `kb read`, `kb write` | none |

Free text comes from stdin or `--file`, never from arguments (CLAUDE.md rule on process arguments). The author is `claude` over MCP and when `CLAUDECODE=1`, otherwise the human.

### 5. Binding
- **Project.** The git common directory of the launch directory resolves to a repository root, so all worktrees map to one project. Roots are linked to projects in the UI or with `acta link`. `ACTA_PROJECT` overrides. An unlinked directory gets no briefing and tools answer with one line saying so.
- **Focus topic.** Held per MCP server process, which is one Claude Code instance. At start it resolves: `ACTA_TOPIC`, then the focus last saved for `CLAUDE_CODE_SESSION_ID`, then the focus last used on this worktree and branch, then none. `brief(id, focus: true)` changes it and saves it under all three keys. Hooks apply the same order with the `session_id` they receive; after `/clear` the `mcp_tool` hook hands the new session id to the server.
- **Explicit ids always win and never move the focus.** That is the rule for subagents, which share the parent's connection.
- **Parallel sessions** are separate server processes with separate focus. Entries are append-only; a state overwrite keeps the old state in history, so concurrent writers lose nothing.
- **Embedded terminal** (M4) sets `ACTA_PROJECT`, `ACTA_TOPIC` when opened from a topic, `ACTA_EPIC` when opened from an epic, and `ACTA_TERMINAL=1`.

### 6. Hooks
Each `command` hook spawns Acta, which needs the keychain and the database. To keep that off unrelated repositories, `acta hook` first checks the launch directory against a file of salted hashes of linked roots and exits silently on a miss, before touching the keychain.

| Event | Matcher | Handler | Effect |
|-------|---------|---------|--------|
| `SessionStart` | `startup\|resume` | `command`, exec form: `acta hook session-start` | Briefing as `additionalContext`, at most 1,200 chars: the `brief` text, the knowledge counts, and up to 5 memory titles. Once per launch. |
| `SessionStart` | `clear\|compact` | `mcp_tool` -> `hook` | Same briefing from the running server; restores orientation after compaction. About 5 ms. |
| `PreCompact` | all | `command`: `acta hook pre-compact` | Prints fixed text, no database: keep the focus id and list decisions and progress not yet logged. The post-compaction briefing asks the agent to log them. |
| `SubagentStart` | all | `mcp_tool` -> `hook` | At most 300 chars: project, epic, parent focus, "pass ids explicitly, log findings". |

**Memory titles in the briefing.** No ranking model and no query: take memories in scope ordered by level (topic, then epic, then project) and, inside a level, by last update, newest first; print `K-id title` (titles cut at 60 chars) until 5 are listed or the 1,200-char budget is reached, whichever comes first. State and recent entries are written first, so titles are what gets dropped when space is short. Counts are always printed; bodies and summaries never. Example tail of a briefing (about 45 tokens):

```text
Knowledge in scope: 3 documents, 11 memories. Recent memories: K-19 Contention test needs --test-threads=1 · K-17 Credential Manager entry name · K-21 Clippy runs with -D warnings
```

Rejected: `Stop` (every turn, costs a model turn each time it speaks, nags), `UserPromptSubmit` and `PreToolUse`/`PostToolUse` (latency on the hot path for nothing the skill cannot do), `SubagentStop`, `PostCompact`, `SessionEnd` (cannot inject; nothing worth recording without the model). A `PostToolUse` hook filtered with `if: "Bash(git commit *)"` that links commits to the focus topic is a candidate for later, once dogfooding shows the need.

### 7. Skills
Slash commands are skills; the plugin ships three, in `plugins/acta/skills/<name>/SKILL.md`:

| Skill | Invoked by | Content |
|-------|-----------|---------|
| `acta:using-acta` | model | The working rules below. Under 60 lines. |
| `/acta:triage` | user | Read the inbox with `find(kind="note")`, group notes, propose epics and topics, create them in one `create` call after the user agrees. |
| `/acta:wrap` | user or model | Log what was done, refresh the state, set the status. |

`using-acta` teaches: read the briefing, call `brief` only when it is missing or stale, read `entries` only to answer a specific question; log after a decision, a finished step, a dead end worth not repeating, never per file edit; a state says what is true now and what comes next, in at most four lines, with no history; when delegating, put the topic id in the prompt. For knowledge it teaches:

- **Which text goes where.** State: where things stand. Entry: what happened and why. Memory: one durable fact worth not relearning (convention, pitfall, preference, constraint, where something lives), a few lines. Document: long-form reference that is re-read (spec, architecture, research result).
- **Routing.** Durable learnings go to Acta with `kb_write`, not to Claude Code's memory files, so the human sees and can correct them. Instructions the human writes for every session stay in CLAUDE.md.
- **Filing rule.** File at the narrowest level where the fact stays true: one task's quirk on the topic, a feature area's rule on the epic, a codebase convention on the project. One fact per memory. Search first (`kb(query=...)`); update the existing entry rather than add a near-duplicate.
- **Reading.** Before touching an unfamiliar area, call `kb`; open an entry only when its summary matches the task, and a long one by section.
- **Summary.** One sentence saying what the entry answers and when to read it; never the title again.

**Amendment (M6, notes triage).** `create` is also how an agreed triage is applied, in one transaction: items take `kind: entry` with `topic` (each note becomes a discussion entry of an existing topic, keeping its author and text), `epic: "@k"` names an epic item of the same call, `from_notes` works on epics too, and a top-level `dismiss` list closes notes without filing them. Every note must be open, in the project and used once; a bad item refuses the whole call. Topics made from notes start as draft, and an item without a description takes the notes' text. At most 40 notes per call; the ack names each new id and its notes, and drops the note lists if over budget. `update(id="N-n", status="open" | "dismissed")` puts a note back in the inbox or dismisses it. No new tool.

## Alternatives
- Editing `~/.claude.json` and `settings.json` ourselves for the server and hooks: breaks on format changes, hard to remove cleanly, and hooks would mix with the user's own.
- `claude mcp add --scope user` plus hooks in settings: half the plugin, still needs our own settings edits for hooks and skills.
- Marketplace in the GitHub repository: third-party marketplaces do not auto-update by default, need git, and can drift from the installed binary. The local directory always matches it.
- Project `.mcp.json`: asks every collaborator for approval and puts a machine path in the repository.
- JSON or `structuredContent` results: about 30 to 40 percent more tokens for the same content, and the text form is dropped when both are sent.
- One tool per verb (about 20), or one `acta(action, ...)` tool: the first floods tool search, the second hides the schema from validation and from permission rules.
- Focus keyed only by session id: wrong after `/clear` for the server. Keyed only by branch: wrong for two sessions in one checkout.
- Auto memory left on and out-instructed by the skill alone: two stores, and the built-in one is prompted from the system prompt. Disabling it globally: affects repositories Acta does not track. Pointing `autoMemoryDirectory` at an Acta-managed folder: plaintext files on disk, against CLAUDE.md.
- Hooks for all context: a `Stop` nudge was the obvious candidate and is rejected above.

## Consequences
- One direct edit of user settings remains (the allow rule). It is a single string and reversible; setup must say so.
- The plugin tree holds an absolute executable path. ADR 0004 makes it stable: the CLI and MCP server are a separate console executable exported as `acta` to a per-user path outside the install root, and the plugin tree references that path, never the install directory.
- Hook latency depends on ADR 0003: opening the database must not run a slow KDF (0003 uses a raw key, no KDF). Target 50 ms for `acta hook session-start`, measured in CI.
- `rmcp` moves weekly and the MCP revision is four months old: pin with `Cargo.lock`, keep the transport behind our own trait, cover it with the stdio end-to-end test.
- Revisit when plugins can declare permissions, when Claude Code exposes the caller (subagent) to servers, or if dogfooding shows agents not logging without a nudge.

## Evidence
Spike in `spikes/integration/` (Windows 11, Claude Code 2.1.329, five headless runs on Haiku, about $0.02 in total): `plugin/` is a throwaway plugin with a Python stdio server and hooks; `rmcp-check/` is a minimal `rmcp` server.

Verified by running:
- `rmcp` 3.11.0 with `default-features = false, features = ["server", "macros", "schemars", "transport-io"]` builds (Rust 1.97.1), answers `server/discover` and `initialize`, emits `annotations.readOnlyHint` and draft 2020-12 schemas, returns a `String` as text only.
- Plugin loaded with `--plugin-dir`: exec-form `SessionStart` hook ran with no shell (59 ms for Python), its `additionalContext` reached the model; server `instructions` reached the model; `alwaysLoad` tool was loaded, the others deferred by name.
- Server environment: `CLAUDE_CODE_SESSION_ID`, `CLAUDE_PROJECT_DIR` equal to the launch subdirectory, shell environment inherited. `tools/call` meta holds only `claudecode/toolUseId` and a progress token.
- Both tools denied without an allow rule, including the read-only one; `--allowedTools "mcp__plugin_acta-spike_acta__*"` allowed them.
- `structuredContent` replaces the text block for the model.
- `mcp_tool` hook on `UserPromptSubmit`: called the plugin server in about 5 ms with `${session_id}` substituted and no permission needed; plain-text output did not reach the model, hook JSON did.
- `claude plugin validate` passes on the spike plugin.

Read in the docs, not run: auto memory settings (`autoMemoryEnabled` per project, no redirect to a server); in-place loading from a local-directory marketplace and the install/uninstall commands (running them would modify `~/.claude`); `mcp_tool` hooks on `SessionStart` after `/clear` and compaction and on `SubagentStart`; `PreCompact` stdout as summary instructions; session id behaviour on `/clear` and `--resume`; output limits.

Assumptions: setting `_meta` through the `rmcp` macros (otherwise patch `list_tools`); interactive sessions behave like `-p` for hooks and permissions; token figures use 4 chars per token; nothing was run on macOS or Linux.

Sources: code.claude.com/docs/en/{hooks, mcp, skills, sub-agents, permissions, env-vars, memory}, code.claude.com/docs/en/plugins/{manifest-reference, components, loading, create-marketplace, host-marketplace, cli-reference}, crates.io/crates/rmcp, github.com/modelcontextprotocol/rust-sdk (README, crates/rmcp, crates/rmcp-macros), docs.rs/rmcp-macros/3.11.0, modelcontextprotocol.io/specification/2026-07-28.

## Amendment (2026-11-21): session presence, plan usage, setup that links

Checked against Claude Code 2.1.330 (`claude --help`) and code.claude.com/docs/en/{hooks, statusline, sessions, cli-reference, plugins/loading, plugins/manifest-reference}.

**Session presence** (data.md request 2). Each `acta mcp` process keeps one row for its session in `presence.db`, a second SQLCipher file beside `acta.db`, opened with the same key, local, never exported and not in snapshots. A separate file because every commit to `acta.db` reloads every page through the change watcher; heartbeats must not. A key that does not open it (after key-loss recovery) deletes and recreates it: it is rebuilt within a heartbeat.
- Row: opaque Claude Code session id, project and focus ids, the `ACTA_LAUNCH` token of the Acta terminal that started it, start, last activity, heartbeat, state (working, waiting, idle, ended), the short name of the last tool, and one line (100 chars) of the agent's last `log` text. Never a prompt, a tool's input or output, or file content.
- Written only by the server: registered at start (on the blocking pool, so `initialize` never waits on the keychain), a heartbeat every 30 s, ended when stdin closes. Stale after 90 s without a beat, deleted a day later. A session in a folder the pre-check file does not know never opens the database for presence (same rule as the hooks).
- New `mcp_tool` hooks call the internal `hook` tool, which answers an empty text for them so nothing reaches the model: `UserPromptSubmit` (working), `PostToolUse` with `detail: "${tool_name}"` (working, last tool; same state within 15 s is not rewritten), `Notification` matched `permission_prompt|elicitation_dialog|elicitation_url_dialog|agent_needs_input` (waiting), `Stop` (idle). Each has `timeout: 5`. No `SessionEnd` hook: run on 2.1.330, Claude Code refuses `mcp_tool` hooks there ("not available for the 'SessionEnd' hook event (no MCP client context)"), and a command hook would open the keychain inside its 1.5 s budget; the server ends the session when its stdin closes, and on `/clear` the process goes on under the new id that the `clear` briefing hook hands over. `${path}` substitution from the hook input is documented for `mcp_tool` inputs. This revisits the rejection of `Stop`, `UserPromptSubmit` and `PostToolUse` in section 6: those were rejected for speaking to the model or adding latency; these never speak and cost one in-process call. `SubagentStart` also marks the session working.
- `acta sessions` lists live sessions (one line each, under the agent budget).
- The app watches `presence.db` (500 ms poll of `data_version`, a full read every 15 s for expiry) and shows the sessions in the status bar, the tree, the record pages, a Sessions view and the terminal header (components-app.md 5, Claude Code sessions).

**Plan usage.** The only documented source of the plan's limits is the status line command's stdin (`rate_limits.five_hour` and `.seven_day`: `used_percentage`, `resets_at`; Pro, Max and Ultra, after the first answer; either may be absent). `acta statusline` reads it, keeps only those numbers and a time in `usage.json` in the data directory (atomic write, no database, no keychain), and prints the focus id (from `ACTA_TOPIC`/`ACTA_EPIC`) and "5h 42% · 7d 18%" for Claude Code. No endpoint is called, no Claude Code file is read, no token is touched. A plugin cannot declare a status line (nothing in the plugin manifest reference), so this is the second direct edit of user settings:
- `acta setup` sets `statusLine.command` to the exported `acta` with forward slashes (Claude Code runs status lines through Git Bash, else PowerShell, on Windows; a path either shell would split falls back to `acta` on `PATH`). A status line the user already had is kept whole in setup's record and run first by `acta statusline` with the same stdin (5 s limit, its output kept whatever its exit status, collected against the same deadline), Acta's segment appended to its first line; its other fields (`padding`, `refreshInterval`) stay. Any Acta status line (an `acta` or `acta-cli` executable at any path, run with `statusline`) counts as ours: setup rewrites its command and keeps the recorded original, and an Acta command is never recorded or run as the user's original. The user's command runs with `ACTA_STATUSLINE_NESTED=1`, and a nested `acta statusline` never runs it again. On a timeout the whole process tree is killed: a job object (`KILL_ON_JOB_CLOSE`) on Windows, the process group elsewhere. On Windows the shell is Git Bash (`CLAUDE_CODE_GIT_BASH_PATH`, next to a `git.exe` on `PATH`, or the usual install folders), else PowerShell. `--remove` puts the user's object back exactly, or removes the key; a status line changed after setup is left alone. `--no-statusline` skips it and is recorded, so `--check` reports it as skipped, not missing.

**One-click setup.** The app's "Set up Claude Code" asks first, saying how many folders of the open project it will touch, then runs `acta setup` and `acta link --attached --project P-n --add`: the attached folders of that project only, read from the store (no folder path on a command line), which refreshes the pre-check file and turns Claude Code's auto memory off there, as `acta link` always did. Nothing is attached or reordered. `acta setup --remove` turns auto memory back on in every folder where Acta turned it off (its local record of what it wrote), and leaves the others. `acta setup --files-only` writes only the plugin tree; the app runs it at launch so the plugin always matches the running build.

Evidence (Windows 11, this branch): unit tests for presence (expiry, rename, throttling, unbound sessions), usage parsing and staleness, the status line wrapper (stdin forwarded, output kept on a non-zero exit, a hanging command killed), setup and removal of the status line; end-to-end over stdio: presence registered, moved by hooks, activity from `log`, `/clear` handover, ended on close, nothing for an unlinked folder; `acta statusline` writing usage and wrapping a recorded command. Not run: an interactive Claude Code session firing these hooks for real (the hook event names, matchers and `${...}` fields are taken from the docs).

## Changelog
- 2026-11-21: ADR 0011 adds a `PreToolUse` guard (`acta hook pre-git`, scoped with `if` to `git commit` and `git push`), a folders and commit line in the session-start briefing (at most 300 chars, before memory titles), a "Commits" section in `using-acta`, and moves the `autoMemoryEnabled` writer into one ledger-backed settings writer in `acta-core`.
- 2026-11-21: due dates (docs/design/data.md request 16). `update` takes `due` (`YYYY-MM-DD` sets, `""` clears) on T, E and P ids (a P id takes nothing else) and refuses it on N ids and any other text (`due must be YYYY-MM-DD (2000-2099) or "" to clear, got "friday"`); the ack shows the date and its distance. `find` takes `due: overdue | week` (open topics and epics due before today or within 7 days, by date, an epic before the topics of its day; not with a query or on notes and entries). `create` topic and epic items take `due`, validated with the rest of the call. `brief` adds the due segment to topic, epic and project headers and the short one to the list lines of open records. CLI: `acta update --due`, `acta find --due`, and `--json` on both with `due` and `overdue`. Today is the local date of the `acta` process.
- 2026-11-21: Overview (docs/design/data.md request 17). `brief` of a project adds one `Attention:` line (blocked, overdue topics and epics, idle, inbox past 7 days; at most 160 chars, cut fairly at item boundaries with `+N`) right after the header, replacing request 16's `Overdue:` line, which never shipped; `brief` of an epic the same line without the inbox, at most 120 chars; an idle Active topic's header ends with `· no activity Nd`. Computed by the store's `overview_list` so the agent and the UI agree. No new tool or parameter.
- 2026-11-22: `tools/list` trimmed from 6,877 to 5,849 chars before the Questions step: tool descriptions say when to call without a "Call to" prefix, field descriptions the tool description or the instructions already give are gone, the update status list points at `log`'s, and `create`'s item type is inlined (no `$defs`). The `Attention:` line reads the scope once (`overview_lists`). `acta find --json` is refused under `CLAUDECODE=1` (titles in JSON are not fenced; Claude Code reads the text output); `acta update --json` carries no stored text and stays. The session-start briefing's folders and commit lines landed (ADR 0011).
