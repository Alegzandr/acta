# Acta design: voice and languages

Part of the Acta design system. [DESIGN.md](../../DESIGN.md) is the index: direction, principles, the never-list and the review checklist. Section numbers are shared across all parts.

## 7. Voice

Plain, specific, short. Acta talks like a careful colleague's notes: it names the object, says what happened and what to do next, and stops.

- Sentence case. No exclamation marks, no "please", no "oops", "sorry", "successfully", "simply", no emoji.
- Verb plus object for actions ("New topic", "Delete entry"); the same word for the same thing everywhere (topic, epic, project, entry, note, inbox, current state; never "task", "ticket", "card", "item", "log").
- The agent is always "Claude Code", never "AI", "the assistant" or "the bot". The human is their name ("Alexandre"), never "You" in attributions and "you" only in instructions.
- Errors say what failed, why if known, and the next step. Never blame, never a bare code.
- Numbers and times are exact and short: "3 open", "2 h ago", "12 Nov 2026". Counts above 99 show "99+".
- Empty states say what the place is for and offer the one action that fills it.
- Every string exists in English and in French (10). The rules above hold in both; French adds its own typography and addresses the user as "vous". What the user and Claude Code write is shown as written, in whatever language they wrote it.

| Where | Before | After |
|---|---|---|
| Button | Submit | Add entry |
| Button | Create New Topic | New topic |
| Menu | Remove | Delete topic |
| Toast | Successfully deleted! | Topic deleted. Undo |
| Empty inbox | No notes yet! Start capturing your brilliant ideas. | Inbox is empty. Notes you capture land here until they become topics. |
| Empty timeline | Nothing to see here. | No entries yet. Add one, or let Claude Code log progress as it works. |
| Error | Error: write failed (code 5) | Could not save the entry. The database is locked by another process. Retry |
| Error | Invalid input | A topic needs a title. |
| Conflict | Conflict detected! | Claude Code changed this while you were editing. |
| Dialog title | Are you sure? | Restore the snapshot from 18 Nov 2026? |
| Dialog body | This action cannot be undone. | Acta will restart with its data as of 18 Nov 2026, 09:12. Everything as it is now is saved as a snapshot first. |
| Export warning | WARNING: unencrypted export! | The file is not encrypted. Anyone who can open it can read these projects. |
| Key loss | Fatal error: decryption failed | Acta cannot open its database. The file is intact, but its key is missing from the system keychain. |
| Import result | Import completed with 9 conflicts | Imported: 12 created, 3 updated. 9 skipped because Acta already has the same or a newer version. |
| Missing folder | Error: invalid path | Not found on this computer. Locate... |
| No folder | No directory configured! | No working folder. The terminal will ask where to start. |
| Attribution | Updated by AI 2 hours ago | Updated 2 h ago by Claude Code |
| Placeholder | Enter description here... | What is this topic about, and what does done look like? |

## 10. Languages

The interface exists in English (the default on a new install) and in French. The language is chosen in Settings or with the palette ("Language: English", "Language: Français"), applies at once without a restart, and is stored in the preferences file (4.7). Acta does not follow the OS language on its own: the default is English until the user chooses.

**What is translated and what never is.** Translated: every string Acta itself puts on screen, the date, time and number formats, and the key names in shortcut hints. Never translated, reformatted or re-punctuated: anything the user or Claude Code wrote (titles, states, entries, notes, documents, memories, summaries), project, epic and topic names inside Acta's sentences, paths, ids, code, terminal content, the claude.ai page, and the proper names "Acta", "Claude Code", "claude.ai", "WebView2", "Markdown". Agent-facing output (MCP, CLI, hooks; ADR 0005) stays in English whatever the interface language: it is a protocol, not an interface.

### 10.1 French conventions

