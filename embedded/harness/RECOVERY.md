---
version: 1.0.0
---

# Maestro Recovery Runbook

Use this file when the `maestro` binary or `.maestro` records are broken enough
that normal status commands cannot explain the next step.

## Repair schema drift

If a command reports `expected maestro.feature.v2, found maestro.feature.v1` or
`expected maestro.task.v2, found maestro.task.v1`, run:

```bash
maestro migrate-v2
maestro doctor
```

If `doctor` still reports missing scaffold files, run:

```bash
maestro init --merge
maestro doctor
```

## Repair missing resources

If bundled skills, hooks, harness files, or this runbook are missing, run:

```bash
maestro sync
maestro doctor
```

Use `maestro sync --dry-run` first when you need a preview.

## Check binary provenance

From the repository root:

```bash
maestro version
git log -1 --oneline
git status --short --branch
```

If `maestro version` cannot run, inspect the built binary or installed path from
the shell:

```bash
command -v maestro
git log -1 --oneline
```
