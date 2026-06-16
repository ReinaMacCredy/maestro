---
amend_log_position: 0
---

### QA Baseline Contract

- Scope: monorepo-and-multi-service-maestro-scopes -- adds a `project` dimension to the Card base, declaration-gated auto-infer, a `--project` read surface, and a per-project doc/agent-spec read-in step in the maestro-setup skill. Surfaces: card schema/store, harness schema/templates, paths resolution, list/ready CLI + JSON, init/sync, embedded maestro-setup skill.
- Critical workflow chains:
  - Multi-project work loop
    - Steps: declare `projects:` in harness -> `cd <project>` -> create card (project auto-inferred + stored) -> `maestro list`/`ready` (badge + grouping) -> `maestro list --project <p>` (filter) -> `--json` (flat field)
    - Touched link: card create (new stored field) and list/ready render (badge/grouping/filter)
    - Minimal proof: scripted real-binary run in a scratch monorepo; compare observed rows + card.yaml + JSON
  - Compatibility chain (the protective bar)
    - Steps: repo with NO `projects:` key -> create cards from subfolders -> `maestro list`/`ready` (human + json)
    - Touched link: all read renders must stay byte-identical to the pre-feature binary
    - Minimal proof: diff new-binary output vs pre-feature binary output on an identical no-projects repo; expect empty diff

