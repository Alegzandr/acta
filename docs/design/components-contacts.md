# Acta design: contacts and your profile

Part of the Acta design system. [DESIGN.md](../../DESIGN.md) is the index: direction, principles and the never-list; the review checklist is [checklist.md](checklist.md). Section numbers are shared across all parts.

## 5. Components (continued): contacts and your profile

Decided by the user on 2026-11-22: the people work is asked of or reported to are **contacts**, records in a contact list, with optional details (job title, company, how to address them); the human configures their own name and job title; and **a person is always a contact**: no field anywhere in Acta holds a free-text person. This replaces the free-text person value, the person field and the person page of [components-questions.md](components-questions.md) (2026-11-21). Model and agent contract: data.md 11, requests 19 (amended), 22 and 23. Copy: catalogue 10.4.11. Wireframes: screens.md 8.23, 8.28 to 8.30.

States, the focus ring and the controls used here (buttons, inputs, rows, menus, notice, empty state) are in [components.md](components.md); the properties block is in [components-records.md](components-records.md). No new token, icon size or colour pair, and two new Lucide icons, `users` (the Contacts row, its tab and palette entry) and `user` (a contact in the trash and in the palette): every text/surface pair below is one already measured in 2.2 (see "Contrast" at the end).

### The rule: a person is always a contact

Every field that names someone is a reference to a contact record, chosen with the contact picker below; never text typed into the field. Audit of 2026-11-22, every place Acta names a person:

| Where | What it holds |
|---|---|
| A question's Asked (components-questions.md 5) | A contact. |
| Report to on a note, a topic, an epic (components-questions.md 5) | A contact. |
| The inbox's Report to filter, the topic table's Report to column, the Overview | Contacts (the Overview names no one). |
| System events "noted to report to Marie" (request 10) | The contact's id, rendered with its current name. |
| Triage's plan (`→ Marie`), `find(person=...)`, `brief` segments | Contacts, by id or by a name that resolves to exactly one (data.md request 22). |
| Author of a write (timeline, notes, knowledge, questions, files) | Not a person field: Claude Code, or the human, named by **your profile** (below). |
| "Commits as {name}" in a folder's settings (ADR 0011) | Git's own `user.name`, read and shown, never stored or written by Acta. Not Acta's data, so not a contact. |
| Text a person or Claude Code writes (note bodies, entries, documents, answers, file names) | Content. It may mention anyone; Acta never parses names out of it (no `@name`). |

Nothing else in the specs names a person (entries, documents, attachments, knowledge, folders, sessions and the chat carry no person field).

### Naming

| Concept | English | French | Why |
|---|---|---|---|
| The record | **Contact**, **contacts** | **Contact**, **contacts** (masculine) | The user's word. Covers a person, a team or an office: the name is whatever the human calls them ("Legal team", "City hall, permits desk"). |
| The page listing them | **Contacts** | **Contacts** | |
| How to address them | **How to address them** | **Comment s'adresser à ce contact** | "Them" and *ce contact* fit a person and a team alike, with no gender. |
| Its level | **All projects**, **Project Acta**, **Epic Terminal** | **Tous les projets**, **Projet Acta**, **Epic Terminal** | "Global" is the model's word; the interface says what it means. |
| You | **Profile** (Settings section) | **Profil** | |

Asked, Report to and their French forms are unchanged (components-questions.md 5, Naming). "Person" stays the word in sentences about someone ("Change person..."); "assignee", "owner", "stakeholder" stay banned (copy.md 7).

### Contact record

