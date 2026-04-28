---
name: maestro-qa
description: One-time QA installer for a repo. Scaffolds `.maestro/qa/` (config, sub-skills, sidecars, templates), symlinks `qa/` and per-app sub-skills into `.claude/skills/` and `.codex/skills/`, and optionally writes a GitHub Actions workflow. Use when the user runs `/maestro-qa`, says "install QA", "set up QA in this repo", or asks to bootstrap a project-local `/qa` runtime.
user-invocable: true
---

# Maestro QA

You are running the QA installer. After install, the user invokes the project-local `/qa` skill (which this installer writes) to run flows on demand. This skill itself does not run QA flows; it scaffolds them.

This is the canonical v1 install behavior. Future `maestro qa …` CLI subcommands must mirror this skill rather than invent a separate model.

## Core Contract

- Pure regenerate. Re-running this installer overwrites every generated file deterministically. Two exceptions: `.maestro/qa/.install-progress.yaml` (append-only resume cursor) and the sidecar trees under `.maestro/qa/quirks/` and `.maestro/qa/failure-modes/` (preserved across runs). `qa-results/` is also preserved.
- Factory parity at the surface, maestro-native underneath. The questionnaire, modality set, and `test_tool` protocol names match Factory's `/install-qa` design; every path and convention is maestro's.
- The agent does the work. Read, Glob, Grep, Write, and Bash are the only tools used. There is no maestro CLI subcommand for this skill in v1.
- Never store real secrets. Reference secrets by env var name only. Tell the user which GitHub secrets to add.
- BLOCKED with remediation if a primitive is missing. If a render-check command, browser tool, or device runner is required by the chosen modality but not present, do not silently skip. Generate a flow that prints a clear `BLOCKED: <what is missing> — <how to install>` instead of a fake pass.

## Generated Layout

Canonical (single source of truth, repo-tracked):

```
.maestro/qa/
├── config.yaml                    # flows, integrations, env-var refs
├── REPORT-TEMPLATE.md             # template the runtime fills per run
├── .install-progress.yaml         # resume cursor (append-only)
├── .gitignore                     # ignore qa-results/ etc.
├── skills/
│   ├── qa/
│   │   └── SKILL.md               # orchestrator runtime, user-invocable
│   └── qa-<app>/                  # one per detected modality
│       └── SKILL.md               # per-app runtime, user-invocable
├── quirks/                        # preserved across runs
└── failure-modes/                 # preserved across runs
```

Symlinks (point at canonical paths so `/qa` works in both agents):

```
.claude/skills/qa            -> ../../.maestro/qa/skills/qa
.claude/skills/qa-<app>      -> ../../.maestro/qa/skills/qa-<app>
.codex/skills/qa             -> ../../.maestro/qa/skills/qa
.codex/skills/qa-<app>       -> ../../.maestro/qa/skills/qa-<app>
```

Top-level (preserved, single overwritten report per run):

```
qa-results/
└── report.md
```

Optional CI:

```
.github/workflows/qa.yml
```

## Install Flow

### 1. Check resume state

Read `.maestro/qa/.install-progress.yaml` if present. If it exists and has `complete: false`, ask the user: resume from the recorded category, or start fresh? On fresh, delete the progress file before continuing. On resume, skip categories already marked done and start from the next pending one.

### 2. Deep codebase analysis

Before asking any question, gather evidence. Use this order:

1. Glob for manifests: `package.json`, `pyproject.toml`, `Cargo.toml`, `go.mod`, `pom.xml`, `build.gradle`, `Package.swift`, `Gemfile`, `composer.json`.
2. Detect modalities (see `## Modality Detection` and `reference/detection.md`).
3. Detect tech stack (frameworks, runtimes, languages) from manifests and lockfiles.
4. Detect integrations (databases, third-party APIs, queues) from imports, env files, and config.
5. Detect CI provider (`.github/workflows/`, `.gitlab-ci.yml`, `.circleci/`, `.buildkite/`).
6. Detect existing tests (test directories, framework configs).
7. Identify candidate critical user flows from route files, CLI command registries, or app entry points.

Hold the findings as a structured record (modalities, stack, integrations, CI, tests, candidate flows). Do not write it to disk yet.

### 3. Present findings

Show the user the detected record before asking anything. Keep it short. They confirm or correct it; corrections feed into the questionnaire defaults.

### 4. Run questionnaire

