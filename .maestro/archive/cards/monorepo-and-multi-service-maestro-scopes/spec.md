# monorepo and multi-service maestro scopes

## Current state

Discovery mechanism: `discover_repo_root_from_with_home` (src/foundation/core/paths.rs:153-175) walks UP from cwd and returns the NEAREST ancestor holding a `.maestro` OR `.git`, with a home-escape guard (paths.rs:177-179). `init_repo_root()` (src/operations/init/mod.rs:120) uses that same discovery, so `maestro init` from a subdir resolves to the ENCLOSING repo root, not the cwd. `init` flags are only [--dry-run --merge --force --yes] (reference/cli.md) -- there is NO force-local/--here flag.

Probe (real binary v0.107.0, scratch repos):
F1 nearest-wins: from fe/src/components, `feature list` returns only the fe-scope card.
F2 true single-git monorepo CANNOT nest: `maestro init` run inside fe/ and be/ (no nested .git) walked up and re-inited the ROOT; no fe/.maestro or be/.maestro was created. A nested .maestro only appears when the subdir has its own .git (submodule / multi-repo workspace).
F3 total isolation when nested: with submodule-style fe/.git+be/.git, root `feature list` and `active` show ONLY root items; fe/be cards and sessions are invisible. No cross-scope aggregation, links, messages, or decisions.

Area-tag gap: `--area` is settable on `feature set` (reference/cli.md:111) but NO read verb filters by it -- `list`/`ready`/`card list` filter by [--parent --type --assignee --status --grep --archived --all] only. So 'one root scope, tag sub-projects by area' is incomplete today: you can label an area but cannot filter the backlog to it (only --grep as a workaround).

Setup/context-ingestion gap (2026-06-16, enhancement ask): the maestro-setup skill v1.4.2 is the repo 'adjust/tune' skill, but step 7 only says 'inspect... existing agent instructions' and step 8 'update harness guidance from verified files' -- a vague manual tune, not a systematic enumeration+read of all docs + agent specs. HarnessConfig::detect (src/domain/harness/schema.rs:168) reads ONLY build manifests (Cargo.toml/package.json/pyproject) for stack+verify; it ingests no docs. And there is no per-project handling: in a monorepo each project's own AGENTS.md/CLAUDE.md/README/docs/ never separately inform that project's context. Ask: make the setup skill read all docs + agent specs (any doc) and fill them into maestro context, per declared project in a multi-project repo.

## Problem

## Research (how the ecosystem solves this)

Two layouts, two DIFFERENT conventions:

MONOREPO (one git repo, N sub-projects): tooling uses ONE workspace root + a project graph; sub-projects are first-class members (Nx project.json, Turborepo workspaces). Per-package config is an OVERRIDE on a shared root baseline (Oxlint/ESLint-flat/Biome 'nearest config wins, anchored at root'), never N independent silos. GitHub guidance: maintain a single source of truth.

POLYREPO / MICROSERVICES (N git repos): each repo is independent; coordination lives in a META layer ABOVE the repos -- Google 'repo'+manifest, GitKraken Workspaces, mani, meta-repo pattern -- which pins/aggregates members and lives OUTSIDE any one repo. Cross-repo work tracking = ONE org-level GitHub Project spanning repos; per-repo labels do NOT cross repos.

Crux for maestro: .maestro state is committed INSIDE a repo. So monorepo => exactly ONE natural .maestro at root (matches current walk-up discovery); polyrepo => N isolated .maestro (matches git/submodule semantics). Aggregation/federation for polyrepo is a separate above-the-repos concern that existing tools already own.

## Recommendation

Lean, in-model split (grounded in research + repo philosophy: YAGNI / no speculative abstraction / state-lives-in-repo):
(1) MONOREPO = the supported, recommended model: one root .maestro, sub-projects become a FIRST-CLASS, FILTERABLE 'project/area' dimension on cards. This is the only real in-model gap today (--area is settable but not filterable). Lean; fits the card model; directly gives 'root sees everything, organized by service'.
(2) POLYREPO = keep scopes ISOLATED (current behavior is correct and matches git). Do NOT build a maestro workspace/federation layer -- it reinvents repo/mani/org-Projects. Optionally add cheap 'which scope am I in?' clarity (the no-confusion goal).
Rejected: a root-fans-out-to-children aggregation/link framework -- speculative, duplicates dedicated tooling, against repo memory.
