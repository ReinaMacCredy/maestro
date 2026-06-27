# ARCHITECTURE

**Status:** as-built, 2026-06-28. §2 describes the shipped card/task model (the
unified card store from `SPEC-beads-model.md`, updated with the D17 Progress card
model, the Card/CardKind/Task taxonomy, and native Memory cards). §3 records what the rewrite
collapsed and the deviations kept.

Related: `./AGENTS.md` (working rules). The full design history (SPEC-beads-model and
its decision log) lives in untracked working notes, archived after close.

---

## 1. Layering (unchanged by the card rewrite)

Four layers; dependencies point one way: **interfaces -> operations -> domain -> foundation**.

| Layer | Path | Owns |
|---|---|---|
| foundation/core | `src/foundation/core/` | paths, schema-version consts, atomic + content-hash-CAS writes, id-reservation markers, hashing, slugs, time, managed blocks, `MaestroError` + `.hint()` |
| domain | `src/domain/` | durable concepts: Card (core + `CardType` enum dispatch), Feature, Task, Harness, Decision, Proof, Run, Memory, Install, Skills, Extraction |
| operations | `src/operations/` | cross-domain workflows: init, sync, update, task_verify, harness apply/measure, feature_prepare, migrate, Memory suggestion/scorer/promotion/maintenance |
| interfaces | `src/interfaces/` | adapters: cli, mcp, tui, hooks, shell — parse + render; domain rules stay behind owning facades |

### Deep primitives (small interface, much hidden behavior) — `foundation/core`
- `write_string_if_unchanged` — content-hash compare-and-swap + `.{name}.write-lock` marker, 15-min stale reclaim — `fs.rs:121`
- `try_reserve_marker_dir` + `DirReservation` — atomic id reservation via `.alloc-` marker dir, RAII cleanup on drop (`ALLOC_MARKER_PREFIX` `fs.rs:30`) — `fs.rs:33`
- `write_new_dir_atomic` — build in a temp root, publish by rename — `fs.rs:291`
- `append_text_file` — append-once / create-new, trailing-newline repair — `fs.rs:51`
- `child_dirs` — symlink-safe directory walk — `fs.rs:324`
- `write_string_atomic` — temp-sibling + rename without blocking fsync on the hot path — `safe_write.rs`

---

## 2. The card model (as-built)

Maestro uses three work levels:

```text
High = Card
Mid  = CardKind / workflow kind
Low  = Task
```

`Card` is the durable identity, container, lifecycle, and governance record.
`Task` is the atomic executable progress unit. `Facet` sidecars (`spec.md`,
`qa.md`, `notes.md`) describe or prove one parent card when needed.

The shipped store still uses a generic **deep card core** + a closed
`pub enum CardType` (`feature | custom | progress | task | bug | chore | idea |
decision | memory`). `type: task` is retained for legacy readable/workable task cards;
the target low-level Task abstraction is the `TaskRecord` payload used by
card-backed task records and by Progress `progress.yml`. There is no `CardType`
trait in the shipped model. The type seam is explicit exhaustive `match`
dispatch at the card-store, query, edit, and CLI boundaries; per-type lifecycle
rules stay in the owning domain modules.

```text
generic CARD CORE  -- never changes when a type is added/altered
  schema · store(save-if-unchanged) · id-reservation · scan · archive ·
  query(ready/list) · CLI(create/show/update/dep/close)
        |  dispatch by exhaustive match on CardType
   enum CardType: feature · custom · progress · task(legacy) · bug · chore · idea · decision · memory
        ├ card store: placement · id prefix · save basis · reconcile
        ├ card query/edit/CLI: ready/list filters · type hints · close guards
        └ domain lifecycles:
             feature  (proposed->ready->in_progress->closed, +cancelled) CardKind
             progress (in_progress card + progress.yml TaskRecord rows) lightweight CardKind
             task     (legacy card-backed TaskRecord compatibility)
             bug/chore/custom (workflow CardKinds that own TaskRecord work)
             idea     (proposed->accepted->measured, +dismissed)
             decision (open->locked, +superseded)
             memory   (card.status outer workflow + memory.lifecycle semantics)
```

Trait revisit trigger: introduce a per-type behavior trait only if future card
type work makes match-based per-type behavior multiply enough that a trait removes
real duplication. Until then, exhaustive matches
are the intended safety mechanism: adding a `CardType` variant forces every dispatch
site to be reviewed by the compiler.