- Sentence case as in English: a capital on the first word and on proper names only ("Nouveau sujet", never "Nouveau Sujet").
- The user is addressed as "vous". Commands and buttons are infinitives ("Supprimer le sujet", "Choisir un dossier..."); states are past participles ("Sujet supprimé"). The one deliberate exception is navigation, "Précédent" and "Suivant" for Back and Forward, as every French browser has it. A key is never written into a sentence: strings take `{keys}` and the hint is rendered with the platform's and the language's key names.
- A narrow no-break space (U+202F) before `;` `:` `!` `?` and `»` and after `«`; in the catalogue (10.4) an ordinary space is typed there and stands for U+202F. Quotation marks are « ». The apostrophe is the typographic one (’) in both languages; the table types `'` for legibility.
- Numbers: U+202F as thousands separator ("1 000"), a comma for decimals, U+202F before `%` and before units ("68 %", "400 Mo").
- A trailing ellipsis is the single character U+2026 in both languages, with no space before it; the table types three dots.
- Vocabulary, fixed like the English one: projet, epic (masculine, plural epics), sujet, entrée, note, boîte de réception, état actuel, journal (the timeline), connaissances, document, mémoire, dossier de travail, corbeille, instantané (snapshot), onglet, paramètres, chat (the chat panel). Two words are kept apart on purpose: an entry has a **type** (Avancement, Décision, Discussion) and a knowledge entry has a **nature** (Document, Mémoire); "Discussion" is an entry type and never names the chat panel.
- Deliberate choices, not oversights: "e-mail" (the form French software uses, rather than "courriel"), "plugin" (Claude Code's own word for what setup installs), "epic" and "chat" kept as loanwords. Statuses: Brouillon, Prévu, En cours, Bloqué, Terminé, Abandonné.
- Key names on Windows and Linux: `Ctrl`, `Maj`, `Alt`, `Entrée`, `Échap`, `Suppr`, `Tab`. macOS glyphs do not change.

### 10.2 Dates, times and numbers

Both languages use the 24-hour clock and day-month-year order; neither follows the OS region, so a screenshot reads the same on every machine. A moment in time is shown in one of four forms, and every catalogue string says which one its placeholder takes.

| Age of the moment | Long, English | Long, French | Compact, English | Compact, French |
|---|---|---|---|---|
| Under a minute | just now | à l'instant | Just now | À l'instant |
| Under an hour | 12 min ago | il y a 12 min | 12 min ago | Il y a 12 min |
| Earlier today | 2 h ago | il y a 2 h | 2 h ago | Il y a 2 h |
| Yesterday | yesterday 18:42 | hier à 18:42 | Yesterday | Hier |
| 2 to 6 days ago | Tue 14:05 | mardi à 14:05 | Tue | Mar. |
| Older, this year | 12 Nov 2026 | le 12 nov. 2026 | 12 Nov | 12 nov. |
| Older, another year | 3 Feb 2025 | le 3 févr. 2025 | Feb 2025 | Févr. 2025 |

- **Long form, `{when}`**: used inside sentences and meta lines ("Updated 2 h ago by Claude Code", "Modifié mardi à 14:05 par Claude Code"). The French forms carry their own preposition or article, so a string never adds one. Lower case inside a sentence, capitalised when it starts a string or stands alone ("Yesterday 18:42", "Hier à 18:42", "Mardi à 14:05", "Le 12 nov. 2026").
- **Compact form, `{when_short}`**: used alone in the fixed time columns (knowledge rows, topic table, history). Measured in Inter at 12px with tabular figures, the widest is "Il y a 59 min" at 68.3px, then "12 min ago" at 63.8 and "Yesterday" at 57.7; all fit the 72 column, which is why the columns are 72 and why the long forms ("Yesterday 18:42" is 95.3, "12 nov. 2026" 77.4) are not used there. The tooltip gives the full form.
- **Absolute date, `{date}`**: "18 Nov 2026" / "18 nov. 2026", with no article; the French string supplies "le" or "du" itself. **Absolute date and time, `{datetime}`**: "18 Nov 2026, 09:12" / "18 nov. 2026 à 09:12". Used where exactness matters more than recency: versions, snapshots, export files.
- **Full form** (tooltips only): "Tuesday 17 November 2026, 14:05" / "mardi 17 novembre 2026 à 14:05".
- **Today with a time** exists only in the snapshot line of Settings and has its own strings: "today, 09:12" / "aujourd'hui à 09:12".
- **Whole days** exist only for the age of the last export: "12 days ago" / "il y a 12 jours", "today" / "aujourd'hui".

| What | English | French |
|---|---|---|
| Weekdays, short | Mon Tue Wed Thu Fri Sat Sun | lun. mar. mer. jeu. ven. sam. dim. |
| Weekdays, in the French long form | | lundi mardi mercredi jeudi vendredi samedi dimanche |
| Months | Jan Feb Mar Apr May Jun Jul Aug Sep Oct Nov Dec | janv. févr. mars avr. mai juin juil. août sept. oct. nov. déc. |
| Thousands, decimals | 1,000 and 0.5 | 1 000 and 0,5 |
| Percent, sizes | 68%, 400 MB | 68 %, 400 Mo |
| Counts over 99 | 99+ | 99+ |

The first of the month is "1er" in French, in the long and the compact forms ("le 1er nov. 2026", "1er nov."); English has no such form. File names Acta proposes (`acta-2026-11-20.acta.json`) are not localised.

### 10.3 Layout under longer text

French runs 15 to 25% longer than English on sentences and up to twice as long on one-word labels. The layout absorbs that by rule, not by case:

- **Nothing has a width fitted to its English text.** Buttons, segments, menu items, tabs, chips and property labels size to their content. The fixed columns (72 for a compact time, 56 for a count) hold only the compact forms of 10.2 and numbers; the widest compact time in either language is 68.3px.
- **What may truncate, with an ellipsis and a tooltip giving the whole text**: tree rows, tab titles, table titles, the breadcrumb (4.3), a tool window header's context text, a property label beyond 40% of the column, a setting's label beyond half the row. **What may never truncate**: buttons, menu items, dialog titles and bodies, notices, toasts, empty states, error messages, status words. They wrap.
- **Segmented controls** size each segment to its label. If the whole control would exceed the width it has, it becomes a dropdown button showing the current value; this is the timeline filter's behaviour in a 356 column in French.
- **Dialogs**: buttons stay on one row while they fit the dialog's content width; otherwise they stack, full width, the confirming action on top. Dialog widths (440, 520) do not change with language.
- **Toasts** wrap to at most three lines inside their 360; the action stays on the last line.
- **The status bar and the palette footer** drop items from the least important end when they do not fit: the status bar truncates the project name to 24 characters, then hides the version label's "Acta" prefix; the palette footer drops its hints from the right.
- **Setting rows** let the description wrap under the label; the control never moves to another line.
- **Sentences are whole catalogue entries with named placeholders.** No sentence is built by joining fragments, because word order differs ("{time} par {author}"). Anything with a count has plural forms: English has one and other; French uses one for 0 and 1. Where a French participle or adjective agrees with a document (masculine) or a memory (feminine), the string has one variant per kind. The one licensed join is a list of counts ("12 documents, 43 memories"): each count is its own pluralised string and the list is joined with ", ".
- **Review aid**: a pseudo-language that lengthens every string by 40% and wraps it in brackets must leave every screen usable at 880 x 560. It is a build switch, not a setting.
- Alphabetical order, where Acta sorts by name, follows the interface language's collation; the order the user arranged by hand is never touched.
