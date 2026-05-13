# Schedule recipes

Maestro is passive: it runs only when invoked. There is no daemon, cron, or
watcher inside the `maestro` binary. Anything that needs to run on a clock
lives in a host runtime that calls `maestro` as a subprocess.

This doc collects the three host runtimes we support today.

## When to schedule what

| Cadence | Use case | Recipe |
|---|---|---|
| On every PR | Verdict + arch-lint + cross-task conflict | GitHub Actions workflow (already shipped at `docs/ci-integration.md`) |
| Nightly | Doc-gardening sweep, slop scan | GitHub Actions cron |
| Per-session | Session start / exit anchors | Claude Code hook |
| Per-edit | `maestro task verify` after substantive edits | Claude Code skill prompt |

## Recipe 1 — GitHub Actions cron

Use this for repo-wide sweeps that should run on a schedule and open a fixup PR.

```yaml
# .github/workflows/maestro-nightly.yml
name: Maestro nightly

on:
  schedule:
    - cron: "17 4 * * *"   # 04:17 UTC daily
  workflow_dispatch:

permissions:
  contents: write
  pull-requests: write

jobs:
  doc-gardening:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - uses: oven-sh/setup-bun@v1
        with:
          bun-version: latest
      - run: bun install --frozen-lockfile
      - run: bun run release:local
      - name: Doc-gardening sweep
        id: sweep
        run: |
          ./dist/maestro gc doc-gardening --json > /tmp/sweep.json
          echo "stale=$(jq '.staleReferences | length' /tmp/sweep.json)" >> "$GITHUB_OUTPUT"
      - name: Open fixup PR if stale references found
        if: steps.sweep.outputs.stale != '0'
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        run: |
          # Convert findings into a markdown body for a tracking issue.
          jq -r '.staleReferences[] | "- `\(.file):\(.line)` → `\(.reference)` (\(.kind))"' /tmp/sweep.json \
            > /tmp/body.md
          gh issue create \
            --title "Nightly doc-gardening: ${{ steps.sweep.outputs.stale }} stale references" \
            --body-file /tmp/body.md \
            --label maestro-gc
```

Notes:
- The job runs `release:local` so the sweep runs against the latest build, not
  whatever `dist/` happens to be checked in.
- We open an Issue rather than a PR because doc-gardening fixes typically
  require human judgment (move? rename? delete the reference?). `gc slop-cleanup`
  (Phase 4) is the verb that opens fixup PRs.

## Recipe 2 — Claude Code session hook

Claude Code can invoke a shell command on session start and end via its
`hooks` configuration. This is the cleanest way to anchor `session start` /
`session exit` evidence without asking the agent to remember.

```jsonc
// ~/.claude.json (or .claude/settings.json in the repo)
{
  "hooks": {
    "SessionStart": [
      {
        "matcher": ".*",
        "hooks": [
          {
            "type": "command",
            "command": "maestro session start \"$CLAUDE_TASK_ID\" || true"
          }
        ]
      }
    ],
    "SessionEnd": [
      {
        "matcher": ".*",
        "hooks": [
          {
            "type": "command",
            "command": "maestro session exit \"$CLAUDE_TASK_ID\" || true"
          }
        ]
      }
    ]
  }
}
```

Notes:
- `|| true` keeps a missing or non-claimed task from breaking the session.
- `CLAUDE_TASK_ID` is the convention for binding a Claude Code session to a
  maestro task; export it from the shell that launches Claude Code.

## Recipe 3 — Agent skill prompt

Use a skill that *teaches the agent when to invoke* a verb, rather than running
it on a schedule. This is the right shape for verbs whose timing is contextual
(after a substantive edit; after compaction; before claiming work).

```markdown
<!-- skills/local/post-edit-verify/SKILL.md -->
---
name: post-edit-verify
description: After a substantive edit to src/, run `maestro task verify --task <id>` and read the verdict before continuing.
---

When you finish a coherent batch of edits to the codebase:

1. Run `maestro task verify --task <id>` (use the active task id).
2. Read the output. If the Trust Verifier reports any error-severity finding,
   fix it before claiming further progress.
3. If the diff is large enough that you've drifted from the contract, run
   `maestro contract show --task <id>` and consider a `contract amend`.

Skip this skill for trivial edits (typos, comments).
```

This is *guidance*, not a hook — the agent decides when to invoke. The trade-off
is reliability (the agent can forget) for context-awareness (no spurious runs
on documentation-only edits).

## Why no scheduler in maestro

A long-running daemon would have to:
- Track its own lifecycle (start/stop, crash recovery).
- Keep the `.maestro/` state consistent with what the agent thinks is true.
- Compete with the host runtime (GitHub Actions, Claude Code) for the same
  triggers.

All three concerns are already solved by the host runtimes. Adding a daemon
inside maestro would duplicate state machines and create a class of bugs
where "what maestro thinks is happening" diverges from "what actually ran."

The trade-off is that scheduling has to be configured per-host. The recipes
above are the supported shapes; anything else is custom.
