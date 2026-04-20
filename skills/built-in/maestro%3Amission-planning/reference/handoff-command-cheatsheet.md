# Handoff Command Cheatsheet

This is the final reference for mission planning. Once the mission exists and the readiness check passes, your job is to draft the exact `maestro handoff` command for the first external worker. Do not run it from the planning skill.

## The command shape

```bash
maestro handoff "<task>" \
  [--provider codex|claude] \
  [--model <model>] \
  [--worktree [slug]] \
  [--base <branch>] \
  [--name <title>] \
  [--wait] \
  [--json]
```

## Provider mapping

Be explicit in the drafted command even though Codex is the CLI default.

| Feature `agentType` | Provider flag |
|---|---|
| `codex-cli` | `--provider codex` |
| `claude-code` | `--provider claude` |
| `subagent` | Do not draft a native handoff command for this feature |
| `human` | Do not draft a native handoff command for this feature |

## What the task string must contain

The quoted task string should be self-sufficient. It should name:

1. the mission id
2. the feature id or title
3. the expected outcome
4. the requirement to run the listed verification steps before stopping

Good task string:

```text
Implement feature auth-impl for mission mis_01h8k2f9. Split session validation from permission checks while preserving the existing middleware signature, and run the listed verification steps before stopping.
```

Bad task string:

```text
finish auth stuff
```

The bad version throws away the mission anchor, the feature anchor, the outcome, and the verification requirement.

## When to add flags

- Add `--worktree [slug]` when the worker should operate in an isolated sibling checkout.
- Add `--base <branch>` only when you are also using `--worktree`.
- Add `--wait` when the operator wants a synchronous run that returns an exit code before the command exits.
- Add `--json` when the next step is automation or structured inspection.
- Add `--name <title>` when the launch needs a stable operator-facing label.
- Add `--model <model>` only when the default is wrong for this feature.

## Good examples

Codex implementation:

```bash
maestro handoff \
  "Implement feature auth-impl for mission mis_01h8k2f9. Build the first working authentication slice and run the listed verification steps before stopping." \
  --provider codex
```

Claude review in a sibling worktree:

```bash
maestro handoff \
  "Review feature auth-impl for mission mis_01h8k2f9. Check for regressions, missing tests, and scope drift before stopping." \
  --provider claude \
  --worktree auth-review
```

Automation-friendly foreground run:

```bash
maestro handoff \
  "Finish feature auth-impl for mission mis_01h8k2f9 and return only after the verification steps pass." \
  --provider codex \
  --wait \
  --json
```

## Common mistakes

- Drafting the command for a `subagent` or `human` feature
- Relying on the default provider instead of writing the explicit mapping
- Using `--base` without `--worktree`
- Writing a vague task string that omits the mission or feature anchor
- Auto-launching the command inside the planning skill instead of handing it back to the operator