- **card fields:** `id`, `type`, `status`, `parent` (card id | null), `deps[]`, `lane`,
  `claimed_by = <agent>#<session>` (agent in claude | codex | future-cli), title,
  description, acceptance, timestamps; prose (`spec.md`/`notes.md`/`qa.md`) as
  optional **facet sidecars** for any card that needs contract, evidence, or history.
- **types:** `feature | custom | progress | task | bug | chore | idea | decision | memory`.
  `task` remains a compatibility card type; new low-ceremony Tasks are stored
  under Progress cards.
- **status:** each type stores its REAL state; a coarse `open | in_progress | closed`
  is DERIVED for the board (single source of truth, can't desync).
- **ids:** stable and opaque after creation. Features use their creation slug; other
  cards mint readable typed slug ids (`progress-<slug>-<hex4>`, `task-<slug>-<hex4>`,
  `bug-...`, `chore-...`, `custom-...`, `dec-...`, `idea-...`, `mem-...`). Legacy `card-<hex>` ids stay valid but are not minted for
  new non-feature cards. The dotted `<feature>.<N>` form is a **display alias**
  rendered only by `show` (marked "display only"), never a ref and never parsed —
  addressing by position broke under reparenting, so it was demoted from the original
  dotted-id design.
- **storage (one card store, feature is just a card):**
  ```text
  .maestro/cards/<feature>/card.yaml                    # feature container card
  .maestro/cards/<feature>/{spec.md,notes.md,qa.md}     # feature facets
  .maestro/cards/<progress>/card.yaml                   # lightweight progress card
  .maestro/cards/<progress>/progress.yml                # low-level TaskRecord rows
  .maestro/cards/<feature>/tasks/<task>/task.yaml       # task/bug/chore cards
  .maestro/cards/<feature>/decisions.yaml               # decision entries
  .maestro/cards/<memory>/card.yaml                     # native Memory card
  .maestro/cards/<memory>/memory/{candidate.yml,lesson.md,signals.jsonl,receipts/}
  .maestro/cards/tasks/<task>/task.yaml                 # parentless task/bug/chore cards
  .maestro/cards/decisions.yaml                         # parentless decision entries
  .maestro/cards/ideas.yaml                             # harness idea entries
  .maestro/memory/{suggestions.jsonl,target-registry.yml,health-ledger.jsonl}
  .maestro/memory/promotions/<promotion>/plan.yml
  .maestro/memory/maintenance/<maintenance>/contract.yml
  .maestro/harness/harness.yml                          # config only
  .maestro/archive/cards/
  ```
  Folds the old `features/` + `tasks/` + `harness/backlog.yaml` + `decisions/` trees
  into one store (`maestro migrate` remints v1 repos). Legacy task-family cards keep
  per-card dirs for compatibility and contention-free gated work; Progress cards keep
  small same-session TaskRecord rows in one `progress.yml`. Decisions and ideas are
  entry-backed where their owning domain still treats them as rosters. (NOT Dolt;
  file-native.)
- **management:** global QUERY, not directory navigation — `maestro ready [<feature>]`,
  `maestro list --parent --type --assignee --status`, beads-style verbs
  (`claim`/`show`/`note`/`dep add`/`archive`), emoji-free, `--json` parity.
- **edges:** `parent` · structured blockers (`dep add <child> <blocker>`) · `related` ·
  `supersedes` (non-blocking).
- **linked-card inbox:** `related` links expose an inbox/message coordination
  channel. Messages are advisory only: they can suggest cross-card task order,
  but readiness, next, claim, and verification consult explicit Task
  blockers/dependencies, not inbox unread state.
- **skills:** one bundled `maestro-card` skill (router `SKILL.md` +
  `reference/{work,feature,verify,qa-baseline,qa-slice}.md`) covers the active-work
  cluster; `maestro-setup` / `maestro-design` / `maestro-audit` stay separate.

Type mapping: feature -> `type:feature` CardKind · custom/bug/chore -> workflow
CardKinds · progress -> lightweight CardKind with `progress.yml` · legacy task
card -> `type:task` · harness item -> `type:idea` · decision -> `type:decision` ·
Memory lesson/proposal -> `type:memory` with card-owned `memory/` sidecars.

Memory is a promotion and reuse layer over existing evidence, not a hidden
planner. `src/domain/memory` validates candidate sidecars, source refs, risk,
gates, and card-status-to-`memory.lifecycle` mapping. `src/operations/memory`
owns visible suggestions, typed scorer receipts, two-stage promotion
plan/apply, approved-Memory read selection, and bounded maintenance contracts.
Normal work and Work Lease can read compact approved Memory refs; current user
instructions, locked acceptance, proof/QA, and run authority outrank Memory.

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
