# Whole-codebase review swarm findings

Date: 2026-09-04

Scope: read-only review of the entire `/Users/reinamaccredy/Code/maestro` codebase, including the Bun/TypeScript CLI and kernel, built-in and external plugin boundaries, SQLite stores and search indexes, SLP runtime, installer and trust handling, tests, desktop app, and site documentation. The checkout changed concurrently during the review, so evidence was collected across the Phase 1 commits through `da0bbf67` and rechecked against the later clean snapshot at `bc3fae8f`.

Intent used for review: Maestro is a local-first coordination CLI. The kernel remains mechanism-only, plugins own verbs and optional policy, there is no daemon or scheduler, declared read-only behavior fails closed, external-plugin trust binds executable content, durable records remain coherent, and source, tests, installer behavior, and documentation agree.

## Summary

Eight material findings survived the main-agent evidence filter:

- 4 high-priority correctness, integrity, security, or reliability findings
- 3 medium-priority contract and data-integrity findings
- 1 medium-priority documentation-contract finding

No source fix was applied as part of the review.

## Findings

### 1. Trusted plugin imports can escape the hashed artifact

- Severity: high
- Category: security / plugin trust
- Confidence: high
- Evidence: `src/plugins/plugin-trust.ts:55-89`, `src/kernel/loader.ts:289-316`

`artifactDigest` hashes only regular files physically contained by the configured plugin artifact root. After that check, the loader imports the entrypoint with normal TypeScript module resolution. A single-file plugin can import another local file outside its one-file root, and a directory plugin can import a path such as `../helper.ts`. Changing that outside-root dependency changes code executed on the next Maestro invocation without changing the stored trust digest.

This violates the documented content-bound trust guarantee: the grant does not necessarily cover all locally imported executable code.

Recommended follow-up:

- Reject local imports whose canonical targets leave the canonical artifact root, or digest and verify the complete local dependency closure.
- Alternatively, load plugins from a content-addressed immutable copy.
- Add a regression test that trusts a plugin, changes an outside-root imported module, and proves the plugin no longer loads.

### 2. Superseded and retracted memory facts remain searchable

- Severity: high
- Category: persistent-memory integrity
- Confidence: high
- Evidence: `src/plugins/memory.ts:151-158`, `src/plugins/memory.ts:390-398`, `src/plugins/memory.ts:557-559`, `src/plugins/memory.ts:615-620`, `src/plugins/observability.ts:159-178`

`indexFact` always inserts a memory fact into `search_index`, regardless of whether its state is `active` or `superseded`. Superseding a target changes the row state but does not remove the target's index entry. Explicit retraction then calls `indexFact` on the retired row. Startup also re-indexes every fact through `listFacts(context, true)`, and `memoryHit` accepts rows without filtering for active state.

As a result, ordinary `maestro search` can return guidance that the owner explicitly superseded or retracted. That defeats the stale-rule kill case the memory feature is intended to guarantee.

Recommended follow-up:

- Index only active facts.
- Delete a target's memory search row atomically when it is superseded or retracted.
- Defensively constrain `memoryHit` to active facts for ordinary search.
- Extend memory tests 569 and 570 to prove both replacement and explicit retraction disappear from search.

### 3. Hub-search failures are returned as successful partial searches

- Severity: high
- Category: reliability / fail-closed contract
- Confidence: high
- Evidence: `src/plugins/observability.ts:295-328`, `src/plugins/observability.ts:469-480`, `src/kernel/cli.ts:260-268`

When the child Hub search exits unsuccessfully, `searchHub` returns `status: "error"`. The parent search merely appends a human-readable `hub: unavailable` line and still returns success with project-only results. In JSON mode the CLI emits only the structured `data`, so the text warning is discarded and the response is indistinguishable from a complete `ok: true` cross-store search.

This is reachable when the Hub search index is stale under `MAESTRO_READ_ONLY`, when the Hub store is too new, corrupt, unreadable, or when the child output cannot be parsed. Normal search promises project plus Hub results, while `--local` already provides the explicit reduced-scope path.

Recommended follow-up:

- Propagate the Hub child's structured failure from normal cross-store search.
- Preserve `--local` as the explicit project-only fallback.
- Add text and JSON tests for stale-index and `STORE_TOO_NEW` Hub failures.

