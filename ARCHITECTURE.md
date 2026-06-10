# ARCHITECTURE

**Status:** as-built, 2026-06-10. §2 describes the shipped card model (the unified
entity selected in `SPEC-beads-model.md`, DM1 = full rewrite, implemented on the
`card-model` branch). §3 records what the rewrite collapsed and the deviations kept.

Related: `./AGENTS.md` (working rules). The full design history (SPEC-beads-model and
its decision log) lives in untracked working notes, archived after ship.

---

## 1. Layering (unchanged by the card rewrite)

Four layers; dependencies point one way: **interfaces -> operations -> domain -> foundation**.

| Layer | Path | Owns |
|---|---|---|
| foundation/core | `src/foundation/core/` | paths, schema-version consts, atomic + content-hash-CAS writes, id-reservation markers, hashing, slugs, time, managed blocks, `MaestroError` + `.hint()` |
| domain | `src/domain/` | durable concepts: Card (core + `CardType` seam), Feature, Task, Harness, Decision, Proof, Run, Install, Skills, Extraction |
| operations | `src/operations/` | cross-domain workflows: init, sync, update, task_verify, harness apply/measure, feature_prepare, migrate |
| interfaces | `src/interfaces/` | adapters: cli, mcp, tui, hooks, shell — parse + render; domain rules stay behind owning facades |

### Deep primitives (small interface, much hidden behavior) — `foundation/core`
- `write_string_if_unchanged` — content-hash compare-and-swap + `.{name}.write-lock` marker, 15-min stale reclaim — `fs.rs:121`
- `try_reserve_marker_dir` + `DirReservation` — atomic id reservation via `.alloc-` marker dir, RAII cleanup on drop (`ALLOC_MARKER_PREFIX` `fs.rs:30`) — `fs.rs:33`
- `write_new_dir_atomic` — build in a temp root, publish by rename — `fs.rs:229`
- `append_text_file` — append-once / create-new, trailing-newline repair — `fs.rs:51`
- `child_dirs` — symlink-safe directory walk — `fs.rs:324`
- `write_string_atomic` — temp-sibling + rename + parent fsync — `safe_write.rs`

---

## 2. The card model (as-built)

One entity `card` replaces feature/task/harness/decision. Structure (DN10) = a generic
**deep card core** + a per-type **`CardType` trait** behind a `type`-dispatch **seam**
(`src/domain/card/`). The per-type lifecycle logic was *preserved* (moved behind the
trait), not rewritten.

```
generic CARD CORE  -- never changes when a type is added/altered
  schema · store(save-if-unchanged) · id-reservation · scan · archive ·
  query(ready/list) · CLI(create/show/update/dep/close)
        |  dispatch by type
   trait CardType:  legal_transitions() · is_terminal() · extra fields · gates
        ├ feature  (proposed->ready->in_progress->closed, +cancelled)   container
        ├ task/bug/chore  (draft->…->verified, +rejected/abandoned/superseded)
        ├ idea     (proposed->accepted->measured, +dismissed)
        └ decision (open->locked, +superseded)
```

- **card fields:** `id`, `type`, `status`, `parent` (feature | null), `deps[]`, `lane`,
  `claimed_by = <agent>#<session>` (agent in claude | codex | future-cli), title,
  description, acceptance, timestamps; prose (`spec.md`/`notes.md`) as **sidecar markdown**.
- **types:** `feature | task | bug | chore | idea | decision` (2 levels; `event` /
  3rd-level `epic` / `message` PARKED).
- **status:** each type stores its REAL state; a coarse `open | in_progress | closed`
  is DERIVED for the board (single source of truth, can't desync).
- **ids:** stable and opaque — hash `card-<hex>` minted via `mint_card_id` for every
  non-feature card; features keep their creation slug. The dotted `<feature>.<N>` form
  is a **display alias** rendered only by `show` (marked "display only"), never a ref
  and never parsed — addressing by position broke under reparenting, so it was demoted
  from the original dotted-id design.
- **storage (one flat store, feature is just a card):**
  ```
  .maestro/cards/<id>/card.yaml             # every card, parent as a field
  .maestro/cards/<feat>/{spec.md,notes.md,qa.md}  # prose + QA sidecars on feature cards
  .maestro/harness/harness.yml              # config only
  .maestro/archive/cards/
  ```
  Folds the old `features/` + `tasks/` + `harness/backlog.yaml` + `decisions/` trees
  into one store (`maestro migrate` remints v1 repos). Per-card DIR = contention-free.
  (NOT Dolt; file-native.)
- **management:** global QUERY, not directory navigation — `maestro ready [<feature>]`,
  `maestro list --parent --type --assignee --status`, beads-style verbs
  (`claim`/`show`/`note`/`dep add`/`archive`), emoji-free, `--json` parity.
- **edges:** `parent` · structured blockers (`dep add <child> <blocker>`) · `related` ·
  `supersedes` (non-blocking).
- **skills:** one bundled `maestro-card` skill (router `SKILL.md` +
  `reference/{work,feature,verify,qa-baseline,qa-slice}.md`) covers the active-work
  cluster; `maestro-setup` / `maestro-design` / `maestro-audit` stay separate.

Type mapping: feature -> `type:feature` (container) · task -> `type:task` ·
harness item -> `type:idea` · decision -> `type:decision` · plus new `bug`/`chore`.

---

## 3. What the rewrite collapsed (deepenings, all landed)

| # | Deepening | Collapsed |
|---|---|---|
| D1 | one save-if-unchanged seam | 3 concurrency strategies -> 1 CAS (closes the feature race) |
| D2 | one id-reservation seam | triplicated reserve loop -> `mint_card_id` |
| D3 | the `CardType` trait | 3 lifecycle styles -> 1 driver (decision/idea gain real guards) |
| D4 | one card-scan seam | strict/tolerant + symlink-safe walk copied 5x |
| D5 | one archive + one note-append | `append_note_file` verbatim 2-3x; archive task~=feature |
| D6 | cross-card rules into the card domain | guards/ref-typing leaking into cli/mcp adapters |
| D7 | one store | harness 2-store txn + hand-rolled rollback gone (ideas are cards) |

Known remainder (tracked, not yet removed): legacy entity CLI verb handlers
(`task ...`/`feature ...` proof + QA machinery), `SaveTaskHook`, and the
`TaskSnapshot::Legacy` archive-read arm coexist with the flat card verbs until the
legacy-verb deletion slice lands. The entity verbs are still the only surface for the
proof-gated lifecycle (complete/verify, accept/prepare/amend/ship).

---

## 4. Invariants any refactor MUST preserve
- content-hash CAS on every mutating write (never last-writer-wins)
- symlink rejection on every directory walk
- atomic id reservation (`.alloc-`) so concurrent sessions never reuse a number
- append-only history with trailing-newline repair
- adapters (cli/mcp/tui) carry no domain rules — rules live behind a domain facade/seam
