---
name: triage
description: Turn the notes in Acta's inbox into epics, topics and timeline entries, with the user's approval. Run as /acta:triage; add exactly "go" to apply the plan without asking.
argument-hint: "[go]"
disable-model-invocation: true
---
# Triage the Acta inbox

1. Read: `find(kind="note")` lists the open notes (page with `cursor` while it says more). Then `find(kind="epic")` and `find()` for the epics and open topics they might belong to.
2. Group notes that are about the same thing. For each group pick one destination:
   - a new topic (in an existing epic `E-n`, or in a new epic of the plan),
   - a new epic, when the group is a whole area of work,
   - an entry on an existing topic `T-n`, when the note adds to work already tracked,
   - dismiss, for duplicates, done items and noise.
   Every note goes to exactly one destination. Titles are short and say the outcome; do not rewrite the notes, Acta keeps their text.
3. Propose the plan as one compact list, one line per destination, and stop:
   `new topic "Quick capture" in E-2 <- N-3 N-5 → Marie`
   `new epic "Sync" <- N-7`, `entry on T-12 <- N-4`, `dismiss N-6 (duplicate of N-3)`.
   A note that names someone to report to shows the person (`→ Marie`): the topic or epic made from it takes that person (an existing topic keeps its own). When notes going to one destination name different people, say so in the plan and let the user settle it; set `report_to` on the item to the person they choose.
   A note naming someone Acta does not know as a contact shows `→ Paul (new contact)`; nothing is created before the plan is applied, and then the contact goes in the same `create` call (`{"kind": "contact", "title": "Paul"}`, named by later items as `"@k"`).
   The arguments of this command are: `$ARGUMENTS`. Only if they are exactly `go`, apply the plan right away. Otherwise ask the user to confirm or adjust it, and write nothing before they agree.
4. Apply the agreed plan in ONE `create` call: `items` with `from_notes` on every item, `epic="@k"` to put a topic in the epic of item k, `kind="entry"` with `topic` for existing topics, and `dismiss` for the rest. It is all or nothing: if it is refused, fix the named item and call again.
5. Report the ack line as is: new ids, the notes each one took, and whose person was carried or not. Topics from notes start as draft for the user to refine in Acta, where every filed note links to its topic and can be put back in the inbox.
