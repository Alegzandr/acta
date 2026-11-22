---
name: wrap
description: Wrap up a work session in Acta - log what was done, refresh the state, set the status. Use at the end of a task or when the user says they are stopping.
---
# Wrap up in Acta

1. Recall what happened since the last `log` on the focus topic: decisions, finished steps, dead ends.
2. Log each decision that is not logged yet with `log(type="decision", ...)`: what was chosen and why, one to three sentences.
3. Log the progress in one `log(type="progress", text=..., state=...)` call. The state says what is true now and what comes next, in at most four lines.
4. If the topic is finished, blocked or dropped, pass `status` in that same call. If you set it `done` and it has a person to report to (`· report to Marie` in its brief), add one line for the user: "Report to Marie: ..." with what to tell them.
5. If you learned a durable fact worth not relearning, save it with `kb_write` (search with `kb` first).
6. Tell the user in one line what was logged.