### 4. Bundle-import database rollback cannot undo copied files

- Severity: high
- Category: reliability / transactionality
- Confidence: high
- Evidence: `src/plugins/bundle-import.ts:199-256`, especially `src/plugins/bundle-import.ts:223-249`, and `src/plugins/bundle-import.ts:342-343`

`importWaymarkTree` performs recursive `cpSync` operations inside a SQLite transaction. If a copy fails partway through, or if any later database operation aborts the transaction, SQLite rolls back the rows but already copied bundle directories remain on disk. A later retry sees those destination directories and reports them as skipped, potentially leaving a filesystem bundle with no corresponding durable store row.

Recommended follow-up:

- Stage each copy in a temporary directory and rename it into place only after the durable commit, or add complete compensating cleanup for failed imports.
- Add an injected mid-import failure test that proves no partial destination survives and a retry succeeds.

### 5. `--limit` is applied independently to each store

- Severity: medium
- Category: search contract
- Confidence: high
- Evidence: `src/plugins/observability.ts:460-475`

Project hits are sliced to the requested limit before Hub search. The Hub child independently returns up to the same limit, and every Hub result is appended. Therefore `maestro search --limit 5` can return ten records. The `more` count is also computed only from project hits, so it is not a truthful count for the combined result.

Recommended follow-up:

- Merge project and Hub result sets before applying one global cap.
- Define deterministic cross-store ordering.
- Compute one combined omitted-result count.
- Add a test with more than the limit in both stores.

### 6. Observer mode blocks the new pure term and memory reads

- Severity: medium
- Category: read-only regression
- Confidence: high
- Evidence: `src/plugins/observer-mode.ts:3-28`, `src/plugins/term.ts:173-206`, `src/plugins/memory.ts:639-674`, `src/plugins/memory.ts:679-737`, `tests/read-purity.test.ts:139-168`

The observer-mode built-in allowlist does not include `term` or `memory`. Therefore `MAESTRO_READ_ONLY=1 maestro term list|show` and `maestro memory list|show` fail before their explicitly non-mutating handlers can load. A live smoke check returned `READ_ONLY` for `term list`.

`memory render --check` is also logically read-only, but it shares one command definition whose default metadata is mutating. The current read-purity sweep does not detect the missing commands because it builds its pure-command set by discarding every invocation that returns `READ_ONLY`.

Recommended follow-up:

- Admit the `term` and `memory` built-ins in observer mode while retaining command-level write guards.
- Give render checking a path whose mutability can be classified before command dispatch, for example a separate pure command or equivalent command metadata design.
- Explicitly assert the expected term and memory pure commands in observer-mode and read-purity tests.

### 7. Generated IDs and user-defined names occupy an ambiguous lookup namespace

- Severity: medium
- Category: data integrity / lookup contract
- Confidence: high
- Evidence: `src/plugins/term.ts:40-44`, `src/plugins/memory.ts:125-129`

Term lookup uses `WHERE id = ? OR name = ?`; memory lookup uses the corresponding `id = ? OR slug = ?` pattern. After a term receives generated ID `t1`, an attempt to add a term literally named `t1` resolves the existing row by ID and silently redefines it instead of creating the requested term. Memory IDs and slugs can collide similarly, allowing `show`, `retract`, or an internal post-insert lookup to select an unintended row.

Recommended follow-up:

- Reserve generated-ID-shaped names and slugs, or expose explicit ID-versus-name lookup forms.
- Audit existing stores for collisions before changing lookup semantics.
- Add crossed-namespace regression tests for term add/show and memory show/retract.

### 8. Published documentation still describes the pre-Phase-1 CLI

- Severity: medium
- Category: documentation / release contract
- Confidence: high
- Evidence: `site/src/content/docs/reference/cli.md:289-295`, `site/src/content/docs/guides/observer-mode.md:9-33`, contrasted with `README.md:160-168` and live CLI help

The published CLI reference still describes search as covering local work, decisions, notes, bundles, and imported legacy records. It omits `term`, every `memory` command, the `--local` flag, Hub search, and term/memory search surfaces. The observer-mode guide also says read-only mode does not grant access to another project store without explaining the new default Hub read. README and runtime help therefore disagree with the site users are directed to.

