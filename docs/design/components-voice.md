# Acta design: dictation

Part of the Acta design system. [DESIGN.md](../../DESIGN.md) is the index: direction, principles and the never-list; the review checklist is [checklist.md](checklist.md). Section numbers are shared across all parts.

## 5. Components (continued): dictation

Added 2026-11-22 at the user's request: text fields where a person may write a lot offer writing by voice, with a button. Engines, models, privacy and the build are decided in [ADR 0014](../decisions/0014-voice-dictation.md); copy in catalogue 10.4.9. States, the focus ring and the controls used here (buttons, dropdown, checkbox, dialog, notice) are in [components.md](components.md). In the interface the word is **dictate** / **dicter** ("Dictate", "Dictation"); "voice" appears only as the Settings section's name, and "speech recognition" / "transcription" never.

Dictation writes text into the editor, as typing would. Nothing records that a text was dictated: Claude Code, the timeline, the history and the export see ordinary text (ADR 0014, 9).

### Where the microphone is

Every multi-line editor (5) and nothing else: the places where a person writes paragraphs.

| Editor | Where the button sits |
|---|---|
| Description (project, epic, topic), current state, a timeline entry being edited | the editor's footer, left of the save hint, after the `paperclip` where there is one |
| Quick capture, the inbox's capture editor, a note being edited | the same footer |
| Memory body (edited in place), document body (document editor) | memory: its editor's footer; document: the 44 bottom bar, after "Editing" and its draft state |
| Question composer, answer editor | composer: the trailing slot of the question text field (compact, inside its right padding); answer editor: its button row, left of "Cancel" |

Not on: titles, summaries, the contact picker, the palette and every search or filter input, the due date picker, the terminal (Claude Code reads the keyboard there) and the chat's web view (a page Acta does not touch). These are short or are someone else's input.

### Microphone button

A ghost compact icon button 24 (5, Button). Its states, in order of a take:

| State | Button | Next to it, in the footer (`type.small`) | Tooltip |
|---|---|---|---|
| **Idle** | `mic` in `text.secondary`; hover `fill.hover`, icon `text.primary` | nothing | "Dictate" with `Mod+Shift+Space` (`voice.button`) |
| **Not set up** (no engine chosen yet, or the chosen model is not on disk) | as idle | nothing | "Dictate..." (`voice.button_setup`); the trailing ellipsis because it asks first |
| **Downloading** (the model is coming) | as idle | nothing | "Dictation is downloading its model: {percent}" (`voice.button_downloading`); a click opens Settings > Voice |
| **Starting** (microphone opening, model loading) | `square` in `text.primary` on `fill.selected` | after 400 ms only: "Starting..." in `text.tertiary` (`voice.starting`) | "Stop dictating" with `Mod+Shift+Space` (`voice.stop`) |
| **Listening** | `square` in `text.primary` on `fill.selected` | 8, the level bar, 8, "Listening" in `text.secondary` (`voice.listening`), 8, elapsed `0:12` in `text.tertiary` `tnum` | "Stop dictating" |
| **Writing** (the last words are being turned into text after the stop) | `mic` in `text.disabled`, not pressable | after 400 ms only: "Writing text..." in `text.tertiary` (`voice.writing`) | none |
| **Error** | as idle | the error line under the footer (below) | as idle |
| **Off** (Settings > Voice > Dictation: Off) | absent; the footer closes up | | |

