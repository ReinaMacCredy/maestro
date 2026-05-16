# Phase H — Handoff MCP Tools (Refined, opus, 2026-05-16)

> Plan-agent initial design refined via `/mcp-builder` and aligned to existing
> maestro MCP conventions (task-tools.ts, evidence-tools.ts, contract-tools.ts).
> Ready for implementation.

## Style alignment (decisions inherited from existing tools)

- **Naming**: `maestro_handoff_<verb>` (snake_case, service prefix). Matches `maestro_task_*`, `maestro_evidence_*`, `maestro_contract_*`.
- **Pagination**: reuse `paginate()` from `src/features/mcp/server/pagination.ts` — default 20, max 100, response `{ items, pagination: { total, limit, offset, hasMore } }`.
- **Projection**: list verbs accept `view: "summary" | "full"` from `PROJECTION_VIEWS`. Summary stays under token budget.
- **Errors**: use `fail(code, message, { hints, arg })` + `toCallToolResult()` — already routes `isError: true`. Codes are SCREAMING_SNAKE.
- **Schemas**: strict zod objects (`.strict()`); reuse the existing `taskId` constant for task ids.
- **Annotations**: declare all four (`readOnlyHint`, `destructiveHint`, `idempotentHint`, `openWorldHint`).
- **Content shape**: `JSON.stringify(result.data)` text content — matches the rest.

## Preliminary decisions (kept from plan-agent draft)

1. **Pickup sidecar**: `<hnd-...>.picked_up.json` alongside the envelope. Atomic exclusive create via `writeFile(path, _, { flag: "wx" })` → `EEXIST` returns `HANDOFF_ALREADY_PICKED_UP`. Matches skill vocabulary ("pickup protocol").
2. **Port extension** (not new port): extend `HandoffEmitterPort` with `markPickedUp(id, payload)` and `getPickup(id)`. `services.v2.handoffEmitter` is already DI-wired.
3. **`emit` accepts all five trigger verbs**: no dedup enforcement. Pure write with no task-state side effects. Returns the full envelope (with generated `id` + `created_at`).
4. **Stale MCP table in `maestro-task/SKILL.md`**: replace only the handoff row this PR; flag other stale rows in a comment but do not silently rewrite.
5. **Tests**: schema tests in `inputs.test.ts`; handler behavior in `mcp-stdio-flow.test.ts`. Match existing convention.
6. **Pickup tool name**: `maestro_handoff_pickup` — a bookkeeping mark, not a process launch.
7. **`list` collapses `open_for_task`**: single `maestro_handoff_list` with optional `task_id` + `include_picked_up` filters.

## Tool surface (refined)

### 1. `maestro_handoff_list`

```
Title:       List handoff envelopes
Read-only:   yes (idempotent)
Description: List handoff envelopes left at .maestro/handoffs/. Filters:
             task_id, include_picked_up (default false = open work only),
             trigger_verb. Paginated (default 20, max 100). view='summary'
             (default) returns id+task_id+trigger_verb+created_at+picked_up;
             view='full' returns the envelope and pickup metadata. Read-only.
```

**Input** (`HandoffListInput`):
| Field               | Type                                  | Notes                                                  |
|---------------------|---------------------------------------|--------------------------------------------------------|
| `task_id`           | `taskId` (optional)                   | Reuse shared zod constant.                             |
| `trigger_verb`      | `enum(...HANDOFF_TRIGGERS)` (opt)     | Filter by `task:claim`/`task:block`/etc.               |
| `include_picked_up` | `boolean` (opt, default `false`)      | Default surfaces only un-claimed envelopes.            |
| `limit`             | shared `limit`                        | 1..100, default 20.                                    |
| `offset`            | shared `offset`                       | Default 0.                                             |
| `view`              | shared `view`                         | summary / full.                                        |

**Summary item**:
```ts
{ id, task_id, trigger_verb, created_at, picked_up: boolean }
```

**Full item**:
```ts
{ envelope: HandoffEnvelope, picked_up?: HandoffPickup }
```

**Errors**: `HANDOFF_LIST_FAILED` (catch-all from emitter port).