| Field | Rule | Shown |
|---|---|---|
| Name | Required. 1 to 80 characters, trimmed, inner whitespace collapsed, stored as typed. Its **key** (case folded, accents removed, whitespace collapsed: `person_key` of request 19) is used to search and to spot duplicates, never shown. Two contacts may share a name (two Maries). | Everywhere the contact is named. |
| Job title | Optional, at most 80. | The contact page; next to the name in the picker and the contacts list; Claude Code's `brief` and `find`. |
| Company | Optional, at most 80. | Same as job title. |
| How to address them | Optional, plain text (not Markdown), at most 500, line breaks kept. What the human wants remembered about writing or speaking to them: "Vouvoiement. Madame Dubois in writing, Marie in person. Short messages, no attachments." | The contact page only, and `find(person=...)` for Claude Code, which drafts messages to them. |
| Level | All projects, one project, or one epic. Default: see "Levels". | Group headers in the picker and the contacts list; the contact page's properties. |
| Id | `C-n`, per installation, as every record (ADR 0005, 0006). | The meta line of the contact page; the agent. |
| Created, updated | Who and when (request 9), human or Claude Code. | The meta line: "C-3 · Added 21 Nov 2026 by Claude Code". |

Nothing else: no e-mail, phone, avatar, tags, birthday or notes beyond the address note. Acta tells, chases and contacts no one, so it needs no way to reach them; the address note is the one detail that changes how work is done (what Claude Code drafts). Each further field would be one more thing to keep up to date.

### Levels: where a contact lives and who sees it

The user's decision of 2026-11-22. A contact lives at exactly **one level**: all projects (global), a project, or an epic. It is **visible at its level and below**, as inherited folders are (components-folders.md 5): a topic in epic Terminal of project Acta sees the contacts of Epic Terminal, of Project Acta and of All projects; a note in Acta's inbox sees Acta's and All projects'; nothing sees a contact of another project or of a sibling epic.

- **References stay in scope.** A question, a note, a topic or an epic can only name a contact visible from it. The picker only offers those; the store refuses anything else (data.md 22).
- **Default level of a new contact**: the project of the record being edited (the picker), so a name typed while working on Acta stays with Acta; from the contacts page, the level of the group whose `plus` was used, else All projects. Moving up is one key and never refused, so the narrow default costs nothing.
- **Moving a contact** (menu "Move to...", `M` on a focused contact row or on the contact page, the palette "Move contact to..."): the palette turns into the list of levels, as "Move question to..." does: "All projects", then each project, each followed by its epics as "Acta / Terminal", the current level marked "Current" (`contact.level.current`) at the right in `type.small` `text.tertiary`. `Enter` moves; one `Mod+Z` step; no toast (the page shows the new level). 
  - **Down** (to a narrower level) is **refused** when the contact is named outside the new scope. The target stays listed, its row's right side reads "Used outside" (`contact.move.blocked`) in `type.small` `text.secondary`; `Enter` on it does not move, it shows under the list, in the palette's footer (`type.small` `text.secondary`, wraps, at most 4 lines), "Named outside Epic Terminal: T-51 Encrypted backups (Report to), Q-9 Who signs the export policy? (Asked). Change those first." (`contact.move.refused`, at most three places then "+4 more", `contact.move.more`). Refused, not asked: a dialog would have to choose for the human which references to clear, and clearing someone's Report to is not the side effect of a move.
  - **Up** is always allowed.
- **Moving a record moves no contact down, and lifts one when it must.** A topic moved to another epic or project, a question moved (`M`), a note filed into a topic of another project: when a contact the record names would no longer be visible from it, the contact is **lifted** to the narrowest level that sees both its old uses and the moved record (epic to its project; project to All projects), in the same `Mod+Z` step. Because it changes something the person did not touch, it says so in a toast with Undo: "Marie is now a contact of all projects, so Encrypted backups still sees them." (`contact.lifted`; the level as `contact.lifted.project` names the project). Lifting never refuses a move and never clears a reference.
- **Deleting a project or an epic** sends its contacts to the trash with it, in the same batch, and restores them with it. References to them can only come from inside that level, so nothing is left pointing at a contact in the trash by this path.

### Contact picker

