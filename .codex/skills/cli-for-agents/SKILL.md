---
name: cli-for-agents
description: "Design or review CLIs so Codex and other coding agents can run them reliably in terminals and automation: non-interactive flags, layered --help with real examples, stdin and pipeline support, fast actionable errors, idempotency, dry-run, confirmation bypass flags, and predictable command structure. Use when building a CLI, adding subcommands, writing help text, improving terminal automation, or making command UX safe for headless agent use."
---

# CLI for Agents

Prefer command patterns that work headlessly, compose in pipelines, and fail in ways an agent can recover from quickly.

## Non-Interactive First

- Express every required input as a flag or flag value.
- Fall back to interactive prompts only when flags are missing and an interactive session is appropriate.
- Avoid arrow-key menus, timed prompts, and flows that block stdin-driven execution.

Bad: `mycli deploy` prompts for an environment.
Good: `mycli deploy --env staging`

## Layer Help Incrementally

- Keep top-level help short and let each subcommand own its own `--help`.
- Expect agents to discover commands step by step: `mycli`, then `mycli deploy --help`.
- Do not print the entire manual on every invocation.

## Make `--help` Copy-Pasteable

- Add `--help` to every subcommand.
- Include real `Examples:` blocks with valid invocations, not placeholders alone.
- Show the common safe path first, then destructive or forceful variants.

```text
Options:
  --env     Target environment (staging, production)
  --tag     Image tag (default: latest)
  --dry-run Preview the deployment plan
  --yes     Skip confirmation

Examples:
  mycli deploy --env staging --tag v1.2.3 --dry-run
  mycli deploy --env staging --tag v1.2.3
  mycli deploy --env production --tag v1.2.3 --yes
```

## Support Flags, Stdin, and Pipelines

- Accept stdin where it is a natural input path.
- Avoid positional argument patterns that are easy to scramble.
- Keep outputs chainable so one command can feed another.

Examples:
- `cat config.json | mycli config import --stdin`
- `mycli deploy --env staging --tag "$(mycli build --output tag-only)"`

## Fail Fast With Actionable Errors

- Exit immediately on missing required flags or invalid combinations.
- Print a short error plus a correct example invocation.
- Prefer guidance that helps the next command succeed without opening external docs.

```text
Error: missing required flag --tag
Try:
  mycli deploy --env staging --tag <image-tag>
List tags:
  mycli build list --output tags
```

## Design for Retries

- Make successful commands safe to retry.
- Return explicit no-op or already-done messages instead of duplicating side effects.
- Keep side effects narrow and obvious so agents can reason about re-runs.

## Guard Destructive Actions

- Provide `--dry-run` or an equivalent preview mode.
- Provide `--yes` or `--force` for non-interactive confirmation bypass.
- Keep the default behavior safe for humans while leaving a clear headless path for automation.

## Keep Command Shapes Predictable

- Use one command grammar consistently across resources and verbs.
- Reuse option names when they mean the same thing.
- Avoid making one command tree resource-first and another verb-first unless there is a strong reason.

## Return Machine-Useful Success Output

- Include IDs, URLs, paths, counts, or durations in success output.
- Plain text is fine if the data is stable and easy to parse.
- Do not rely only on decorative output or prose summaries.

```text
deployed v1.2.3 to staging
url: https://staging.myapp.com
deploy_id: dep_abc123
duration: 34s
```

## Review Checklist

When reviewing an existing CLI, check for:

- A complete non-interactive path
- Short layered help instead of one giant manual
- Real `Examples:` blocks on `--help`
- Sensible stdin and pipeline support
- Actionable errors with correct invocations
- Retry-safe and idempotent behavior
- `--dry-run` plus confirmation bypass flags for risky actions
- Consistent command and option naming
- Success output with machine-useful fields
