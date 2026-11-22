# Acta design: voice and languages

Part of the Acta design system. [DESIGN.md](../../DESIGN.md) is the index: direction, principles, the never-list and the review checklist. Section numbers are shared across all parts.

## 7. Voice

Plain, specific, short. Acta talks like a careful colleague's notes: it names the object, says what happened and what to do next, and stops.

- Sentence case. No exclamation marks, no "please", no "oops", "sorry", "successfully", "simply", no emoji.
- Verb plus object for actions ("New topic", "Delete entry"); the same word for the same thing everywhere (topic, epic, project, entry, note, inbox, current state, question, report to, contact, person, file; never "task", "ticket", "card", "item", "log", and never "assignee" or "stakeholder": a contact is only asked or told. "Contact" names the record and its list; "person" stays in sentences about someone ("Change person...")).
- The agent is always "Claude Code", never "AI", "the assistant" or "the bot". The human is their name ("Alexandre"), never "You" in attributions and "you" only in instructions.
- Errors say what failed, why if known, and the next step. Never blame, never a bare code.
- Numbers and times are exact and short: "3 open", "2 h ago", "12 Nov 2026". Counts above 99 show "99+".
- Empty states say what the place is for and offer the one action that fills it, when the human has one (an empty timeline has none: Claude Code fills it).
- One idea per line. A sentence that explains how something works, what it costs or where it is stored is read once: it goes in a tooltip or behind "Details", not on screen at rest (clarity review 2026-11-22). Labels never repeat on their own button.
- Every string exists in English and in French (10). The rules above hold in both; French adds its own typography and addresses the user as "vous". What the user and Claude Code write is shown as written, in whatever language they wrote it.

| Where | Before | After |
|---|---|---|
| Button | Submit | Save answer |
| Button | Create New Topic | New topic |
| Menu | Remove | Delete topic |
| Toast | Successfully deleted! | Topic deleted. Undo |
| Empty inbox | No notes yet! Start capturing your brilliant ideas. | Inbox is empty. Notes you capture land here until they become topics. |
| Empty timeline | Nothing to see here. | No entries yet. Claude Code logs progress here as it works. |
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
- Vocabulary, fixed like the English one: projet, epic (masculine, plural epics), sujet, entrée, note, boîte de réception, état actuel, journal (the timeline), connaissances, document, mémoire, dossier de travail, corbeille, instantané (snapshot), onglet, paramètres, chat (the chat panel), question (feminine; En suspens, Répondue, Abandonnée), « Posée à », « À rapporter à », contact (masculine), personne, « Comment s'adresser à ce contact », profil. A person is user text and in French always follows a colon (« Posée à : le service RH »), never a preposition, so no article contracts with it. Two words are kept apart on purpose: an entry has a **type** (Avancement, Décision, Discussion) and a knowledge entry has a **nature** (Document, Mémoire); "Discussion" is an entry type and never names the chat panel.
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
- **Whole days** exist only for the age of the last export ("12 days ago" / "il y a 12 jours", "today" / "aujourd'hui") and inside the Overview's sentences ("the oldest for 12 days" / "la plus ancienne depuis 12 jours"), counted in local calendar days as in 10.2.1.

| What | English | French |
|---|---|---|
| Weekdays, short | Mon Tue Wed Thu Fri Sat Sun | lun. mar. mer. jeu. ven. sam. dim. |
| Weekdays, in the French long form | | lundi mardi mercredi jeudi vendredi samedi dimanche |
| Months | Jan Feb Mar Apr May Jun Jul Aug Sep Oct Nov Dec | janv. févr. mars avr. mai juin juil. août sept. oct. nov. déc. |
| Thousands, decimals | 1,000 and 0.5 | 1 000 and 0,5 |
| Percent, sizes | 68%, 400 MB | 68 %, 400 Mo |
| Counts over 99 | 99+ | 99+ |

The first of the month is "1er" in French, in the long and the compact forms ("le 1er nov. 2026", "1er nov."); English has no such form. File names Acta proposes (`acta-2026-11-20.acta.json`) are not localised.

### 10.2.1 Due dates

Added 2026-11-21 with due dates (components-records.md 5, data.md 11 request 16). The date a question's answer is expected (components-questions.md 5) is the same kind of day and uses every form, threshold and parsing rule below; only its label differs ("Answer by"). A due date is a **calendar day**, not a moment: it has no time and no time zone, and reads as the same day on every machine. Everything relative ("today", "overdue") is computed against **the local date of the machine doing the reading**, the app or the `acta` process, at the moment of reading. The app re-evaluates every relative form at local midnight, on resume from sleep and when the system clock or time zone changes, without moving anything else (the rows keep their place; only text and the overdue marks change). "N days" is the difference in calendar days between the two local dates, never 24-hour periods, so a daylight-saving change never makes a date off by one.