Use AskUserQuestion. One category at a time. After each category, write the recorded answers into `.maestro/qa/.install-progress.yaml` so a later resume picks up cleanly. Categories and conditional rules live in `reference/questionnaire.md`. Modalities are not asked — they come from the detection pass and the user's confirmation in step 3. `test_tool` is auto-derived from each modality and is user-editable post-install. The eight categories:

1. Critical flow confirmation
2. Default QA target (environments + default)
3. Personas and roles
4. Cleanup strategy (with `protected_envs`)
5. Failure-learning strategy
6. External sandbox/test credentials (per detected integration)
7. ImageMagick install (only if a visual modality is detected)
8. GitHub Actions generation + agent runtime (only if `.github/` exists)

### 5. Generate canonical files

Write the canonical tree. Items 1, 2, 4, 5, 6, 7 are independent — issue them in a single parallel batch. Item 3 (`.install-progress.yaml` with `complete: true`) is written last after the others succeed so a crash mid-write never leaves a "complete" cursor for incomplete output.

1. `.maestro/qa/config.yaml` from `reference/templates/config.yaml.tmpl`.
2. `.maestro/qa/REPORT-TEMPLATE.md` from `reference/templates/report-template.md.tmpl`.
3. `.maestro/qa/.install-progress.yaml` from `reference/templates/install-progress.yaml.tmpl` with `complete: true`.
4. `.maestro/qa/skills/qa/SKILL.md` from `reference/templates/orchestrator-skill.md.tmpl`.
5. One `.maestro/qa/skills/qa-<app>/SKILL.md` per chosen modality from `reference/templates/app-skill.md.tmpl`. The template's `{{flow_protocol_block}}` slot embeds the matching `flow-*.md.tmpl` (per `test_tool`); the template's `{{flow_bodies_block}}` slot renders one `## Flow: <anchor>` section per flow with `**Persona:**`, `**Goal:**`, `### Steps`, `### Expect`, `### Cleanup` subsections matching the protocol's body shape. Flow registry entries (in `config.yaml`) and flow body anchors (in `qa-<app>/SKILL.md`) are emitted from one template per flow; never let them drift.
6. Sidecar dirs: `.maestro/qa/quirks/` and `.maestro/qa/failure-modes/`. Create the directories and seed each one per app with an empty `<app_slug>.md` rendered from `reference/templates/quirks.md.tmpl` and `reference/templates/failure-modes.md.tmpl` respectively. The runtime appends entries on failure; the installer never rewrites these files once created.
7. `.maestro/qa/.gitignore` containing `qa-results/` (the ignore lives inside `.maestro/qa/` so removing the QA install removes its ignore rules cleanly).

Templates use `{{placeholder}}` substitution. The agent does the substitution; there is no templating engine to install. Common slots: `{{app_slug}}`, `{{modality}}`, `{{render_check_cmd}}`, `{{integration_list}}`, `{{cleanup_strategy}}`, `{{failure_learning_policy}}`.

### 6. Create symlinks

For each generated `qa/` and `qa-<app>/` directory under `.maestro/qa/skills/`, link it under `.claude/skills/` and `.codex/skills/`. The link calls have no inter-dependencies — issue them in a single parallel batch.

```bash
# Linux/macOS. Use relative targets so links survive a repo move:
ln -s ../../.maestro/qa/skills/qa        .claude/skills/qa
ln -s ../../.maestro/qa/skills/qa        .codex/skills/qa
ln -s ../../.maestro/qa/skills/qa-<app>  .claude/skills/qa-<app>
ln -s ../../.maestro/qa/skills/qa-<app>  .codex/skills/qa-<app>
```

Decision table (mirrors `ensureSkillLink` in the maestro CLI):

- Parent dir (`.claude/skills/`, `.codex/skills/`) does not exist: `mkdir -p` first.
- Path is already a symlink pointing at the canonical target: leave it alone.
- Path is a symlink pointing elsewhere within the maestro tree (e.g. an old target): remove and recreate.
- Path is a symlink pointing outside the maestro tree (user-authored override): warn to stderr and leave it in place. Replacing it would silently destroy the user's override.
- Path is a real directory holding our managed content (manifest matches): migrate it to a symlink, preserving any user edits.
- Path is a real directory with no manifest (user-authored): warn to stderr and leave it. Do not create the link.
- Path is a plain file: warn to stderr and leave it. Do not clobber.
- Windows: same rules. `symlink(target, link, "junction")` is used under the hood, which works without admin or Developer Mode as long as the link and target sit on the same volume (the maestro tree always satisfies this).