Recommended follow-up:

- Update the CLI reference and observer-mode guide with the Phase 1 command and store semantics.
- Add documentation-contract assertions for `term`, `memory`, `--local`, and default Hub search.

## Prioritized remediation

### Fix now

1. Close the plugin trust dependency-closure gap.
2. Remove superseded and retracted memory from default retrieval.
3. Make normal cross-store search fail closed when Hub search fails.
4. Make bundle import recover atomically from filesystem or database failure.

### Fix soon

1. Apply one global result limit across project and Hub stores.
2. Restore observer access to all declared pure term and memory reads.
3. Separate generated IDs from user-defined names and slugs.
4. Bring the published CLI and observer documentation into parity.

## Verification evidence

Verification was read-only with respect to source files. Tests created and removed their own temporary fixtures.

- `bunx tsc --noEmit`: passed on the current expanded workflow/skills snapshot.
- Focused tests covering workflow, docs contracts, handoff, memory, and bundle import: 33 passed, 0 failed.
- `git diff --check`: passed.
- Earlier full-suite snapshot: 532 passed, 1 skipped, 2 failed across 535 tests.
  - `attention --json computes findings without background state` failed because the sandbox denied spawning `ps` with `EPERM`.
  - `SLP v2 reads the pane identity from an ancestor process when the shell dropped HERDR_PANE_ID` returned `ROLE_UNPROVEN` under the same constrained process-inspection environment.
  - The skipped test was the live-Herdr nine-operation journey.
- The five Phase 1 memory, term, and cross-store-search tests passed.
- Architectural gate inspection found no kernel policy vocabulary violation and no escape-hatch flag violation. The only `setTimeout` matches were bounded SLP runtime timers, not a daemon or scheduler.

## Review limitations

- The repository changed concurrently while the review was running. Phase 1 was committed during the review, followed by workflow/skill expansion commits. Findings were rechecked against the later clean snapshot, but the review was not performed against one immutable commit from beginning to end.
- The first four fixed-role reviewers failed before inspection because their configured model was unavailable to the account. Replacement reviewers exceeded the useful wait bound. A second bounded set then failed on account usage limits. Two independent reviewer reports did arrive, and the main reviewer completed and validated the regression, reliability, contracts, and coverage passes.
- No installed-runtime, live-Herdr, desktop GUI, or published-site journey was claimed by this review.

## Considered but not retained as findings

### Pane IDs as authentication

A reviewer raised role impersonation through caller-supplied `HERDR_PANE_ID`. This was not retained because Maestro explicitly documents SLP as a cooperative protocol rather than a shell security sandbox; host policy and the Human own topology and external-effect enforcement. Treating the pane ID as a hostile-principal authentication token would impose a stronger security claim than the product makes.

### WAL sidecar creation in read-only mode

A reviewer raised the writable SQLite fallback that can recreate `-wal` or `-shm` sidecars. This was not retained because the repository explicitly defines read purity in terms of logical store content, documents why WAL sidecars may need to reappear, enables `PRAGMA query_only`, and tests that domain content remains unchanged.

### `plugin add` grants trust in one command

A reviewer characterized immediate trust after `plugin add <url>` as accidental. The implementation explicitly states that naming the Git URL is the trust act, and tests enforce that behavior. This is a documentation/product-policy choice rather than an implementation regression. The separate plugin dependency-closure finding remains valid because it breaks the bytes-bound guarantee even after that intentional trust act.

## Disposition (2026-09-05)

- Finding 1 fixed (w602, a48f5941): a Bun resolver hook refuses any relative or absolute import that leaves the canonical artifact root; `plugin list` shows the plugin as `error` with the file and specifier; test 516.
- Finding 7 fixed (w603, 9a98ec97): `t<number>` and `m<number>` reserved at the write seam; `bundle import` reports such a CONTEXT.md term as skipped in dry run and real run; tests 610-612; live stores audited, no existing collisions, no migration.
- Findings 2-6 fixed earlier (w599); finding 8 docs fixed in the same pass. Released as v0.118.0.