**Forms.**

| Form | Used in | English | French |
|---|---|---|---|
| Absolute, `{date}` (10.2) | The Due property, tooltips, system events | 15 Dec 2026 | 15 déc. 2026 (the string supplies "le" or "au") |
| Full, `{date_full}` | The picker's preview line | Friday 27 November 2026 | Vendredi 27 novembre 2026 (capitalised: it stands alone) |
| Month header | The picker | November 2026 | Novembre 2026 |
| Relative phrase | The Due property, the picker's preview | Today · Tomorrow · In 3 days · Overdue by 3 days | Aujourd'hui · Demain · Dans 3 jours · En retard de 3 jours |
| Sentence, tooltips | Tree marks, table cells | Due today · Due tomorrow · Due 27 Nov 2026, in 6 days · Overdue by 3 days, due 18 Nov 2026 | Échéance aujourd'hui · Échéance demain · Échéance le 27 nov. 2026, dans 6 jours · En retard de 3 jours, échéance le 18 nov. 2026 |
| Compact, `{due_short}` | The table's Due column | see below | see below |

**Compact form** (the Due column, 72 of text): today "Today" / "Aujourd'hui"; tomorrow "Tomorrow" / "Demain"; 2 to 6 days ahead, the short weekday, capitalised ("Fri" / "Ven."); later this year "15 Dec" / "15 déc."; another year "Feb 2027" / "Févr. 2027". **A past date is always written as a date** ("18 Nov", "Feb 2025"), never as a weekday or "Yesterday", so a past date can never be read as an upcoming one. Measured at 12px with fontTools from the font files (advance widths, before `tnum`): the widest is "Aujourd'hui", 64.7 in Inter and 61.7 in Segoe UI; then "Sept. 2027" 61.8 / 55.5 and "Tomorrow" 58.7 / 54.7. All fit 72; with the 12 icon and its 4 the column is 88.

Full month names: January ... December / janvier février mars avril mai juin juillet août septembre octobre novembre décembre. Full weekday names: Monday ... Sunday / lundi ... dimanche (10.2). Weeks start on Monday in both languages, in the picker and in "next week".

**Typed dates** (the picker's input; the only place a person types a date). The parser accepts English and French whatever the interface language, because the words do not collide: a weekday stands alone, a month needs a day next to it ("mar" alone is mardi, "15 mar" is the 15th of March). It ignores case, accents, a trailing period and the space between a number and its unit; the preview line shows the exact day before anything is set, which is how any remaining doubt is settled.

| Typed | English | French | Result |
|---|---|---|---|
| Today, tomorrow, the day after | `today`, `tod`, `tomorrow`, `tom`, `tmr` | `aujourd'hui`, `auj`, `demain`, `après-demain` | That day |
| Yesterday | `yesterday` | `hier` | Yesterday (immediately overdue; allowed) |
| A weekday | `fri`, `friday`, `next fri` | `ven`, `vendredi`, `vendredi prochain` | The next such day after today; today is never matched (it has its own word), so `fri` on a Friday is in 7 days. "next" / "prochain" changes nothing |
| An offset | `+3`, `3d`, `in 3d`, `in 3 days`, `in 2w`, `in 2 weeks`, `in 1m`, `in 1 month`, `-2` | `+3`, `3j`, `dans 3 j`, `dans 3 jours`, `dans 2 sem`, `dans 2 semaines`, `dans 1 mois`, `-2` | Today plus or minus days or weeks; months keep the day number, clamped to the month's last day |
| Next week, next month, end of month | `next week`, `next month`, `end of month`, `eom` | `semaine prochaine`, `mois prochain`, `fin du mois` | Monday of next week; the 1st of next month; the last day of this month |
| Day and month, no year | `15/12`, `15.12`, `15 dec`, `dec 15`, `15 december` | `15/12`, `15.12`, `15 déc`, `15 décembre`, `1er mai` | The next such day on or after today |
| Day only | `15` | `15` | The next 15th on or after today |
| With a year | `15/12/2026`, `15/12/26`, `15 dec 2026` | `15/12/2026`, `15/12/26`, `15 déc 2026` | That day (two-digit years are 20xx) |
| ISO | `2026-12-15` | `2026-12-15` | That day |

Day and month are always in that order, in both languages (10.2): `03/04` is the 3rd of April. A day that does not exist (`31/02`) or a year outside 2000 to 2099 is not a date. Anything else is not a date; nothing is guessed.

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