### 7. Optional: install ImageMagick

Only if the user opted in during the questionnaire and a visual modality (`render-check`, `agent-browser`, `agent-device`, `electron`) was enabled. Run `brew install imagemagick` via Bash on macOS, otherwise print the platform-specific install hint and skip. Never fail the install on this step.

### 8. Optional: write GitHub Actions workflow

Only if `.github/` exists and the user opted in via questionnaire category 8.

1. If `.github/workflows/qa.yml` already exists, copy it to `.github/workflows/qa.yml.bak.<timestamp>` first.
2. Write `.github/workflows/qa.yml` from `reference/templates/workflow-qa.yml.tmpl`.
3. Substitute the agent-runtime placeholders based on `progress.ci.agent_runtime`:

   **For `claude`:**
   - `{{ci_agent_install_block}}` →
     ```yaml
     - name: Install Claude Code
       run: npm install -g @anthropic-ai/claude-code
     ```
   - `{{ci_agent_invoke_command}}` → `claude -p "/qa" --dangerously-skip-permissions`
   - `{{secret_env_block}}` → `          ANTHROPIC_API_KEY: ${{ secrets.ANTHROPIC_API_KEY }}`

   **For `codex`:**
   - `{{ci_agent_install_block}}` →
     ```yaml
     - name: Install Codex
       run: npm install -g @openai/codex-cli
     ```
   - `{{ci_agent_invoke_command}}` → `codex run "/qa" --headless`
   - `{{secret_env_block}}` → `          OPENAI_API_KEY: ${{ secrets.OPENAI_API_KEY }}`

4. Reference any integration secrets by name only (e.g., `${{ secrets.QA_BROWSER_TOKEN }}`). Never bake values in.

### 9. Validate generated files

Before reporting success:

- `.maestro/qa/config.yaml` parses as YAML and matches the schema in `reference/config-schema.md`.
- Every generated `SKILL.md` starts with valid frontmatter (`---`, `name:`, `description:`, `user-invocable: true`, `---`).
- Sidecar dirs `quirks/` and `failure-modes/` exist.
- If a workflow was written, it parses as YAML.

Step 6 already handled the symlink decision table; do not re-check link resolution here unless the Windows fallback or a user-symlink-pointing-outside warning was emitted, in which case the final response must surface those by name.

If any check fails, surface it in the final response as a blocker.

### 10. Final response

Report:

- files created or updated (full canonical paths)
- symlinks created and any that were skipped with the reason
- GitHub secrets the user must add (referenced by name)
- manual setup remaining (e.g., browser auth, device emulator setup)
- the call to action: "Run `/qa` to execute QA flows."

## Modalities

The supported set: `cli`, `web`, `api`, `mobile`, `desktop`, `tui`. A repo can have multiple. The questionnaire's modality category presents the detected set as defaults; the user toggles each on or off.

Detection rules (per-modality signals, integrations, CI provider, app-slug derivation): `reference/detection.md`. Read it once during step 2 and hold the structured findings record for the rest of the install — do not re-glob the same evidence in later steps.

## Questionnaire Categories

Run sequentially via AskUserQuestion. Save progress after each. Full prompts and conditional rules live in `reference/questionnaire.md`. Canonical enum values live in `reference/config-schema.md` — the schema is authoritative if any drift appears.

1. **Critical flow confirmation** — confirm/edit the candidate flow list detected in step 2.
2. **Default QA target** — `environments` (each with `url` and optional `restrictions`) plus `default_target`.
3. **Personas and roles** — at least one persona with `name`, `test_focus`, and optional `cannot_do`.
4. **Cleanup strategy** — `manual | auto_after_run | ephemeral_env | none` plus `protected_envs` (always includes `production`).
5. **Failure-learning strategy** — `suggest_in_report | auto_commit | open_pr`.
6. **External sandbox/test credentials** — per detected integration, choose `mock | live | skip`. `live` requires naming env vars.
7. **ImageMagick** — only asked if a visual modality was detected.
8. **GitHub Actions + agent runtime** — only if `.github/` exists; choose `provider` and (if `github-actions`) `agent_runtime` (`claude | codex`).

## test_tool Protocols

`test_tool` per app is auto-derived from the modality at install time and recorded in `apps.<slug>.test_tool`; the user can edit it post-install in `config.yaml`.

