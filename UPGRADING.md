# Upgrading maestro: v1 → v2

> **Status: Phase 7 finalized.** Breaking-changes list confirmed at Phase 5 source-code sunset. Verb wording, edge-case notes, and the v2.0.0 CHANGELOG header are confirmed against the actual v2 CLI shipped at the v2.0.0 tag. v0.LAST tag on `main` carries the last v1 commit (was v0.83.0) for downstream consumers who need to pin away from v2.

The current harness is a big-bang release. There is no parallel binary, no aliasing of legacy verbs, and no compatibility shim. Existing pre-rebuild projects upgrade in one step via `maestro setup migrate-v2`.

If you are not ready to upgrade, pin to the `v0.LAST` tag on `main`. (Tag is named for the actual version lineage — maestro has only ever shipped `0.x` releases; v2 is the first major version.)

---

## TL;DR — one command

```bash
# In a v1 maestro project:
maestro setup migrate-v2
```

This writes a backup tarball to `.maestro/backups/pre-v2-<timestamp>.tar.gz`, rewrites your `.maestro/` directory to v2 shape, and stamps `.maestro/v2-migrated.flag` so subsequent runs short-circuit. Idempotent: re-running takes another backup and exits.

To restore: `tar -xzf .maestro/backups/pre-v2-<timestamp>.tar.gz -C .` (v2 does not ship an automated restore verb).

---

## What changes

### Verbs

**Removed (no replacement):** `ralph`, `ralph-review`, `note`, `notes` (standalone create/list verb), `inspect`, `state`, the seven `memory-*` verbs, the two `memory-ratchet` verbs, `generateAgentPrompt`, the two `graph` verbs, `session detect`.

**Renamed / replaced:**

| v1 verb | v2 equivalent |
|---|---|
| `mission *` | `plan *` (`plan from-spec`, `plan decompose`, `plan show`) |
| `intake`, `brainstorm` | `spec new` (with the grill protocol) |
| `task complete` | `task ship` (alias: `ship`) |
| `task unblock` | Removed — block resolves via `verify` PASS or evidence transition |
| `maestro-classify` skill | Absorbed into `maestro-design` (work-type classification at spec authoring) |
| `maestro-qa` skill | Absorbed into `maestro-setup --qa` |

**Added (new in v2):** `spec new`, `spec validate`, `task from-spec`, `plan from-spec`, `plan decompose`, `principle promote`, `setup check`, `setup bootstrap`, `setup migrate-v2`, `setup migrate-corrections`. Hot-path aliases: `claim`, `verify`, `ship`, `block`, `abandon`.

### Vocabulary

| v1 term | v2 term |
|---|---|
| mission | exec-plan |
| spec | product-spec |
| intake / brainstorm | folded into design-docs reading + product-spec authoring |
| session, notes | handoff |
| memory (runtime store) | `docs/principles/*.md` (corrections) + `docs/design-docs/learnings/` (learnings) |
| graph (runtime store) | `docs/references/project-graph.yaml` |

### File layout

| v1 path | v2 path |
|---|---|
| `.maestro/missions/<id>/` | `.maestro/plans/<id>/` |
| `.maestro/memory/corrections/*.json` | `docs/principles/legacy/*.md` |
| `.maestro/memory/ratchet/` | Deleted |
| `.maestro/graph.json` | `docs/references/project-graph.yaml` |
| `.maestro/session/` | Folded into `.maestro/runs/<id>/agent.json` for in-flight worktrees |
| `tasks.jsonl` | `tasks.jsonl` (v1 file preserved unchanged during migration) |
| `.maestro/MAESTRO.md` | Deleted — was a v1 operational compass referencing v1-only verbs (`intake`, `task plan`, `memory-correct`). v2 projects use `AGENTS.md` + `context/` for operator guidance. |
| `skills/built-in/maestro:*` | Deleted (colon-namespaced tier removed) |

### State machines

Task states migrate per the table in master-plan §9. Mission states migrate to exec-plan states per the same section. The migration reads raw legacy state from `tasks.jsonl` and bypasses normalization so legacy `deferred` and `closed` map correctly.

### MCP server

If your agent uses `mcp__maestro__*` tools, the surface area is unchanged but tools are renamed:

| v1 MCP tool | v2 MCP tool |
|---|---|
| `task_complete` | `task_ship` |
| `task_unblock` | Removed |
| `task_create`, `task_plan` | `task_from_spec` |

New v2 MCP tools: `principle_promote`, `setup_check`, `setup_migrate_v2`. Grill-driven verbs (`spec new`, `plan from-spec`, `plan decompose`) are CLI-only — MCP cannot sustain the interactive grill protocol.

Kept unchanged: `task_claim`, `task_block`, `task_get`, `task_list`, all `evidence_*`, `verdict_*`, `policy_*`, `handoff_*`, `contract_*` tools.

**Semantic break — `task_block`:** the tool signature is unchanged but the semantics changed. In v1, `task block` was a bidirectional graph edge (calling it on task A added A to B's `blockedBy` list and B to A's `blocks` list symmetrically). In v2, `task block` is a one-directional state-transition verb with a required reason: it moves the task to the `blocked` state and records a `kind=transition` evidence row. The unblock path is now via `task verify` PASS (the harness auto-transitions `blocked → verifying` on PASS). If your agent called `task_block` to set up blocker-graph edges, update it to use the explicit `blockedBy` field on task creation via `task_from_spec` instead.

**No MCP `task_verify` tool:** `maestro task verify` is CLI-only. The interactive grill-and-exit-code routing cannot be faithfully proxied over MCP. Agents that need to verify must call the CLI directly (e.g., via a shell tool) and inspect the exit code.

### Skill bundle

The 10-skill v1 bundle collapses to 6: `maestro-setup`, `maestro-design`, `maestro-plan`, `maestro-task`, `maestro-verify`, `maestro-handoff`.

---

## What does not change

Per master-plan §11, these surfaces are kept as-is and not rewritten in v2.0:

- Mission Control TUI (with mission → exec-plan rename in the snapshot read model)
- CI integration (`maestro ci verify` + PR check + auto-merge eligibility)
- Hooks (`hooks/`)
- GitNexus integration
- `gc`, `recover`, `bundle`
- Verdict types, witness levels L0–L7, policy YAML, risk-class derivation
- Deploy gate (L7), runtime monitor, rollback witness
- Cross-task conflict detection

If your usage is confined to these surfaces, the upgrade is a no-op apart from CLI verb renames you may not even hit.

---

## If something breaks

1. **Migration failed mid-run.** Restore from `.maestro/backups/pre-v2-<timestamp>.tar.gz` (manual `tar -xzf`). File an issue with the migration command output.
2. **A v1 verb you depended on is gone.** Check the renamed/replaced table above. If your verb is not listed and not in the removed list, that is a bug — file an issue.
3. **Tests pass locally but CI fails after migration.** Verify your CI runner installs v2 (`v0.100.0` or later) and that `maestro ci verify` is bound to the v2 verb surface.
4. **You need to roll back the whole release.** Pin to `v0.LAST` until v2 lands the fix.

---

## Reference

- Decision register: `docs/adr/`
- CLI reference: `docs/cli-reference.md`
