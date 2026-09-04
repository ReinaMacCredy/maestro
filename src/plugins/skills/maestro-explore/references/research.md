# Research

This mode is for facts that **decide something**: a fork in the design waits
on the answer. A small fact needed mid-`maestro-work` or mid-`maestro-diagnose`
just to keep coding (an API detail, a version quirk) needs no ceremony: look
it up inline against primary sources, record the finding and its link with
`maestro work note <id>`, and move on.

Spin up a **background agent** to do the research, so you keep working while
it reads.

Its job:

1. Investigate the question against **primary sources**: official docs,
   source code, specs, first-party APIs, not a secondary write-up of them.
   Follow every claim back to the source that owns it.
2. Write the findings to a single Markdown file, citing each claim's source.
3. Land the findings where they will be used: a settled fork becomes a
   `maestro decision draft ... --rationale "<why + source>"` then `lock`;
   working evidence and the file's link go to `maestro work note <id>` and,
   in a bundle, NOTES.md. Only when no work item exists, save the file where
   the repo already keeps such notes and say where.