| Protocol | One-line behavior | Default for modality |
|---|---|---|
| `shell` | Runs a shell command; passes if exit code matches and stdout contains expected pattern. | `cli` |
| `render-check` | Runs the project's render-check command (e.g., `mission-control --render-check --size 120x40`); passes if it exits 0 and emits a non-empty frame. | `tui` |
| `curl` | Issues an HTTP request; passes if status code matches and body satisfies a JSON or substring assertion. | `api` |
| `agent-browser` | Drives a headless browser via the project's agent-browser tool to assert page state. | `web` |
| `agent-device` | Drives a mobile simulator/emulator via agent-device to assert screen state. | `mobile` |
| `electron` | Drives an Electron app via agent-browser's CDP attach mode. | `desktop` |

The agent reads this table when generating per-app `SKILL.md` flows. Each protocol's body shape and execution rules live in `reference/templates/flow-*.md.tmpl`.

## Flows: Registry vs Body

The `flows[]` array in `config.yaml` is a **registry**, not a body. Each entry has only `{id, name, anchor, persona, blocked_if_missing[]}`. The executable body — `### Steps`, `### Expect`, `### Cleanup` — lives as a markdown section in the corresponding `qa-<app>/SKILL.md` under `## Flow: <anchor>`.

The installer emits both pieces from one template per flow, so the anchor matches between YAML and markdown on initial generation. Validators (and the runtime) read the registry to enumerate flows, then jump to the SKILL.md heading by anchor to execute. Anchor stability is the generator invariant: never rename a heading without updating the registry's `anchor`, and vice versa.

## Pure Regenerate vs Append-Only

Regenerated on every run (overwrite without prompting):

- `.maestro/qa/config.yaml`
- `.maestro/qa/REPORT-TEMPLATE.md`
- `.maestro/qa/skills/qa/SKILL.md`
- `.maestro/qa/skills/qa-<app>/SKILL.md` (each)
- `.maestro/qa/.gitignore`

Preserved across runs (append-only or user-owned):

- `.maestro/qa/.install-progress.yaml` (resume cursor; deleted only on user-confirmed fresh start)
- `.maestro/qa/quirks/<app_slug>.md` (one per app; installer seeds the file once and never rewrites it)
- `.maestro/qa/failure-modes/<app_slug>.md` (same as quirks)
- `qa-results/` (runtime output, not installer output)
- `.github/workflows/qa.yml` (regenerated only with explicit opt-in; previous file is backed up first)

User-editable regions inside regenerated files are wrapped in managed markers and preserved during overwrite:

```
<!-- maestro-qa:user:start -->
... user content survives regeneration ...
<!-- maestro-qa:user:end -->
```

This idiom matches `maestro-setup`'s `<!-- maestro-setup:start -->` markers. The installer treats anything between `:user:start` and `:user:end` as opaque user content and round-trips it.

## Sub-skill Frontmatter

Generated `qa/SKILL.md` (orchestrator runtime):

```yaml
---
name: qa
description: Run QA flows declared in .maestro/qa/config.yaml. Dispatches per-app runners (qa-<app_slug>...) and writes qa-results/report.md. Use when the user says "/qa", "run QA", "qa this repo", or asks to verify the chosen critical flows.
user-invocable: true
---
```

Generated `qa-<app_slug>/SKILL.md` (per-app runner):

```yaml
---
name: qa-<app_slug>
description: Run the <modality> QA flows for this repo. Invoked by /qa or directly. Reads its flows from .maestro/qa/config.yaml under apps.<app_slug>.
user-invocable: true
---
```

Both must include `user-invocable: true`. This matches the `maestro-handoff` precedent for skills the user is meant to invoke directly.

## Future CLI Contract

The later CLI must mirror this skill. Out of scope for v1, listed here so a future implementer matches the install model rather than reinventing it:

- `maestro qa install --json`
- `maestro qa install --resume --json`
- `maestro qa check --json` (lint config + symlinks)
- `maestro qa modalities --json` (re-run detection without writing)

Do not design extra CLI behavior while running this skill.

## Reference

- `reference/config-schema.md`: canonical `config.yaml` schema, field by field.
- `reference/questionnaire.md`: full prompts for all 8 categories with conditional rules.
- `reference/detection.md`: modality, integration, and app-slug detection rules.
- `reference/templates/`: every file the installer writes, as a `.tmpl` with `{{placeholder}}` slots.
