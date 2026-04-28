# Questionnaire

Run categories sequentially via AskUserQuestion. After each category, persist the answers to `.maestro/qa/.install-progress.yaml` under `progress.<category>` with `status: done` and the recorded values. A later resume reads this file and skips done categories.

This is Factory's 8-category questionnaire, adapted to maestro paths and conventions. Modality and `test_tool` selection are NOT questionnaire categories — modalities are auto-detected (and confirmed in install-flow step 3), and `test_tool` is auto-derived from modality (defaults documented in `detection.md`; user edits `config.yaml` post-install if a non-default protocol is needed).

Skip rules:

- Skip a category whose precondition is not met (each category lists its own).
- Never re-ask a category whose `progress.<category>.status` is `done` unless the user explicitly chose "fresh start" in step 1.

---

## 1. Critical flow confirmation

**Precondition:** detection completed (install-flow step 2).

The agent has already enumerated candidate flows from the codebase per modality (route files, CLI command registries, screen names, etc. — see `detection.md`'s "Critical-flow candidate detection" section). Present the candidate list and ask the user to confirm, edit, or replace.

**Prompt:** "I detected these candidate critical flows from your code: `<candidate list grouped by modality>`. Confirm to use as-is, or edit/add/remove."

**Default:** "Use exactly these flows" (one-click acceptance, matches Factory's demo UX).

**Persist:** `progress.critical_flows.values: [{modality, id, name, description}...]`.

---

## 2. Default QA target

**Precondition:** none.

**Prompt:** "Where should `/qa` run flows by default? Provide one or more environments. Each environment needs a name (e.g., `development`, `staging`, `production`), a URL or local target, and any restrictions (e.g., `read-only only, never create data`). Pick one as the default."

For each environment the user names, ask:
- `url`: URL, local-host port, or local command
- `restrictions`: free-text list (optional)

For the set of named environments, ask:
- `default_target`: which one is the default

**Default:** detected from common signals (`localhost:3000`, `http://localhost:5173`, `127.0.0.1:5000`, env files); falls back to a single `development` entry with empty url + a TODO marker the user fills post-install.

**Persist:** `progress.default_qa_target.environments: [{name, url, restrictions}...]`, `progress.default_qa_target.default: <name>`.

---

## 3. Personas and roles

**Precondition:** none.

**Prompt:** "QA flows are run under named personas. Define one or more — a persona has a `name`, a `test_focus` list (areas it cares about), and optional `cannot_do` list (negative-path actions it should not be authorized for)."

If the repo has detected auth roles (e.g., `admin`, `viewer`, `member` from a permissions config), pre-populate as suggestions.

**Default:** a single persona named `default` with empty `test_focus` and no `cannot_do` — the user fills it post-install if they want richer personas.

**Persist:** `progress.personas.values: [{name, test_focus, cannot_do}...]`.

---

## 4. Cleanup strategy

**Precondition:** none.

**Prompt:** "How should `/qa` clean up after a run?"

Options:

- `manual` — QA does nothing; user cleans up by hand. Safest for first runs and dev-machine QA. **(default)**
- `auto_after_run` — Each flow runs its own `### Cleanup` block after assertions. Right answer for repos with idempotent teardown.
- `ephemeral_env` — Env is torn down/reset between runs (CI with disposable databases, preview deployments). Different mechanism: env teardown, not per-flow blocks.
- `none` — QA never creates persistent data; assertions are read-only. Right answer for production smoke tests.

Follow-up: "Which environments must NEVER be auto-cleaned regardless of the strategy above?"

**Default:** `[production]` (always, even when the user picked `manual` or `none`). User can extend.

**Persist:** `progress.cleanup.strategy: <option>`, `progress.cleanup.protected_envs: [<env_name>...]`.

---

## 5. Failure-learning strategy

**Precondition:** none.

**Prompt:** "When a flow fails, how should the failure-catalog be updated?"

Options:

- `suggest_in_report` — Include copy-paste-ready snippets in `qa-results/report.md` for manual review. **(default)**
- `auto_commit` — Automatically commit updates to `.maestro/qa/quirks/<app>.md` and `.maestro/qa/failure-modes/<app>.md` after each failed run.
- `open_pr` — Open a draft PR with the failure-catalog updates instead of committing directly.

Sidecar files are keyed per app (`quirks/cli.md`, `failure-modes/web.md`), not per date — knowledge accumulates per-app across runs.

**Persist:** `progress.failure_learning.value: <option>`.

---

## 6. External sandbox/test credentials

**Precondition:** at least one external integration was detected in install-flow step 2.

**Prompt:** "I detected these external integrations: `<list with kinds>`. For each, choose coverage:"

Options per integration:
- `mock` — Use stubbed responses; no real credentials needed. **(default)**
- `live` — Hit the real service; requires sandbox/test credentials referenced by env-var name.
- `skip` — Don't test this integration.

For any integration set to `live`, follow up: "Which env var(s) hold the test credentials? I will reference them by name only — values are never stored."

**Persist:** `progress.external_creds.values: [{name, kind, coverage, env_vars}...]`.

---

## 7. ImageMagick install

**Precondition:** at least one visual modality (`web`, `desktop`, or any modality whose `test_tool` is `agent-browser`/`agent-device`/`electron`) was detected.

**Prompt:** "Visual diff GIFs in QA reports require ImageMagick. Install via Homebrew now? (Skip if unsure — flows still run without GIF diffs.)"

Options: `yes`, `skip`.

**Default:** `skip`.

If `yes`, the installer runs `brew install imagemagick` via Bash on macOS. On other platforms it prints the platform-specific install hint and proceeds.

**Persist:** `progress.imagemagick.value: <option>`.

---

## 8. GitHub Actions generation + agent runtime

**Precondition:** `.github/` directory exists.

**Prompt 1:** "Generate `.github/workflows/qa.yml` to run `/qa` on push to main and on PR? An existing `qa.yml` will be backed up to `qa.yml.bak.<timestamp>`."

Options: `yes`, `skip`.

**Default:** `yes` if no existing `qa.yml`, else `skip` (so the user explicitly opts in to overwrite).

If the user picked `yes`, ask follow-up:

**Prompt 2:** "Which agent runtime should the workflow use to run `/qa` headlessly?"

Options: `claude` (uses `claude -p "/qa" --dangerously-skip-permissions` with `ANTHROPIC_API_KEY`), `codex` (uses Codex CLI with the OpenAI key).

**Default:** `claude`.

The workflow references the secret name only (`${{ secrets.ANTHROPIC_API_KEY }}` or the Codex equivalent); the value is never stored.

**Persist:** `progress.ci.provider: github-actions | none`, `progress.ci.agent_runtime: claude | codex`.

---

## Save points

Write `.maestro/qa/.install-progress.yaml` after every category, even if the user picks the default. The file's purpose is resume; partial progress must always be recoverable.
