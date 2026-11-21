# Acta design: attachments

Part of the Acta design system. [DESIGN.md](../../DESIGN.md) is the index: direction, principles, the never-list and the review checklist. Section numbers are shared across all parts.

## 5. Components (continued): attachments

Added 2026-11-21 at the user's request: files can be pasted into notes (clipboard, drag and drop, a button), and go with the note when it is filed. Storage, limits, export and agent access are decided in [ADR 0012](../decisions/0012-attachments.md); model in data.md 11, request 20; copy in catalogue 10.4.7. In the interface the word is **file** / **fichier** ("Attach files", "3 files"); "attachment" is the docs' and the code's word.

**Where files can be.** A note; a timeline entry; the description of a topic or an epic (where a note's text and files land when it becomes one, ADR 0012, 5). Files are added wherever such text is written: quick capture, the inbox's capture editor, a note being edited, the timeline composer and an entry being edited, a description being edited. Knowledge documents and memories take no files (ADR 0007).

### Attachment strip

Under the text it belongs to (a note card's body, an entry's body, a description), 8 below it; in an editor, between the text and the editor's footer. Absent when there is no file.
- **Images first**, as thumbnails: tiles 64 x 64, `radius.sm`, 1px `border.subtle`, the stored 256 px thumbnail scaled to cover the tile and centred (crop, never stretch), on `bg.sunken` while it decodes; 8 apart, wrapping. GIFs show their first frame (6.3: nothing animates by itself). Tooltip: the name and the size.
- **Other files** as chips, 8 below the tiles (or at the top when there is no image), wrapping, 8 apart: height 28, padding 0 x 8, `radius.sm`, 1px `border.default`, no fill; a kind icon 16 in `text.secondary` (`file-text` for text-like files and PDFs, `file` for anything else; new icon), 8, the name in `type.ui` `text.primary`, truncated in the middle so the extension stays ("Quarterly-rev…ew-final.pdf"), at most 200 wide, the whole name in the tooltip; 8, the size in `type.small` `text.tertiary` with `tnum`.
- **Sizes**: bytes under 1 KB ("812 B" / "812 o"), then KB and MB with one decimal under 10 and none above ("340 KB", "2.1 MB", "12 MB"; French "340 Ko", "2,1 Mo", "12 Mo"), 1 KB = 1,024 bytes, as the system file managers on Windows show them.
- **States**: chip hover `fill.hover`, its size then in `text.secondary` (`text.tertiary` on `fill.hover` over a card's `bg.raised` is 4.46 in dark, under 4.5; 2.2); tile hover a 1px `border.strong` outline instead of its `border.subtle`, since a fill cannot show over an image; keyboard focus ring; pressed as buttons. A focused card or entry reveals no extra controls for files: their actions are on the files themselves.
- **Keys.** The strip is one stop after the text of its card, entry or description (`Tab` from the focused card, or `Down` from the text in an editor); inside it `Left` / `Right` move from file to file, `Home` / `End`, `Esc` back to the card. On a file: `Enter` or `Space` views it (images and text-like files) or opens its menu (others); `Mod+S` Save as...; `Delete` (or `Cmd+Backspace`) removes it with the toast "File removed." and Undo; `Shift+F10` or the menu key opens the menu: View, Save as..., Remove.
- **Pointer**: a click views or opens the menu as `Enter` does; right click opens the menu. Files cannot be reordered in v1 (the order is the order they were added), and cannot be dragged out of Acta (a drag out would write the plaintext somewhere Acta does not choose).

### Adding files

- **Paste** (`Mod+V`) in any of the editors above: if the clipboard holds files (copied in Explorer, Finder or a Linux file manager), they are attached; if it holds an image and no text (a screenshot), the image is attached as a PNG named "Pasted image 2026-11-21 14.05.png" (`attach.pasted_name`, digits never localised, dots because `:` is not allowed in Windows file names); if it holds text, the text is pasted, whatever else it holds, because a person pasting from a web page or a document means the words. The caret does not move when files are attached.
- **Drop**: files dragged over one of those editors, or over a note card at rest, show the drop target: the editor or card gets `accent.subtle` behind it and the line "Drop to attach" (`attach.drop`, `type.ui` `text.primary`, 2.2) centred in it (8.46 / 10.69 over `bg.raised`, 7.73 / 10.69 over `bg.overlay`, dark / light), replacing nothing (it is drawn over the content, which stays in place). Dropping attaches; `Esc` or leaving the target cancels. Elsewhere in the window a file drop does nothing.
- **Button**: in each editor's footer, left of the save hint, a ghost compact icon button 24, `paperclip` (new icon), tooltip "Attach files..." with `Mod+Shift+A` (`attach.button`), opening the system's open dialog with several files allowed. `Mod+Shift+A` works only while one of these editors has focus; it is not a global key.
- **While adding.** Each file appears at once as its tile or chip, with the name; reading, hashing and storing run off the UI thread. If a file is still being stored after 400 ms, its chip shows "Adding… 40%" (`attach.adding`, `type.small` `text.tertiary`, `tnum`) in place of the size. Saving the note or entry is not blocked: it waits for the files in the background and the record shows them as they finish.
- **Refusals**, under the strip, one line each, `type.small` with `triangle-alert` 14 in `danger`, the file not added: over 25 MB "{name} is 48 MB; the limit is 25 MB per file." (`attach.too_large`); over 20 files "20 files is the most a note, an entry or a description can hold." (`attach.too_many`); unreadable "Could not read {name}. It may have moved, or another app may be holding it." (`attach.read_failed`). They stay until the editor closes or the next add.
- **Large files**: when a file over 10 MB is added, one line in `type.small` `text.tertiary` with `info` 14, "Files over 10 MB are kept in the database and in each of its snapshots." (`attach.large_notice`), once per editor session.
- **Undo**: adding (all files of one paste, drop or dialog together) and removing are one `Mod+Z` step each; filing a note moves its files with it in the same step.

### Viewer

An overlay for images and text-like files, decrypted in memory only (ADR 0012, 6). Opened by `Enter`, `Space` or a click on a tile or a text-like chip.
- A floating layer inset 32 from the window's edges (24 under 880 x 560), `bg.overlay`, 1px `border.default`, `radius.xl`, `shadow.overlay`, over the `scrim`; the chat and the agent browser web views are hidden as under any overlay (components-app.md 5).
- **Header** (44, padding 0 x 16, 1px `border.default` under it): the name in `type.ui-strong` `text.primary`, middle-truncated; 8, the size and, for images, the pixel size ("3.4 MB · 3024 x 4032") in `type.small` `text.tertiary` `tnum`; at the right, when the owner has more than one viewable file, "2 of 3" (`attach.position`) and the compact ghost icon buttons `chevron-left` / `chevron-right` ("Previous file" / "Next file", `Left` / `Right`); then a ghost button "Save as..." (`Mod+S`) and the ghost icon button `x` (`Esc`).
- **Image**: centred on `bg.sunken`, fitted inside the body, never scaled above 100%. `0` or a double click toggles fit and 100% ("Fit" / "Actual size", the button's tooltip); at 100%, drag or arrow keys with `Shift` pan, the wheel scrolls. No zoom levels beyond those two, no editing.
- **Text**: `type.code` `text.primary` on `bg.overlay`, padding 16, no wrapping, both scrollbars, the first 1 MB; beyond it a notice line at the bottom, "Showing the first 1 MB. Save the file to see all of it." (`attach.truncated`). `Mod+A` / `Mod+C` select and copy.
- Focus enters the viewer and returns to the file when it closes (6.2). `Esc` closes; nothing in the viewer changes data.

### Save as and Open in another app

- **Save as...** opens the system's save dialog with the file's name, in the last folder used for Save as in this session (the system default the first time). On success a toast "Saved to {folder}." (`attach.saved`) offers the action "Show in folder" (`attach.show_in_folder`; fr « Afficher dans le dossier »), which opens the system file manager on the parent folder with the file selected (Explorer `/select,`, Finder reveal, the file manager on Linux), never the file itself. A failure shows the error toast "Could not save {name}. {reason}" (`attach.save_failed`) with Retry.
- **Open in another app** (once ADR 0012, 8, is accepted): the first use per installation shows a dialog of 440, "Open in another app?" (`attach.open_title`), body "Acta writes a decrypted copy for the other app and deletes it when Acta quits." (`attach.open_notice`), Cancel and Open; after that it acts at once. The system's default app opens the copy. It is not offered while ADR 0012, 8, is proposed; PDFs and other files then offer Save as only, and their chip's `Enter` opens the menu with Save as first.

### Where else files show

- **Note card** (components-records.md 5): the strip under the body, inside the card's padding; the meta row unchanged. A card clamped to 6 lines keeps its strip visible below the clamp.
- **Timeline entry**: the strip under the body, 8 below it, inside the decision box for a Decision.
- **Description** of a topic or an epic: the strip under the rendered description.
- **Inbox, Filed**: filed notes keep showing their files' names and sizes as chips in `text.secondary`, without thumbnails, with the line "Moved to T-51" (`attach.moved_to`, the id a link) in place of the actions: the files now live with the topic or entry.
- **Trash** (components-app.md 5): a removed file is a row with `paperclip`, its name and its owner as the original place; restoring it puts it back at the end of its owner's strip.
- **Settings, Data**: "Attached files: 312 MB in 48 files" (`settings.attachments`) under the database size.
- **Export** (components-app.md 5): the checkbox "Include attached files (32 MB)" (`export.include_files`), on by default, hidden when there is none.
- **Tree, tables, tabs, palette**: nothing. A file is part of its note or entry, not a record to navigate to.
