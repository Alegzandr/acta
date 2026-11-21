---
name: triage
description: Sort Acta's inbox of notes into epics and topics with the user.
disable-model-invocation: true
---
# Triage the Acta inbox

1. Call `find(kind="note")` to read the inbox. Page with `cursor` if it says there are more.
2. Call `find(kind="epic")` to see the existing epics.
3. Group the notes by subject. For each group, propose either an existing epic and a new topic, or a new epic. Name each note id in the proposal. Notes that are not work (ideas to drop, duplicates) are listed separately.
4. Show the proposal as a short list and ask the user to confirm or adjust it. Do not create anything before they agree.
5. Create everything in one `create` call: epics first, topics with `epic` and `from_notes` so the notes are filed.
6. Report the new ids in one line.