**Annotations**: `readOnlyHint: true, idempotentHint: true, destructiveHint: false, openWorldHint: false`.

---

### 2. `maestro_handoff_show`

```
Title:       Show a handoff envelope
Read-only:   yes (idempotent)
Description: Fetch a single envelope by id (hnd-*). Returns
             {envelope, picked_up?}. Error codes: HANDOFF_NOT_FOUND,
             HANDOFF_MALFORMED. Read-only.
```

**Input** (`HandoffShowInput`):
| Field | Type                                                   | Notes                          |
|-------|--------------------------------------------------------|--------------------------------|
| `id`  | `regex(/^hnd-[a-z0-9]+-[a-z0-9]+$/)`                   | Match the generator pattern.   |

**Errors**:
- `HANDOFF_NOT_FOUND` — envelope file missing. Hint: list with `task_id` filter.
- `HANDOFF_MALFORMED` — JSON parse fails or required field missing. Hint: `.maestro/handoffs/<id>.json may have been hand-edited`.

**Annotations**: read-only, idempotent.

---

### 3. `maestro_handoff_emit`

```
Title:       Emit a handoff envelope
Read-only:   no (writes to .maestro/handoffs/)
Idempotent:  no (each call creates a unique id)
Description: Write a handoff envelope so a follow-up agent can pick up the
             task. Used when an agent must hand off mid-stream without going
             through claim/block (e.g. ship/verify/abandon paths that don't
             yet emit on their own). The lifecycle verbs claim and block
             already emit automatically — do not re-emit them. Returns the
             envelope including the generated id and created_at. Error
             codes: HANDOFF_EMIT_FAILED, INVALID_ARG.
```

**Input** (`HandoffEmitInput`):
| Field           | Type                                       | Notes                                                |
|-----------------|--------------------------------------------|------------------------------------------------------|
| `task_id`       | `taskId`                                   | Required.                                            |
| `trigger_verb`  | `enum(...HANDOFF_TRIGGERS)`                | One of the five canonical verbs.                     |
| `agent_id`      | `string.min(1)` (opt)                      | Defaults to MCP session id, matching `task_claim`.   |
| `worktree_path` | `string.min(1)` (opt)                      | Absolute or repo-root-relative.                      |
| `spec_path`     | `string.min(1)` (opt)                      | Spec markdown the receiver should load first.        |
| `reason`        | `string.min(1)` (opt; required if verb=`task:block`) | Cross-field refine.                  |

**Cross-field refine**: when `trigger_verb === "task:block"` and `reason` is undefined → `INVALID_ARG` (`arg: "reason"`).

**Returns**: `{ envelope }` — the materialized envelope with id + timestamp.

**Errors**:
- `INVALID_ARG` — refine failures (missing reason on block, etc.). Hint enumerates allowed verbs.
- `HANDOFF_EMIT_FAILED` — fs write failure.

**Annotations**: `readOnlyHint: false, idempotentHint: false, destructiveHint: false, openWorldHint: false`.

---

### 4. `maestro_handoff_pickup`

```
Title:       Mark a handoff envelope as picked up
Read-only:   no (writes a pickup sidecar)
Idempotent:  no (second call returns HANDOFF_ALREADY_PICKED_UP)
Description: Record that the calling agent has read this envelope and is
             taking the work, so concurrent agents do not duplicate. This is
             a bookkeeping mark; it does not claim the task — call
             maestro_task_claim separately. Writes a sidecar at
             .maestro/handoffs/<id>.picked_up.json using exclusive create;
             a second pickup attempt returns HANDOFF_ALREADY_PICKED_UP.
             Error codes: HANDOFF_NOT_FOUND, HANDOFF_ALREADY_PICKED_UP,
             HANDOFF_PICKUP_FAILED, HANDOFF_MALFORMED.
```

**Input** (`HandoffPickupInput`):
| Field           | Type                                                 | Notes                                              |
|-----------------|------------------------------------------------------|----------------------------------------------------|
| `id`            | `regex(/^hnd-[a-z0-9]+-[a-z0-9]+$/)`                 | Envelope id.                                       |
| `picked_up_by`  | `string.min(1)` (opt)                                | Defaults to MCP session id.                        |
| `note`          | `string.min(1)` (opt)                                | Free-text reason / intent.                         |