- Scenario Matrix:
  - [bl-001] Card persists `project` through create and read (covers: ac-1)
    - Dimensions: data shape, persistence, actor=any card type
    - Setup: `maestro init` a scratch repo
    - Action: `maestro task new "x" --project svc-pay`; inspect `.maestro/cards/<id>/card.yaml` and `maestro show <id>`; also create one card with no `--project`
    - Oracle: card.yaml contains `project: svc-pay`; the no-flag card has no `project` key; same holds for feature/bug/chore/idea/decision card types
    - Evidence to capture: card.yaml contents, `maestro show` output
    - Reproduction: create-with-flag and create-without-flag in a fresh repo
  - [bl-002] Pre-existing card with no `project` field still loads (covers: ac-1)
    - Dimensions: compatibility, failure-recovery
    - Setup: a `card.yaml` authored before this feature (no `project` key)
    - Action: `maestro show <id>`, `maestro list`
    - Oracle: loads without error; project treated as absent; round-trips unchanged on rewrite
    - Evidence to capture: command exit 0 + output; git diff of card.yaml after a touch verb shows no spurious `project:` line
    - Reproduction: point the binary at a legacy card fixture
  - [bl-003] Declaration-gated auto-infer stores the matched segment (covers: ac-2)
    - Dimensions: state/lifecycle, data shape, environment=cwd
    - Setup: harness `projects:` set to each form: `["*"]`, then `["services/*"]`, then `["fe","be"]`
    - Action: create a card from `<root>/svc-pay/...` (form 1), from `services/pay/...` (form 2), from `docs/...` (form 3)
    - Oracle: stored `project` = `svc-pay` (form 1), `pay` (form 2), and NONE (form 3, docs unmatched by explicit set)
    - Evidence to capture: card.yaml `project` value per run
    - Reproduction: scripted cd + create per form
  - [bl-004] No declaration => no stored project and byte-identical human output (covers: ac-3, ac-15)
    - Dimensions: compatibility (the protective bar), non-functional
    - Setup: harness with NO `projects:` key; cards created from several subfolders
    - Action: `maestro list`, `maestro ready` (human); also `maestro task new x` from a subfolder
    - Oracle: no `project` stored; human output byte-identical to the pre-feature binary (no `[project]` badge, no grouping); explicit `--project` still sets/stores
    - Evidence to capture: `diff` of new vs pre-feature binary output (expect empty)
    - Reproduction: build pre-feature binary at the branch point, run both on the same repo
  - [bl-005] `--project` filter on list/ready, unknown is empty not error (covers: ac-4)
    - Dimensions: entrypoint, failure-recovery
    - Setup: cards across >=2 projects
    - Action: `maestro list --project svc-pay`, `maestro ready --project svc-pay`, card-namespaced equivalents; then `--project does-not-exist`; confirm `maestro status` exposes NO `--project` flag
    - Oracle: only cards with stored project == svc-pay returned; unknown project => empty result, exit 0; status has no `--project`
    - Evidence to capture: filtered row sets; exit code on unknown; `maestro status --help`/cli.md absence of the flag
    - Reproduction: scripted multi-project repo
  - [bl-006] `[project]` badge + group-by-project with a >=2 threshold (covers: ac-5, ac-6)
    - Dimensions: output shape, non-functional
    - Setup: (a) cards across >=2 distinct projects; (b) a repo with 0 or 1 distinct project
    - Action: `maestro list` with no `--project`
    - Oracle: rows with a project show a `[project]` badge, rows without show none; grouping under project headers appears ONLY when >=2 distinct projects among shown cards (no-project cards under a root/unassigned group); with 0 or 1 distinct project the list is flat and byte-identical to today
    - Evidence to capture: grouped output (>=2) and flat output (<=1)
    - Reproduction: two scratch repos, one multi-project one single
  - [bl-007] `--json` stays flat with `project` as one field (covers: ac-7)
    - Dimensions: integration boundary, data shape
    - Setup: cards across >=2 projects
    - Action: `maestro list --json`, `maestro ready --json`, `maestro status --json`
    - Oracle: existing dense single-line-per-item envelope, `project` added as one flat field; never grouped or nested regardless of distinct-project count
    - Evidence to capture: raw JSON lines; confirm one object per line, no nesting key added
    - Reproduction: scripted multi-project repo with `--json`
  - [bl-008] `projects:` config survives init/sync/harness re-detect; absent key still loads (covers: ac-8, ac-9)
    - Dimensions: persistence, install ownership, compatibility
    - Setup: harness.yml carrying a user-authored `projects:` declaration; separately, a harness.yml with NO `projects:` key
    - Action: `maestro init` (re-init/merge), `maestro init --force`, `maestro sync`, and any harness re-detect path
    - Oracle: `projects:` preserved verbatim (not stripped, reordered, or overwritten); the no-key config loads unchanged
    - Evidence to capture: harness.yml diff before/after each lifecycle command (expect `projects:` intact)
    - Reproduction: author projects:, run each lifecycle verb, diff
  - [bl-009] set_contract / feature readiness gate unchanged by `project` (covers: ac-10)
    - Dimensions: safety-critical invariant
    - Setup: a feature card with `project` set but empty acceptance AND empty affected_areas
    - Action: `maestro feature accept --dry-run` / set_contract readiness check
    - Oracle: readiness still requires acceptance OR affected_areas; `project` neither satisfies nor alters the gate
    - Evidence to capture: gate failure message with project-only feature; gate pass once acceptance/area present
    - Reproduction: scripted feature with project-only contract
  - [bl-010] One root scope, no federation (covers: ac-11)
    - Dimensions: integration boundary, install ownership
    - Setup: a single git repo (monorepo) with subfolders; separately two distinct git repos
    - Action: create a card from a subfolder; inspect where it lands; scan CLI surface for an aggregation verb
    - Oracle: discovery returns exactly one root `.maestro` (nearest-wins walk unchanged); card lands in the single root `.maestro/cards/`; no cross-repo aggregation verb exists; separate git repos stay isolated
    - Evidence to capture: card path; `announce_repo_root` line; cli.md verb list shows no new aggregation verb
    - Reproduction: scratch monorepo + scratch sibling repos
  - [bl-011] maestro-setup read-in per project, bounded, no write-out (covers: ac-12, ac-13, ac-14)
    - Dimensions: workflow, install ownership, trust boundary
    - Setup: a monorepo with `projects:` declared and per-project docs (AGENTS.md/CLAUDE.md/README.md/docs/*.md)
    - Action: run the maestro-setup skill flow (enumerate + synthesize); then inspect each sub-project's AGENTS.md/CLAUDE.md and the root harness guidance
    - Oracle: a BOUNDED doc set is enumerated at root AND under each declared project; guidance is synthesized into the SINGLE root harness guidance with one section per project; NO maestro-managed guidance is written into any sub-project AGENTS.md/CLAUDE.md (install/sync still write managed blocks only at root); no new CLI verb and no new harness schema field were added
    - Evidence to capture: root harness guidance with per-project sections; sub-project specs unmodified by maestro; `git diff` shows only root-managed blocks; cli.md/schema unchanged for ingestion
    - Reproduction: scratch monorepo with per-project docs, run setup

- Preserved behaviors:
  - Nearest-wins discovery walk -> Proof: `bl-010` card-path + announce line
  - Dense multi-item JSON envelope (one object per line) -> Proof: `bl-007`
  - Single-repo list/ready human output -> Proof: `bl-004`, `bl-006` (<=1 project), `bl-015` rolled into bl-004
  - Install/sync write managed blocks only at repo root -> Proof: `bl-008`, `bl-011`
  - Feature readiness gate (acceptance OR affected_areas) -> Proof: `bl-009`

- Changed behaviors:
  - Card base gains optional `project` field (additive)
  - HarnessConfig gains optional `projects:` declaration (additive)
  - list/ready gain `--project` filter, `[project]` badge, and group-by-project (>=2) -- all gated so undeclared repos are unchanged
  - maestro-setup skill gains a per-project bounded doc/agent-spec read-in step

- Critical probes before commit:
  - Forward-compat round-trip (no spurious `project:`/`projects:` on legacy fixtures) -> `cargo test` round-trip cases for Card + HarnessConfig
  - Byte-identical no-projects output -> diff vs pre-feature binary on an identical repo
  - Install ownership unchanged -> `rg -n 'AGENTS\.md|CLAUDE\.md' src/domain/install src/operations/sync` still root-only

- Required artifacts:
  - None beyond the scratch repos used for evidence

- Baseline gaps:
  - maestro-setup is a markdown skill, so bl-011 is exercised by an agent flow rather than a single command -> Proposed probe: assert the skill content names the bounded set + per-project enumeration + read-in-only, and assert install/sync code paths remain root-only (static guard), plus one real scratch-repo setup run

```yaml
slices:
  - at: "2026-06-16T16:14:36Z"
    scenarios: ["bl-001", "bl-002"]
    probes: ["real binary in scratch repos: create -t {feature,task,bug,chore,idea,decision} --project svc-pay; create without --project"]
    result: pass
    evidence:
      - "--project svc-pay persists 'project: svc-pay' for all 6 card types (feature card.yaml; task/bug/chore tasks/<id>/task.yaml; idea/decision aggregate ideas.yaml/decisions.yaml)"
      - "no --project => no project key for feature/task; np-feature show exit 0 (legacy load OK); no spurious project after touch verb"
  - at: "2026-06-16T16:14:36Z"
    scenarios: ["bl-003"]
    probes: ["real binary: projects:[*]|[services/*]|[fe,be] auto-infer from svc-pay/, services/pay/, docs/, fe/, repo root; --project override"]
    result: pass
    evidence:
      - "[*] under svc-pay/ => svc-pay; [services/*] under services/pay/ => pay; [fe,be] under docs/ => none, under fe/ => fe; --project => override-wins; [*] from repo root => none"
  - at: "2026-06-16T16:14:36Z"
    scenarios: ["bl-004"]
    probes: ["diff NEW binary vs pre-feature gdc1c65f4 human output (list/ready/cardlist); explicit --project with no declaration"]
    result: pass
    evidence:
      - "list/ready/cardlist human output byte-identical (empty diff) to pre-feature binary; no badge/grouping on project-less rows"
      - "create --project manual-set with no projects: declared still stores project: manual-set"
  - at: "2026-06-16T16:14:36Z"
    scenarios: ["bl-005"]
    probes: ["real binary: list/ready --project, card-namespaced card list/ready --project, unknown project name, status flag check"]
    result: pass
    evidence:
      - "list/ready --project svc-pay and card list/ready --project return only matching cards with [svc-pay] badge; unknown 'does-not-exist' => empty, exit 0; status has no --project flag"
  - at: "2026-06-16T16:14:36Z"
    scenarios: ["bl-006"]
    probes: ["real binary: unfiltered list with 2 distinct projects vs 1 distinct project"]
    result: pass
    evidence:
      - ">=2 distinct (svc-auth, svc-pay) groups under headers svc-auth/svc-pay/unassigned; 1 distinct is flat (no headers); ac-5 badge shows unconditionally"
  - at: "2026-06-16T16:14:36Z"
    scenarios: ["bl-007"]
    probes: ["real binary: list/ready/status --json"]
    result: pass
    evidence:
      - "list/ready/status --json emit dense single-line envelope with project as one flat field (string or null); never grouped/nested; root-c shows project:null"
  - at: "2026-06-16T16:14:36Z"
    scenarios: ["bl-008"]
    probes: ["real binary: authored projects:[be,fe,services/*] across init --yes, init --force --yes, sync"]
    result: pass
    evidence:
      - "projects: declaration preserved verbatim across init, init --force, sync (no strip/reorder/overwrite)"
  - at: "2026-06-16T16:14:36Z"
    scenarios: ["bl-009"]
    probes: ["real binary: feature with project:svc-pay but empty acceptance+areas at accept gate"]
    result: pass
    evidence:
      - "project-only feature BLOCKED at accept (incomplete contract); adding acceptance clears that gap with project untouched; project never satisfies/alters the gate"
  - at: "2026-06-16T16:14:36Z"
    scenarios: ["bl-010"]
    probes: ["real binary: card from services/deep/nested/; .maestro dir scan; CLI verb scan; two sibling git repos"]
    result: pass
    evidence:
      - "deep-subfolder card lands in single root .maestro/cards/tasks/; one .maestro dir; no aggregation/federation verb; sibling repos each list only own card"
  - at: "2026-06-16T16:14:36Z"
    scenarios: ["bl-011"]
    probes: ["real binary: maestro install --agent codex in monorepo; T6 commit bbabfc2f scope; tests/setup_skill_context_readin.rs"]
    result: pass
    evidence:
      - "install writes maestro:start/end block ONLY at root AGENTS.md/CLAUDE.md; fe/be specs unchanged (no managed block); install_mirrors 12/12"
      - "T6 commit changed only SKILL.md + resources_version_guard + setup_skill_context_readin test; no CLI verb, no harness schema field; 3 content-guard tests pass"
```