- **Level bar**: 32 x 4, `radius.xs`, track `fill.selected`, fill `text.secondary` from the left, its length the input level (RMS of the last 50 ms, on a decibel scale from -60 to 0 dBFS). It follows the sound as data arrives, as the updater's bar follows bytes: never eased, never animated by itself, and it does not count as motion (6.3). It is how a person sees that the right microphone is live; silence leaves it at the left.
- **No red, no accent.** A recording state is said by the shape (`square`, the stop icon of media players), the pressed fill and the word "Listening", so it reads in greyscale. The system adds its own sign of a live microphone (the taskbar's microphone icon on Windows, the orange dot on macOS); Acta does not imitate it.
- While listening, the save hint (`Mod+Enter to save`) yields its place when the footer is too narrow; the primary button never moves.

### Taking dictation

- **Start**: a click on the button, or `Mod+Shift+Space` while one of the editors above has focus (6.1; not a global key, like `Mod+Shift+A`). Focus stays in the editor: the caret keeps blinking and the person can type, select and scroll while speaking.
- **Two gestures, one key, no setting.** A short press (under 300 ms) toggles: press to start, press again to stop. Holding the key (or the mouse button on the microphone) for 300 ms or more is push-to-talk: releasing it stops. The tooltip and Settings say both.
- **Where the text goes.** At the caret where dictation started: that point is an anchor that moves with the text around it, so typing elsewhere in the editor meanwhile never splits a sentence. Each phrase is written when a pause ends it (ADR 0014, 6: about 0.6 s of silence), so a long dictation fills in as it goes; there are no words that appear and then change. If the caret was at the anchor, it follows the inserted text. A space is put before a phrase that follows a character that is not a space or a line start; nothing else is added or changed. Whisper writes its own punctuation and capitals; nothing is said aloud to get a full stop or a new line in v1.
- **Stop**: the button, the key again (or its release), or `Mod+Enter` (stops, waits for the last phrase, then saves: the footer shows "Writing text..." meanwhile). Focus leaving the editor, or the editor closing, stops too, and the last phrase is still written into that editor's draft (kept as every draft is, request 15). A take stops by itself after 60 s without speech (`voice.stopped_silence`, one line in `text.tertiary` that stays until the next take) and after 10 minutes in all (`voice.stopped_limit`).
- **`Esc`** while listening or writing stops and throws away what has not been written yet; text already written stays (undo removes it). The editor is not cancelled: the next `Esc` does that, as always.
- **Undo**: one take is one `Mod+Z` step of the editor, whatever number of phrases it wrote.
- **Nothing heard**: a take that ends with no speech writes nothing and shows "Nothing was heard. Check the microphone in Settings > Voice." in `text.tertiary` with `info` 14 (`voice.nothing_heard`), with the link "Voice settings".
- **One take at a time** in the whole app. Starting dictation in a second editor stops the first take (its last phrase goes to the first editor).
- The window losing focus does not stop a take (a person may read from another window while speaking); the 60 s and 10 minute limits still apply.

### Errors

One line under the editor's footer, 4 below, `type.small` `text.primary` with `triangle-alert` 14 in `danger`, and at most one ghost action. It stays until the next take or until the editor closes. Never a toast, never a dialog.

| Case | Line | Action |
|---|---|---|
| The system blocks the microphone (Windows privacy setting, macOS permission refused) | "Acta is not allowed to use the microphone." (`voice.error.blocked`) | Windows: "Open privacy settings" (`ms-settings:privacy-microphone`); macOS: "Open System Settings" (Privacy & Security > Microphone) |
| No microphone | "No microphone was found." (`voice.error.no_device`) | "Voice settings" |
| The chosen microphone is gone (unplugged mid-take) | "The microphone stopped. What was heard is kept." (`voice.error.device_lost`); what was captured is still written | "Voice settings" |
| The model file is missing or fails its check | "The dictation model is missing or damaged." (`voice.error.model`) | "Download again" |
| The engine stopped (the worker process exited, ADR 0014, 5) | "Dictation stopped unexpectedly. Your text is safe." (`voice.error.engine`) | "Try again" |
| Too slow (CPU model taking over 3 times the audio length) | not an error: one line in `text.tertiary`, "Dictation is slow on this computer. A smaller model is faster." (`voice.slow`), once per session | "Voice settings" |

### First use

The first time the microphone is pressed (no engine chosen yet), a dialog (5, Dialog, 440) opens instead of listening. Focus starts on Cancel.

- Title "Dictate with your voice" (`voice.setup.title`).
- Body (`voice.setup.body`): "Acta turns what you say into text on this computer. Your voice never leaves it and is never saved."
- The recommendation, one block (ADR 0014, 4): `type.ui-strong` "Whisper Small" with `type.small` `text.secondary` "190 MB download · works on this computer's processor" (or "· uses this computer's graphics" for the GPU tier); under it `type.small` `text.tertiary`: "Free on this disk: 112 GB." (`voice.setup.free`). When the free space is under the model's size plus 1 GB the line is `warning` text with `triangle-alert` 14 and Download is disabled: "Not enough free space: 574 MB needed, 310 MB free." (`voice.setup.no_space`).
- On a Mac whose recogniser works on the device for the chosen language (ADR 0014, 3), a ghost button "Use the macOS recogniser instead: nothing to download" (`voice.setup.system`). It becomes the recommendation, with "Use the macOS recogniser" as the primary button, when the free space cannot hold the smallest model plus 1 GB or the computer is under the hardware floor; on Windows and Linux under the floor the dialog says "Dictation needs at least 2 GB of memory; this computer has {memory}." (`voice.setup.unavailable`) with Close only.
- "Other models are in Settings > Voice." in `type.small` `text.tertiary`, "Settings > Voice" a link.
- Buttons: Cancel, "Download 190 MB" (`voice.setup.download`, primary; the size is the model's).

After Download the same dialog shows the progress: the status line "Downloading Whisper Small..." (`voice.download.progress_title`), the bar of the updater window (5: full width, 4 high, never eased) and "120 of 190 MB" (`voice.download.bytes`, `tnum`) under it; then "Checking..." (`voice.download.checking`) while the hash is verified; then "Ready. Press Mod+Shift+Space in a text, or the microphone, to dictate." (`voice.download.ready`). Buttons during the download: "Hide" (the download continues; progress is in Settings > Voice and in the microphone's tooltip) and "Cancel download" (deletes the partial file). When it is ready: "Close", focused. Listening never starts by itself at the end of a download. A failed download shows its line (below) with "Retry".

### Settings > Voice

A section of the Settings page (components-app.md 5) after Claude Code and before Data; rows as every setting row (44, description under the label, control at the right). Everything applies at once.

| Row | Control | Description |
|---|---|---|
| Dictation | dropdown: "Whisper, on this computer" / "macOS recogniser, on this computer" (macOS only, when it works on the device) / "Off" | "Dictation in notes, descriptions and documents. Your voice never leaves this computer." (`voice.settings.engine_hint`) |
| Model (Whisper only) | the model list (below) | "Recommended for this computer: Small. Larger models are more accurate and slower." (the recommendation's reason when it is the GPU tier: "This computer's graphics can run the large model quickly.") |
| Use the graphics card | checkbox, on by default; shown only on a build and a computer where Whisper can use the GPU (ADR 0014, 5: Apple Silicon first, Vulkan later) | "Faster with large models. Acta uses the processor when the graphics card cannot be used." |
| Language | dropdown: "English or French, detected" / "English" / "French" | "Pick one if dictation mistakes one language for the other." (`voice.settings.language_hint`) |
| Microphone | dropdown: "System default" then the input devices by their system names | the level bar of the button, so the right device can be checked by speaking. It is live only while the pointer is over this row or its dropdown has the focus or is open; Settings being open, or the row being on screen, never opens the microphone (privacy: the system's microphone sign and, on macOS, its permission prompt; decided 2026-11-22). It stops within a second after. |
| Shortcut | no control; the kbd hint `Mod+Shift+Space` at the right | "Press once to start and again to stop, or hold it while you speak." (`voice.settings.shortcut_hint`) |

**Model list.** Under the Model row, one row of 44 per model of ADR 0014, 4 (Tiny, Base, Small, Large turbo), no box: name in `type.ui`, then `type.small` `text.tertiary` "31 MB · for older computers; rough in French" / "60 MB · fast; fair accuracy" / "190 MB · good for English and French" / "574 MB · most accurate; best with a graphics card"; "Recommended" in `type.small` `text.secondary` after the name of the recommended one; at the right its state and action:
- not downloaded: ghost "Download" (`voice.model.download`);
- downloading: the bar (80 wide) and "45%" `tnum`, ghost "Cancel";
- downloaded and in use: `check` 14 in `success` and "In use" (`voice.model.in_use`), ghost `ellipsis` menu with "Delete model" (refused while a take runs);
- downloaded, not in use: ghost "Use" (`voice.model.use`) and the menu.
Under the list, `type.small` `text.tertiary`: "Models are kept in {path}. Free on this disk: {free}." with the path in `type.code` and "Show in Explorer" (Finder, file manager), and, when models take space: "Downloaded models: 764 MB." Deleting a model is undoable only by downloading it again, so it asks (6.4): "Delete Whisper Large turbo? It will be downloaded again (574 MB) if you choose it later." Cancel focused, Delete.

The microphone device is stored by its system id in the database's local values, not in `preferences.json` (device names can carry a person's name, 4.7); engine, model, language and the graphics checkbox are settings and live in `preferences.json`.

### Keys and palette

- `Mod+Shift+Space`: start or stop dictating, only while an editor above has focus (6.1).
- Palette: "Dictate" (`cmd.dictate`; listed only while such an editor has focus or was the last focused element of the active page, and acts on it), "Stop dictating" (while a take runs), "Voice settings" (`cmd.voice_settings`).
- `Esc` while listening: stop and throw away what is not yet written (above).

### Review lines

The **Dictation** block of the review checklist ([checklist.md](checklist.md), 9).