**Returns**: `{ envelope, pickup: HandoffPickup }` so the caller can immediately read envelope fields without a follow-up `show`.

**Errors**:
- `HANDOFF_NOT_FOUND` — envelope absent. Hint: list to discover ids.
- `HANDOFF_ALREADY_PICKED_UP` — sidecar already exists. Returned in the body alongside the existing pickup metadata as a hint, so the caller can choose to defer.
- `HANDOFF_MALFORMED` — envelope JSON corrupt.
- `HANDOFF_PICKUP_FAILED` — fs write failure (non-EEXIST).

**Annotations**: `readOnlyHint: false, idempotentHint: false, destructiveHint: false, openWorldHint: false`.

## Pickup design (unchanged from plan-agent draft)

- Sidecar file at `<hnd-id>.picked_up.json` alongside the envelope.
- Atomic exclusive create via `writeFile(path, body, { flag: "wx" })`. `EEXIST` → `HANDOFF_ALREADY_PICKED_UP`.
- Envelopes remain immutable. Pickup state lives outside the envelope.
- Sidecar payload:
  ```ts
  interface HandoffPickup {
    readonly id: string;              // pkp-<base36>-<rand>
    readonly envelope_id: string;     // hnd-...
    readonly picked_up_by: string;    // agent / session id
    readonly picked_up_at: string;    // ISO-8601
    readonly note?: string;
  }
  ```

## Constants (new)

In `src/repo/handoff-emitter.port.ts`:

```ts
export const HANDOFF_TRIGGERS = [
  "task:claim",
  "task:block",
  "task:abandon",
  "task:ship",
  "task:verify",
] as const;

export type HandoffTrigger = (typeof HANDOFF_TRIGGERS)[number];
```

So schemas can `z.enum(HANDOFF_TRIGGERS)` without redefining the list.

## Files

**Create**:
- `src/features/mcp/server/tools/handoff-tools.ts` — 4 `server.registerTool` calls.

**Modify**:
- `src/repo/handoff-emitter.port.ts` — export `HANDOFF_TRIGGERS` constant, add `HandoffPickup` interface, add 2 method signatures (`markPickedUp`, `getPickup`).
- `src/repo/fs-handoff-emitter.adapter.ts`
  - Implement `markPickedUp` (exclusive create) + `getPickup`.
  - **Critical**: update `list()` filter to exclude `*.picked_up.json` sidecars. Without this fix, the existing `list()` deserializes sidecars as malformed envelopes and breaks every read.
- `src/features/mcp/server/schemas/inputs.ts` — add 4 schemas (`HandoffListInput`, `HandoffShowInput`, `HandoffEmitInput` w/ refine, `HandoffPickupInput`).
- `src/features/mcp/server/mcp-server.ts` — `registerHandoffTools(server, deps)`.
- `tests/unit/features/mcp/server/schemas/inputs.test.ts` — 4 describe blocks (one per schema, covering happy + refine failure cases).
- `tests/e2e/mcp-stdio-flow.test.ts` — 5 it-blocks: list-empty, emit-then-list, show, pickup-first-call, pickup-second-call-EEXIST.
- `skills/bundled/maestro-handoff/SKILL.md` — append MCP tools table section.
- `skills/bundled/maestro-task/SKILL.md` — replace the `_no MCP tools for handoffs_` row with the new tool list; add a one-line note flagging that other stale rows in the table will be rewritten in a follow-up.

## Risks

- **R1 (resolved)**: `list()` sidecar bleed — fixed in the same PR, tested.
- **R2 (debt)**: port naming tension — `HandoffEmitterPort` will own read+write methods. Accept as known debt. Renaming to `HandoffStorePort` is a follow-up that can land alongside the ship/verify/abandon emit wiring (Phase H+1).
- **R3 (debt)**: stale rows elsewhere in `maestro-task` MCP table — flagged but not rewritten this PR. Track as Phase H+2.

## Status

- Refined design complete. Ready to implement.
- Implementation order: port + adapter + sidecar test → schemas + schema tests → tools file + e2e test → skill updates → sync + check + build.
