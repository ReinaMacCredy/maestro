# ARCHITECTURE

**Status:** as-built, 2026-06-10. §2 describes the shipped card model (the unified
entity selected in `SPEC-beads-model.md`, DM1 = full rewrite, implemented on the
`card-model` branch). §3 records what the rewrite collapsed and the deviations kept.

Related: `./AGENTS.md` (working rules). The full design history (SPEC-beads-model and
its decision log) lives in untracked working notes, archived after close.

---

## 1. Layering (unchanged by the card rewrite)

Four layers; dependencies point one way: **interfaces -> operations -> domain -> foundation**.

| Layer | Path | Owns |
|---|---|---|
| foundation/core | `src/foundation/core/` | paths, schema-version consts, atomic + content-hash-CAS writes, id-reservation markers, hashing, slugs, time, managed blocks, `MaestroError` + `.hint()` |
| domain | `src/domain/` | durable concepts: Card (core + `CardType` enum dispatch), Feature, Task, Harness, Decision, Proof, Run, Install, Skills, Extraction |
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
**deep card core** + a closed `pub enum CardType` (`feature | task | bug | chore |
idea | decision`). There is no `CardType` trait in the shipped model. The type seam
is explicit exhaustive `match` dispatch at the card-store, query, edit, and CLI
boundaries; per-type lifecycle rules stay in the owning domain modules.

```text
generic CARD CORE  -- never changes when a type is added/altered
  schema · store(save-if-unchanged) · id-reservation · scan · archive ·
  query(ready/list) · CLI(create/show/update/dep/close)
        |  dispatch by exhaustive match on CardType
   enum CardType: feature · task · bug · chore · idea · decision
        ├ card store: placement · id prefix · save basis · reconcile
        ├ card query/edit/CLI: ready/list filters · type hints · close guards
        └ domain lifecycles:
             feature  (proposed->ready->in_progress->closed, +cancelled) container
             task/bug/chore  (draft->…->verified, +rejected/abandoned/superseded)
             idea     (proposed->accepted->measured, +dismissed)
             decision (open->locked, +superseded)
```

Trait revisit trigger: introduce a per-type behavior trait only if a seventh card
type lands, or if WS5 schema-compatibility work makes match-based per-type behavior
multiply enough that a trait removes real duplication. Until then, exhaustive matches
are the intended safety mechanism: adding a `CardType` variant forces every dispatch
site to be reviewed by the compiler.

- **card fields:** `id`, `type`, `status`, `parent` (feature | null), `deps[]`, `lane`,
  `claimed_by = <agent>#<session>` (agent in claude | codex | future-cli), title,
  description, acceptance, timestamps; prose (`spec.md`/`notes.md`) as **sidecar markdown**.
- **types:** `feature | task | bug | chore | idea | decision` (2 levels; `event` /
  3rd-level `epic` / `message` PARKED).
- **status:** each type stores its REAL state; a coarse `open | in_progress | closed`
  is DERIVED for the board (single source of truth, can't desync).
- **ids:** stable and opaque after creation. Features use their creation slug; other
  cards mint readable typed slug ids (`task-<slug>-<hex4>`, `bug-...`, `chore-...`,
  `dec-...`, `idea-...`). Legacy `card-<hex>` ids stay valid but are not minted for
  new non-feature cards. The dotted `<feature>.<N>` form is a **display alias**
  rendered only by `show` (marked "display only"), never a ref and never parsed —
  addressing by position broke under reparenting, so it was demoted from the original
  dotted-id design.
- **storage (one card store, feature is just a card):**
  ```text
  .maestro/cards/<feature>/card.yaml                    # feature container card
  .maestro/cards/<feature>/{spec.md,notes.md,qa.md}     # feature prose + QA
  .maestro/cards/<feature>/tasks/<task>/task.yaml       # task/bug/chore cards
  .maestro/cards/<feature>/decisions.yaml               # decision entries
  .maestro/cards/tasks/<task>/task.yaml                 # parentless task/bug/chore cards
  .maestro/cards/decisions.yaml                         # parentless decision entries
  .maestro/cards/ideas.yaml                             # harness idea entries
  .maestro/harness/harness.yml                          # config only
  .maestro/archive/cards/
  ```
  Folds the old `features/` + `tasks/` + `harness/backlog.yaml` + `decisions/` trees
  into one store (`maestro migrate` remints v1 repos). Task-family records keep
  per-card dirs for contention-free work; decisions and ideas are entry-backed where
  their owning domain still treats them as rosters. (NOT Dolt; file-native.)
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
| D3 | closed `CardType` enum + exhaustive dispatch | 3 lifecycle styles -> 1 reviewed type seam (decision/idea gain real guards) |
| D4 | one card-scan seam | strict/tolerant + symlink-safe walk copied 5x |
| D5 | one archive + one note-append | `append_note_file` verbatim 2-3x; archive task~=feature |
| D6 | cross-card rules into the card domain | guards/ref-typing leaking into cli/mcp adapters |
| D7 | one store | harness 2-store txn + hand-rolled rollback gone (ideas are cards) |

The entity verbs (`task ...`/`feature ...` proof + QA machinery) coexist with the flat
card verbs by design: they are the only surface for the proof-gated lifecycle
(complete/verify, accept/prepare/amend/close), and every handler operates on the card
store. The legacy persistence plumbing behind them (`SaveTaskHook`, the
`TaskSnapshot::Legacy` arm, the `task.yaml` tree readers) is gone.

---

## 4. Invariants any refactor MUST preserve
- content-hash CAS on every mutating write (never last-writer-wins)
- symlink rejection on every directory walk
- atomic id reservation (`.alloc-`) so concurrent sessions never reuse a number
- append-only history with trailing-newline repair
- adapters (cli/mcp/tui) carry no domain rules — rules live behind a domain facade/seam
