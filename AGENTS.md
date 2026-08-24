# maestro — TypeScript rewrite (greenfield branch)

This branch is a from-scratch rewrite; the old Rust tree lives on `main`.
The spec bundle is the only authority here. Read, in order:

1. `.spec-workflow/active/maestro-ts-rewrite/SPEC.md` — contract: red tests, verb surface, anti-goals
2. `.spec-workflow/active/maestro-ts-rewrite/NOTES.md` — locked decisions + flagged defaults
3. `.spec-workflow/adr/` — 0001–0004, the recorded why
4. `.spec-workflow/active/maestro-ts-rewrite/VERIFY.md` — the checklist that defines done

Rules:

- Toolchain: bun only (runtime, `bun:sqlite`, `bun:test`).
- Kernel is mechanism only — no policy vocabulary in `src/kernel/` (Anti-goal A2).
- Test-first: implement against the SPEC red-test list; VERIFY rows prove done.
- Append working notes to NOTES.md; never rewrite its history.
- Commit per verified step on this branch; never push unasked.

<!-- maestro:begin -->
Live maestro state is injected by hooks. Use `maestro status` for the current session view and `maestro ready` for available work.
Track work with `maestro work add|start|done`; method depth: `maestro recipe show work`.
If no harness hook fired, run `maestro hook record --event SessionStart` and read the brief from stdout.
Failed commands print a JSON error envelope on stderr and exit nonzero; when the fix is mechanical, the message names the next command to run.
<!-- maestro:end -->
