# `.maestro/qa/config.yaml` Schema

This is the canonical contract. The installer writes it, the project-local `/qa` runtime reads it, and a future `maestro:user-testing-validator` integration must round-trip it.

## Top-level shape

```yaml
version: 1
generated_by: maestro-qa
generated_at: <ISO-8601 timestamp>
project: <repo name>
modalities: [<modality>...]
environments:
  <env_name>:
    url: <url-or-local-path>
    restrictions: [<free-text>...]
default_target: <env_name>
personas:
  - name: <persona-name>
    test_focus: [<area>...]
    cannot_do: [<action>...]
apps:
  <app_slug>:
    modality: <modality>
    test_tool: <protocol>
    flows: [<flow>...]
integrations: [<integration>...]
policies:
  cleanup:
    strategy: manual | auto_after_run | ephemeral_env | none
    protected_envs: [<env_name>...]
  failure_learning: suggest_in_report | auto_commit | open_pr
  integration_default: mock | live | skip
ci:
  provider: none | github-actions
  agent_runtime: claude | codex
report:
  path: qa-results/report.md
```

## `project`

Free-form repo name. Defaults to the directory name; user can override during questionnaire category 2.

## `modalities`

A subset of `cli`, `web`, `api`, `mobile`, `desktop`, `tui`. Driven by the detection pass (not asked in the questionnaire). The user confirms or corrects detection in install-flow step 3.

## `environments` and `default_target`

`environments` is a map of named environments the QA runtime can target. Each entry has a `url` (or local path/command) and an optional `restrictions` list (e.g., `[read-only only, never create data]`). `default_target` names the entry to use when the runtime is invoked without an explicit target. Filled from questionnaire category 2 ("Default QA target").

## `personas`

List of user personas the flows are run under. Each persona has a `name`, a `test_focus` list (areas the persona cares about), and an optional `cannot_do` list (actions the persona is not authorized to perform — used by negative-path flows). Filled from questionnaire category 3.

## `apps.<app_slug>`

One entry per chosen modality. `<app_slug>` is derived from the manifest (see `detection.md`); when a repo has multiple apps of the same modality, slugs are suffixed (`web-marketing`, `web-admin`).

| Field | Type | Required | Notes |
|---|---|---|---|
| `modality` | enum | yes | One of the modality names. |
| `test_tool` | enum | yes | `shell`, `render-check`, `curl`, `agent-browser`, `agent-device`, `electron`. Auto-derived from modality at install; user-editable post-install. |
| `flows` | list | yes | Registry of flows; each entry references a body section in `qa-<app>/SKILL.md` by anchor. Empty list is invalid. |

## `flows[]` — registry only (body lives in `qa-<app>/SKILL.md`)

Each flow entry is a registry record; the executable body (steps, expectations, cleanup) lives as a markdown section in the corresponding `qa-<app>/SKILL.md` file under `## Flow: <anchor>`.

```yaml
- id: <stable-id>
  name: <human title>
  anchor: <heading-slug>
  persona: <persona-name>
  blocked_if_missing: [<tool-or-env-var>...]
```

| Field | Required | Notes |
|---|---|---|
| `id` | yes | Stable cross-reference; must be unique across the file. The validator and runtime key on this. |
| `name` | yes | Human-readable title. |
| `anchor` | yes | Heading slug under `## Flow:` in `qa-<app>/SKILL.md`. The installer emits the registry entry and the markdown heading from one template so they cannot drift on initial generation. |
| `persona` | yes | Must match a `personas[].name`. |
| `blocked_if_missing` | no | Tools or env vars the flow body needs. Runtime emits `BLOCKED: <name>` and skips when any are absent. |

The body in `qa-<app>/SKILL.md` for each registry entry follows this shape:

```markdown
## Flow: <anchor>

**Persona:** <persona-name>
**Goal:** <one-sentence description>

### Steps
1. <protocol-specific step>
2. ...

### Expect
- <protocol-specific expectation>
- ...

### Cleanup
1. <protocol-specific cleanup step>
```

Per-protocol step/expect/cleanup vocabularies are documented in `flow-*.md.tmpl`.

## `integrations[]`

```yaml
- name: <integration-name>
  kind: database | http-api | queue | storage | other
  coverage: mock | live | skip
  env_vars: [<NAME>...]
  notes: ""
```

`env_vars` lists names only. The installer never writes secret values.

## `policies.cleanup`

```yaml
cleanup:
  strategy: manual | auto_after_run | ephemeral_env | none
  protected_envs: [<env_name>...]
```

| `strategy` value | Behavior |
|---|---|
| `manual` *(default)* | QA does nothing; user cleans up by hand. Safest first-run mode. |
| `auto_after_run` | Each flow runs its own `### Cleanup` block after assertions. Most common automated mode. |
| `ephemeral_env` | The QA target environment is torn down/reset between runs (CI with disposable databases, preview deployments). |
| `none` | QA never creates persistent data; assertions are read-only. Right answer for production smoke tests. |

`protected_envs` is a safety belt. Even with `auto_after_run`, the runtime refuses to execute cleanup blocks against any environment listed here. Default: `[production]`. The user can extend the list.

## `policies.failure_learning`

| Value | Behavior |
|---|---|
| `suggest_in_report` *(default)* | Failure analysis is included as copy-paste-ready snippets in `qa-results/report.md` for manual review. |
| `auto_commit` | The runtime commits updates to `.maestro/qa/quirks/<app>.md` and `.maestro/qa/failure-modes/<app>.md` after each failed run. |
| `open_pr` | The runtime opens a draft PR with failure-catalog updates instead of committing directly. |

Sidecar files are keyed per app (e.g., `quirks/cli.md`, `failure-modes/web.md`), not per date — quirks and failure modes accumulate per-app knowledge across runs.

## `policies.integration_default`

Default coverage when an `integrations[]` entry does not specify its own `coverage`. Default: `mock`.

## `ci`

```yaml
ci:
  provider: none | github-actions
  agent_runtime: claude | codex
```

`provider: github-actions` triggers `.github/workflows/qa.yml` generation. `agent_runtime` selects which agent the workflow installs and invokes (`claude -p "/qa"` vs `codex run "/qa"` — see `templates/workflow-qa.yml.tmpl`). Both are filled from questionnaire category 8.

## `report.path`

Single overwritten report per run. v1 does not keep history; the user can `git stash` or commit the report between runs if they want a record.

## Validation rules

- `modalities` must be non-empty.
- For each entry in `modalities`, an `apps.<slug>` entry with `modality` matching must exist.
- `apps.<slug>.flows` must be non-empty.
- Every flow `id` must be unique across the file.
- Every flow `anchor` must match a `## Flow: <anchor>` heading in the corresponding `qa-<slug>/SKILL.md`.
- Every flow `persona` must match a `personas[].name`.
- Every `integrations[].env_vars[]` entry must be a valid env-var name (`[A-Z_][A-Z0-9_]*`).
- `policies.cleanup.protected_envs` entries must each match an `environments` key.
- If `ci.provider == github-actions`, `ci.agent_runtime` must be set.

The installer runs these checks as part of step 9 (Validate generated files).