The one control that chooses a person, in every place one is chosen: the question composer and editor (Asked), the Report to popover of topics and epics, quick capture and the inbox capture editor (Report to of a note), and the inbox's Report to filter's search. It replaces the person field. It is a **search input over contacts** with an inline way to add one; its value is always a contact or nothing, never text.

- **Input**: the text input of 5, 28 high, placeholder "Search contacts" (`contact.placeholder`). When a contact is chosen the input shows its name in `type.ui` `text.primary`; focusing the input selects that name, so typing replaces it. Max 80 characters typed (the name's limit; further typing is ignored).
- **List**: under the input, 4 below; inline (composer, quick capture) it is a floating layer (`bg.overlay`, 1px `border.default`, `radius.lg`, `shadow.overlay`, padding 4, the input's width, min 240); in a popover it is part of the popover. It opens when the input is focused and has text, or on `Down`; at most **8 contact rows**, grouped by level, **nearest level first** (for a topic: its epic, its project, All projects). Group headers: 24, padding 0 x 8, the level ("Epic Terminal", "Project Acta", "All projects") in `type.small` `text.tertiary`, not focusable; a group with no match is absent; with one group only, its header is absent too.
- **Contact row** (32, padding 0 x 8, `radius.sm`): the name in `type.ui` `text.primary`, matched characters weight 600; then, 8 after, the details "Head of legal · Acme" (job title, company, whichever exist, joined by " · ") in `type.small` `text.tertiary`, truncated first (the name truncates only when it alone is wider than the row). No count, no avatar. Two Maries read apart by their details or their group. Hover `fill.hover`; highlighted `fill.selected`, and on either the details turn `text.secondary` (5.24 / 6.79 on `fill.hover`, 4.83 / 6.42 on `fill.selected`, over `bg.overlay`, dark / light, 2.2; `text.tertiary` is not allowed on `fill.selected`).
- **"New contact" row**: last, after the groups, whenever the input has text, separated by 4 of space (no line): `plus` 14 in `text.secondary`, 8, "New contact "Mari"" (`contact.new_named`) in `type.ui` `text.primary`, the typed text as typed, then at the right "in Project Acta" (`contact.new_in`, the default level of "Levels") in `type.small` `text.tertiary` (`text.secondary` when hovered or highlighted). Always present with text, even when a contact of that exact name exists: there can be two Maries.
- **Ranking** (a pure function, next frame with 10,000 contacts): inside each group, contacts whose key has a word starting with the typed key, then those containing it; ties by open items in the active project, then most recently named. A contact whose key equals the typed key is first in its group. With an empty input (opened by `Down`): each group's contacts with open items in the active project, then the most recently named, 8 rows in all, no "New contact" row.
- **Keys**: typing filters; the **first row is highlighted** as soon as there is text (a match, else "New contact", the only row), so `Enter` takes what is highlighted; `Down` / `Up` move the highlight over contact rows and the "New contact" row, skipping headers; `Up` from the first row goes to the "New contact" row (one key to create when a prefix matched someone); `Enter` takes the highlighted row; `Tab` takes the highlighted row and moves to the next field (with no text, `Tab` just moves on); `Esc` closes the list and restores the previous value, a second `Esc` cancels the editor as before. `Enter` or `Delete`-then-`Enter` on an emptied input clears the value where clearing is allowed (Asked, Report to). The input never keeps free text: leaving it by pointer or `Shift+Tab` with unconfirmed text restores the previous value.
- **Creating inline** ("New contact" taken by `Enter`, `Tab` or a click): creates the contact with **the name only**, at the default level, author the human, and sets it in the field, as **one** `Mod+Z` step with the write that uses it (undo removes both). No toast and no form: details are added later on the contact page. In the composer and quick capture, the contact is created when the question or note is saved, not when the row is taken (a cancelled composer creates nothing); until then the field shows the name followed by "new" (`contact.new_tag`) in `type.small` `text.tertiary`.
- **Errors**: none in the picker; a name over 80 cannot be typed. A store refusal (contact out of scope after a concurrent move) restores the field and shows the 5 inline error under it: "Marie is no longer visible here. Choose again." (`contact.out_of_scope`).
- **Report to popover**: as before (components-questions.md 5), with this picker inside it, width 280 (was 248: the details need the room), and the footer's ghost "Clear" when a contact is set.
- **Inbox filter**: the dropdown "Report to: anyone" lists the contacts of the listed notes with their counts, unchanged; with more than 8, a search input at its top filters them (the picker's input and ranking, no "New contact" row: a filter does not create).

### Contacts page

The address book: every contact, to find, fix and tidy. A page with the fixed key `contacts` (restored after a restart like Trash; no content in the key).

- **Reached** from the pinned sidebar row "Contacts" (`users`, above Trash, layout.md 4.2), present **only once a contact exists** (nothing about people shows until something uses them); from the palette "Go to contacts" and "Go to contact..." (the palette lists contacts with their details and level, `Enter` opens the contact page); from any contact link.
- **Top**, in the content column: title "Contacts" (`type.title`); 8, the count line in `type.small` `text.tertiary` "14 contacts" (`contacts.count`); 24, the toolbar (28): a search input "Search contacts" (focused when the page opens, filters as typed over name, job title and company), then a segmented control "This project" / "All" (`contacts.filter.project`, `contacts.filter.all`; default This project: the contacts the active project sees, so the page agrees with the picker), then at the right a secondary "New contact" (`contacts.new`).
- **List**, grouped by level in tree order: "All projects", then for each project in view "Project Acta" and its epics "Epic Terminal"; group headers 28 as on the knowledge base page (the level a link to its page, except "All projects"; the count at the right; on hover a ghost `plus` "New contact here"). Inside a group, sorted by name (locale collation of 10.2), as an address book is.
- **Row** (32, padding 0 x 8, hangs 8 into the gutter as list rows do): name `type.ui` `text.primary`; 8, details "Head of legal · Acme" in `type.small` `text.tertiary`, truncated first; at the right, the open count across the contact's scope "3 open" (`contact.open_count`: open questions asked to them plus things not yet reported to them) in `type.small` `text.tertiary` `tnum`, absent at 0. Hover `fill.hover` (details and count stay `text.tertiary`: 4.97 / 5.09 over `bg.base`, 2.2); keyboard focus the ring. A contact created by Claude Code and not yet shown here carries the unread dot (6, `accent`) at the far right, as other records do (request 1).
- **Keys** on the list: `Up` / `Down`, `Enter` opens the contact page, `F2` renames in place (the name only), `M` moves (Levels), `Delete` deletes, `Shift+F10` the menu: Open, Rename `F2`, Move to... `M`, Merge into... , Delete `Delete`.
- **New contact** (the button, a group's `plus`, the palette): adds an editing row at the top of the target group with the name input focused, placeholder "Name" (`contact.name_placeholder`); `Enter` creates (name only) and opens nothing, focus stays on the new row; `Esc` cancels. Details are added on its page.
- **Empty**: with no contact at all (reachable from the palette only): "No contacts yet. A contact is added when you choose who a question is asked of or who to report to." (`contacts.empty`) in `type.reading` `text.secondary` and the "New contact" button. Search with no match: "No contact matches "{query}"." (`contacts.no_match`). "This project" with none while others exist: "No contacts in {project}." (`contacts.none_here`) with the ghost "Show all" (`contacts.filter.all`).

### Contact page

The answer to "who is Marie, how do I talk to them, and what is open with them?": the old person page with the contact's details on top. A page keyed by the contact id, so it is restored after a restart like any record.

- **Title**: the name (`type.title`, user text), edited in place like a record title (click, or `F2`); empty is refused with the 5 inline error "A contact needs a name." (`contact.name_required`) and the old name kept.
- **Properties block** (components-records.md 5; label column as there): rows **only when set**, as Due and Report to (clarity review 2026-11-22): "Job title" (`contact.prop.job_title`), "Company" (`contact.prop.company`), each a one-line text value edited in place (`Enter` saves, `Esc` cancels, emptied and saved removes the row); "Level" (`contact.prop.level`) always: "Project Acta" as a link to its page ("All projects" plain), its row menu offering "Move to..." `M`. Unset values are the properties block's add chips on hover or keyboard focus: "Job title", "Company", "How to address them" (`contact.add.*`). Then the meta line: "C-3 · Added 21 Nov 2026 by Claude Code" (`contact.meta`), with " · Updated 2 h ago" when it changed since.
- **How to address them** (section, only when set; added with its chip or the palette "Edit how to address them"): the 28 section header "How to address them" (`contact.section.address`), then the text in `type.reading` `text.primary`, line breaks kept, not clamped (500 characters at most). Click, `Enter` on the focused section or `E` edits it in the multi-line editor of 5 (placeholder "Formal or first name, tone, what to avoid", `contact.address_placeholder`; counter "432 / 500" over 400); `Mod+Enter` saves, `Esc` cancels; saving it empty removes the section. 
- **Possible duplicate** (notice of 5, `info` glyph, above the sections): only while another live contact **with the same key** is visible from this one or sees it (one is at a level above or below the other, or both global): "Another contact is also named Marie, in All projects." (`contact.duplicate`), ghost actions "Merge into it..." (opens Merge with that contact chosen) and "Keep both" ( records the pair as distinct, so the notice does not return for it; one `Mod+Z` step). Two Maries in two different projects never see each other and are never flagged.
- **What is open with them**, the person page's two sections, over the contact's whole scope (its level and below): "Questions asked" (`person.section.asked`) and "To report" (`person.section.report`), with the rows, order, keys (`Space` marks reported, `Enter` opens) and folded "N closed" / "N reported" rows of components-questions.md 5 (Person page, now this section). The scope line under the title becomes "2 questions to ask, 3 things to report" (`contact.scope`, the two `count.person_*` strings), with " in 2 projects" (`contact.scope_projects`) for a global contact used in more than one; rows then carry their project in the meta ("in Project Atlas", `question.in`). Nothing open: one line "Nothing open with Marie." (`person.empty`) in `type.reading` `text.secondary`.
- **Menu** (`ellipsis` at the right of the title on hover, `Shift+F10` on the title, the palette): Rename `F2`, Move to... `M`, Merge into..., Delete contact.

### Merging

For the duplicates inline creation and agents inevitably make ("Marie" and "Marie Dubois"). "Merge into..." (contact page menu, list menu, the duplicate notice, palette "Merge contact into...") turns the palette into the list of other contacts visible to or from this one, with details and level; `Enter` merges **this** contact into the chosen one:
- Every question, note, topic, epic and system event naming this contact names the chosen one; "reported" states are kept (the same person, so nothing was un-told).
- The chosen contact keeps its name and its non-empty fields; its empty fields take this one's; two address notes are joined, the chosen one's first, a blank line between (the 500 limit applies to typing, not to a merge, so nothing typed is lost).
- The result lives at the **wider** of the two levels, so every reference stays in scope.
- This contact goes to the trash, its row there saying "Merged into Marie Dubois" (`trash.merged_into`); the merge is **one** `Mod+Z` step, with the toast "Merged into Marie Dubois." and Undo (`contact.merged`), because references moved on pages the person is not looking at. Restoring it from the trash after the undo window is not an unmerge: it comes back with no references, and the toast says so ("Contact restored. Its uses stayed with Marie Dubois.", `contact.restored_merged`).

### Deleting

`Delete` on a row or the contact page, the menu, the palette "Delete contact": soft delete with the toast "Contact deleted." and Undo (`contact.deleted`), and the trash row (components-app.md 5: icon `user`, the name, its level as the place, "by Alexandre", days left).
- **References are kept.** Questions and records that name a deleted contact still show its name, in the same style but not as a link, with the tooltip "In the trash" (`contact.in_trash`); the picker and the agent never offer it; restoring it makes everything live again. Clearing a dozen Report to's as a side effect of a delete would be the irreversible surprise principle 7 forbids.
- **Expiry**: a trashed contact still named by a live record is not purged at 30 days; its trash row reads "Kept: still named in 4 places" (`trash.kept_named`) instead of the days left, and it expires 30 days after the last record naming it goes.

### Profile (Settings)

The first section of Settings (components-app.md 5, screens.md 8.30), "Profile" (`settings.profile`), two setting rows, under the heading one line in `type.small` `text.tertiary`: "Claude Code sees your name and job title." (`settings.profile.hint`):
- **Name** (`settings.profile.name`): a text input, 240 wide, at most 80; placeholder the name Acta uses today (the system user name, capitalised: "Alexandre"); description "Shown on what you write." (`settings.profile.name_hint`).
- **Job title** (`settings.profile.job_title`): a text input, 240 wide, at most 80, no placeholder, no description.
- Applies when the input loses focus or on `Enter` (`Esc` restores), with no Save button, as every setting. Emptying the name returns to the system user name. Palette: "Set your name" and "Set your job title" open Settings focused on the input.
- **Where it shows**: the human's name everywhere Acta names the author of a write (timeline headers, events "Alexandre set status to Active", "by Alexandre" in trash and history, the author mark's initial), resolved when drawn, so a change renames every past write: there is one human per installation. The job title shows nowhere in the interface; it exists for Claude Code.
- **Stored** in the encrypted database (request 23), never in `preferences.json` (4.7): a name is personal data. First run and the key-loss screen name no author, so nothing needs it before the database opens.

### What Claude Code sees

Everything below is stored text, so it is inside the data fence like titles (ADR 0005), and within the budgets that exist (data.md 22, 23):
- **Your profile**, once per session: the session-start briefing gains one line, only when a name is set in Settings (the system user name is a login, not a name, and is never sent): `User: Alexandre, Product designer` (job title omitted when empty), at most 170 characters, never dropped for space (memory titles go first). Not in `brief`: it does not change per record.
- **A contact's job title and company**, where the contact is named in a header: the topic and epic `brief` header `· report to Marie (Head of legal, Acme)`, the pair cut to 40 characters. Question lines and list lines keep the name only (context economy).
- **The address note**, only on request: in the header of `find(person=...)` and of `find(kind="contact")` with one result, cut to 300 characters at a word. The skill tells Claude Code to read it before drafting a message to someone, and to write in the user's name and job title.
- **Scope**: the agent sees and resolves only the contacts visible from the record it writes to (its focus by default), as the picker does.
- **What it may write**: a reference by `C-n` or by a name that resolves to exactly one contact in scope (an unknown name is refused with near matches, an ambiguous one with the ids; a name never creates a contact); a new contact through an explicit `create(kind="contact")`, **name and level only**, when the user named someone new (triage and wrap need it mid-conversation); a contact's level, under the same refusal as the UI. Renaming, details, merging, deleting and the profile are the human's. A contact Claude Code created carries the square author mark in its meta line ("Added by Claude Code") and the unread dot on the Contacts page until shown.

### Contrast

No new colour, token or pair. The pairs used: `text.primary` on `bg.base` / `bg.overlay` 12.55 / 10.30 dark, 12.85 / 13.40 light; `text.tertiary` on the same 5.78 / 4.74 and 5.67 / 5.92; `text.secondary` on `fill.hover` over `bg.overlay` 5.24 / 6.79 and on `fill.selected` over `bg.overlay` 4.83 / 6.42; `text.tertiary` on `fill.hover` over `bg.base` 4.97 / 5.09; `accent` links on `bg.base` 6.79 / 5.67. All pass AA (2.2).
