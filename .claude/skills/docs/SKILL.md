---
name: docs
description: >-
  Update repository documentation to match the current state of the codebase.
  Local replacement for the remote /docs command (which needs the Claude GitHub
  app). Use when the user says /docs, "update the docs", "sync the README",
  "document this feature", or asks you to refresh docs to reflect current code.
  Works inline in the current session and edits files in the working tree.
---

# docs

Local, in-session documentation updater. Replaces the remote `/docs` command so you do not need the Claude GitHub app or a cloud session. No PR is opened. Changes are left staged in the working tree for the user to review and commit.

## Scope the task

Read the user's request literally before touching anything.

- Full repo sync ("update the docs", "sync README"): update `README.md` and relevant `docs/*.md`. Prefer delegating the mechanical work to the `doc-sync` skill if it is available in this session.
- Specific file ("update AGENTS.md", "refresh CLAUDE.md"): edit only that file.
- Feature doc ("document the task feature"): extend the existing doc for that feature. Create a new file in `docs/` only if the user explicitly asked for a new file.
- No argument / vague ask: ask one direct question about which file or area to update, then stop.

## Ground every claim in current state

Never document from memory. Every factual claim you write (file path, function name, flag, command, version, behavior) must trace to something you just read or ran.

Required reads before writing:

- `AGENTS.md` and `CLAUDE.md` at the repo root: source of truth for conventions, style, and project rules.
- For feature docs: that feature's `index.ts`, `services.ts`, and `commands/` to see the real public surface.
- For CLI docs: run the binary's `--help` to capture real flags and argument order. Do not invent flags.
- For version or release docs: check `package.json` and `git log --oneline -20` for recent commits.

If you cannot verify a claim right now, mark it `(unverified)` inline or delete it. Do not ship an unverified specific.

## Edit surgically

- Prefer editing existing files over creating new ones.
- Match the existing heading style, tone, and voice.
- Do not reformat adjacent content that is not part of your change.
- Do not "improve" code or comments you happen to read while gathering context. That is a separate task.
- Plain markers only: `[!]`, `[ok]`, `-->`, `[x]`. No emoji. No em-dashes. Project rule from CLAUDE.md.

## Verify after writing

After each file edit:

- Re-read the changed section against the file or command output you read.
- If you claimed a command exists, run it with `--help` and confirm the flags.
- If you referenced a path, confirm the file exists with `Glob` or `Read`.

## Do not commit

Leave changes staged in the working tree. The user reviews and commits.

If the user explicitly asks for a commit, use Conventional Commits: `docs(scope): short summary`. Do not mix doc changes with unrelated behavior changes in one commit.

## Guardrails

- Do not touch `.maestro/memory/**`, `.maestro/tasks/**`, or `.maestro/missions/**`.
- Do not touch files under `~/.claude/projects/**/memory/**`.
- Read-only tools (`Read`, `Grep`, `Glob`, `git log`, `--help`) are always fine. Any mutation outside doc files requires explicit user authorization.
- Never push, force-push, amend, or rewrite history as part of a docs task.

## Report

End with a brief report, at most 6 bullets:

- Files changed, absolute paths.
- One line per file: what changed and why.
- Anything flagged `(unverified)` and why you could not verify it.
- Suggested commit message if the user wants to commit.
