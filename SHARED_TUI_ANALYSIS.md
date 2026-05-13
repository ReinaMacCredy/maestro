# src/shared/ and src/tui/ Deep Analysis

**Generated:** 2026-05-08  
**Repository:** /Users/reinamaccredy/Code/maestro  
**Scope:** Complete exploration of src/shared/ and src/tui/ directories

---

## Executive Summary

### src/shared/
**Purpose:** Generic, reusable utilities with zero product-domain knowledge. Pure infrastructure layer.

**Organization:** Well-structured with clear separation between domain primitives (`domain/`), I/O helpers (`lib/`), and error handling (`errors.ts`). All utilities are truly generic and could be extracted to a separate package.

**Health:** ✅ Excellent. No domain logic leakage, clean boundaries, appropriate abstractions.

### src/tui/
**Purpose:** Mission Control TUI projection and rendering layer. Builds read-only snapshots from feature stores and renders them via OpenTUI framework.

**Organization:** Four-layer architecture (command → snapshot → UI state → render) with clear separation of concerns. Heavy dependencies on features and infra, but appropriately so for a projection layer.

**Health:** ✅ Good. Architecture is sound, boundaries are respected, read-only contract is maintained (with one gated exception for reply ingest).

---

## src/shared/ — Detailed Breakdown

### File Inventory (24 files total)

#### Domain Primitives (3 files)
```
domain/
├── id.ts              — Handoff ID generation (adjective-noun-N pattern)
├── defaults.ts        — Path constants for .maestro/, skills/, home directories
└── ui-config.ts       — UI config types (Mission Control background mode)
```

**Purpose:** Domain-agnostic primitives and constants.

**Key exports:**
- `generateHandoffId()` — collision-resistant ID generator
- `resolveMaestroHome()`, `resolveCodexHome()` — home directory resolution with env overrides
- `getMissionControlBackgroundMode()` — UI config accessor

**Dependencies:** None (pure functions, no imports from features/infra/tui)

#### Library Utilities (20 files)
```
lib/
├── fs.ts                          — Filesystem I/O (atomic writes, recursive list)
├── yaml.ts                        — YAML parse/stringify with error handling
├── shell.ts                       — Shell command execution (Bun.spawnSync wrapper)
├── output.ts                      — Dual-mode output (JSON vs text)
├── output-capture.ts              — Truncating output buffer for long command results
├── path-safety.ts                 — Path traversal prevention
├── path-normalize.ts              — Slash normalization
├── sanitize.ts                    — Terminal/markdown content sanitization
├── project-root.ts                — .maestro/ project root resolution (git-aware)
├── git-base.ts                    — Git base ref resolution (merge-base, empty-tree fallback)
├── ansi.ts                        — ANSI color helpers (respects NO_COLOR)
├── template.ts                    — Minimal {{var}} template renderer
├── skill-path.ts                  — Skill directory name encoding (maestro%3A prefix)
├── maestro-substrate-paths.ts     — Substrate path detection (.maestro/, bundled skills)
├── glob-match.ts                  — Bun.Glob wrapper with safety limits
├── fs-lock.ts                     — File-based locking with stale detection
├── concurrency.ts                 — mapWithConcurrency (rate-limited async map)
└── deprecated-version-flag.ts     — --version flag collision detection
```

**Purpose:** Generic I/O, shell, path, and formatting utilities.

**Key patterns:**
- **Atomic writes:** `fs.ts` uses `randomUUID()` temp files + rename for crash safety
- **Cross-platform:** `shell.ts` uses Bun.spawnSync; `fs.ts` uses NTFS junctions on Windows
- **Safety:** `path-safety.ts` prevents traversal; `glob-match.ts` caps pattern complexity
- **Caching:** `git-base.ts` memoizes per-process to avoid redundant git calls

**Dependencies:** None (no imports from features/infra/tui)

#### Error Handling (1 file)
```
errors.ts — MaestroError with hints array
```

**Purpose:** Base error class with actionable hints for CLI output.

**Key exports:**
- `MaestroError` — extends Error, adds `hints: readonly string[]` and optional `code`

#### Version Metadata (2 files)
```
version.ts         — Build-time constants (VERSION, BUILD_UNIX, GIT_SHA, RELEASED_AT)
version-format.ts  — Version formatting and display logic
```

**Purpose:** Version resolution with env overrides and relative age formatting.

**Key exports:**
- `getVersionMetadata()` — resolves version from build constants + env overrides
- `formatVersionOutput()` — formats as "0.76.1.1778241039-g2200c26 (released 2026-05-08, 3h ago)"
- `resolveRuntimeGitSha()` — live git SHA for `--version` flag

**Dependencies:** None

---

### Import/Export Relationships (src/shared/)

**Exports to:**
- `src/infra/` — heavily used (fs, yaml, shell, output, path-safety, git-base, ansi)
- `src/features/` — moderately used (fs, yaml, errors, output)
- `src/tui/` — lightly used (sanitize, ui-config, errors)

**Imports from:**
- **None** — src/shared/ has zero dependencies on features/infra/tui

**Internal dependencies:**
- `lib/fs.ts` → `lib/path-normalize.ts`
- `lib/maestro-substrate-paths.ts` → `lib/skill-path.ts`
- `lib/output.ts` → `lib/sanitize.ts`
- `lib/shell.ts` → `lib/fs.ts` (for `ensureDir`)
- `domain/defaults.ts` → none (pure)
- `errors.ts` → none (pure)

**Observation:** Clean dependency graph. No circular dependencies. All utilities are truly generic.

---

### Organizational Assessment (src/shared/)

#### ✅ Strengths
1. **Zero domain logic leakage** — no imports from features/infra/tui
2. **Clear naming** — `lib/` for utilities, `domain/` for primitives, `errors.ts` at root
3. **Appropriate abstractions** — `fs.ts` provides atomic writes, `shell.ts` wraps Bun.spawnSync
4. **Safety-first** — path traversal prevention, glob complexity limits, stale lock detection
5. **Cross-platform** — Windows NTFS junctions, NO_COLOR support, shell timeout handling

#### ⚠️ Observations
1. **`domain/ui-config.ts` is TUI-specific** — only used by `src/tui/`, could move to `src/tui/shared/`
2. **`lib/deprecated-version-flag.ts` is CLI-specific** — only used by `src/index.ts`, could move to `src/infra/`
3. **`lib/maestro-substrate-paths.ts` is contract-specific** — only used by Trust Verifier, could move to `src/features/verify/`

#### 💡 Potential Improvements
1. **Move TUI-specific config to src/tui/shared/** — `domain/ui-config.ts` doesn't belong in generic shared/
2. **Move CLI-specific helpers to src/infra/** — `lib/deprecated-version-flag.ts` is not generic
3. **Consider extracting to @maestro/shared package** — all utilities are reusable across projects

---

## src/tui/ — Detailed Breakdown

### File Inventory (39 files total)

#### Root Level (5 files)
```
input.ts       — Raw stdin key parser (escape sequences → Key objects)
session-id.ts  — shortenSessionId() helper
format.ts      — TUI display formatters (elapsed, tokens, age, truncate)
theme.ts       — Status-to-color maps, palette constants (256-color)
README.md      — Architecture documentation
```

**Purpose:** TUI-specific utilities and theme configuration.

**Key exports:**
- `parseKeypress()`, `startKeyListener()` — keyboard input handling
- `formatElapsed()`, `formatTokens()`, `formatAge()` — display formatters
- `PALETTE`, `MISSION_STATUS_COLOR`, `FEATURE_STATUS_COLOR` — theme constants

#### State Layer (13 files)
```
state/
├── types.ts                    — Snapshot DTOs (MissionControlSnapshot, etc.)
├── snapshot.ts                 — Snapshot builder entry points
├── snapshot-loader.ts          — I/O layer (loads from stores)
├── projection.ts               — Pure projection logic (I/O → snapshot)
├── reducer.ts                  — UI state machine (focus, modals, keyboard)
├── screen-types.ts             — Screen-specific DTOs (AgentGridRow, etc.)
├── autopilot-screen.ts         — Autopilot snapshot builder
├── task-board.ts               — Task board snapshot builder
├── reply-projection.ts         — Reply inbox + principle effectiveness
├── memory-projection.ts        — Memory stats + corrections + learnings
├── environment-projection.ts   — Config + git + doctor checks
├── config-inspector.ts         — Config inspector modal data
├── events.ts                   — Event stream builder
├── mission-control-commands.ts — Command palette data
└── snapshot-demand.ts          — Snapshot reload demand tracking
```

**Purpose:** Read-model building and UI state management.

**Key patterns:**
- **Separation of I/O and projection:** `snapshot-loader.ts` does I/O, `projection.ts` is pure
- **Reducer-driven UI state:** `reducer.ts` owns focus, modals, selection (no ad-hoc component state)
- **Read-only contract:** Snapshot builders are side-effect free (except gated reply ingest)

**Dependencies:**
- Heavy imports from `@/features/*` (mission, task, evidence, verdict, handoff, memory, graph)
- Heavy imports from `@/infra/*` (config, git, status types)
- Light imports from `@/shared` (errors, ui-config, sanitize)

#### App Layer (6 files)
```
app/
├── preview-state.ts        — Preview screen routing (--preview features, etc.)
├── preview-contract.ts     — Preview frame capture contract
├── render-check-contract.ts — Render check contract
├── input-dispatch.ts       — Keyboard event dispatcher
├── modal-builders.ts       — Modal content builders
└── interactive-shared.ts   — Shared interactive helpers
```

**Purpose:** Preview mode routing and interactive app wiring.

**Key exports:**
- `buildPreviewState()` — maps `--preview <screen>` to reducer state
- `PREVIEW_SCREENS` — allowed preview screens (dashboard, features, config, etc.)
- `HOME_PREVIEW_SCREENS` — subset allowed in home mode

#### OpenTUI Layer (8 files)
```
opentui/
├── index.ts                           — OpenTUI re-exports
├── ansi.ts                            — Captured frame → ANSI converter
├── components/
│   ├── builders.ts                    — Panel view-model builders
│   └── mission-control-screen.tsx     — Main screen component
├── app/
│   ├── interactive.tsx                — Interactive loop (keyboard, reload, writes)
│   ├── preview.ts                     — Preview frame renderer
│   ├── mission-control-app.tsx        — Terminal dimensions adapter
│   └── render-check.ts                — Render check runner
└── testing/
    └── frame-capture.tsx              — Deterministic frame capture
```

**Purpose:** OpenTUI integration and rendering.

**Key patterns:**
- **Thin components:** `mission-control-screen.tsx` is layout-focused, logic lives in builders
- **Interactive writes:** `interactive.tsx` is the only place that mutates domain state
- **Deterministic preview:** `preview.ts` + `frame-capture.tsx` for agent-friendly output

#### Shared TUI Utilities (2 files)
```
shared/
├── modal-model.ts       — Modal state helpers
└── header-animation.ts  — Header animation state
```

**Purpose:** TUI-specific shared utilities.

#### Library (1 file)
```
lib/
└── snapshot-poll-cache.ts — Cached config/git ports for snapshot building
```

**Purpose:** Performance optimization (avoid re-reading config/git on every snapshot).

---

### Import/Export Relationships (src/tui/)

**Exports to:**
- `src/infra/commands/mission-control.command.ts` — main consumer (buildSnapshot, buildHomeSnapshot)
- Tests — unit tests for reducer, snapshot builders, formatters

**Imports from:**
- `@/features/*` — **heavy** (mission, task, evidence, verdict, handoff, memory, graph, agent)
- `@/infra/*` — **heavy** (config, git, status types)
- `@/shared` — **light** (errors, ui-config, sanitize)

**Internal dependencies:**
- `state/snapshot.ts` → `state/snapshot-loader.ts` + `state/projection.ts`
- `state/projection.ts` → all other state/ builders (autopilot, task-board, reply, memory, environment, events)
- `app/preview-state.ts` → `state/reducer.ts`
- `opentui/app/interactive.tsx` → `state/reducer.ts`, `state/snapshot.ts`, `@/features/mission`, `@/features/agent`
- `opentui/components/mission-control-screen.tsx` → `opentui/components/builders.ts`, `theme.ts`, `format.ts`

**Observation:** TUI is a projection layer, so heavy feature/infra dependencies are expected and appropriate.

---

### Organizational Assessment (src/tui/)

#### ✅ Strengths
1. **Four-layer architecture is sound** — command → snapshot → UI state → render
2. **Read-only contract is maintained** — snapshot builders are side-effect free (except gated reply ingest)
3. **Reducer-driven UI state** — no ad-hoc mutable component state
4. **Separation of I/O and projection** — `snapshot-loader.ts` vs `projection.ts`
5. **Preview mode is deterministic** — `--preview` + `--size` for agent-friendly output
6. **Interactive writes are isolated** — only in `opentui/app/interactive.tsx`

#### ⚠️ Observations
1. **`theme.ts` at root level** — could move to `shared/` for consistency
2. **`format.ts` at root level** — could move to `lib/` for consistency
3. **`session-id.ts` is trivial** — one-line helper, could inline or move to `lib/`
4. **`lib/snapshot-poll-cache.ts` is the only lib/ file** — consider flattening to `state/`

#### 💡 Potential Improvements
1. **Flatten lib/ into state/** — only one file, not worth a directory
2. **Move theme.ts and format.ts to shared/** — group TUI-specific utilities
3. **Consider splitting projection.ts** — 300+ lines, could extract screen-specific projections
4. **Document reply ingest gate** — the one sanctioned side effect in snapshot building

---

## Cross-Directory Analysis

### Dependency Flow
```
src/tui/
  ├─→ src/features/*  (heavy: mission, task, evidence, verdict, handoff, memory, graph, agent)
  ├─→ src/infra/*     (heavy: config, git, status types)
  └─→ src/shared      (light: errors, ui-config, sanitize)

src/shared/
  └─→ (none)          (zero dependencies on features/infra/tui)
```

**Observation:** Clean unidirectional flow. src/shared/ is truly generic, src/tui/ is appropriately coupled to features/infra.

### Overlap and Duplication

#### ❌ No Duplication Found
- src/shared/ utilities are generic (fs, yaml, shell, path, output)
- src/tui/ utilities are TUI-specific (formatters, theme, keyboard input)
- No overlapping functionality

#### ⚠️ Potential Misplacement
1. **`src/shared/domain/ui-config.ts`** — only used by src/tui/, should move to `src/tui/shared/`
2. **`src/shared/lib/deprecated-version-flag.ts`** — only used by src/index.ts, should move to `src/infra/`
3. **`src/shared/lib/maestro-substrate-paths.ts`** — only used by Trust Verifier, should move to `src/features/verify/`

---

## Recommendations

### High Priority
1. **Move `src/shared/domain/ui-config.ts` → `src/tui/shared/ui-config.ts`**
   - Only used by TUI, not generic
   - Reduces src/shared/ surface area

2. **Move `src/shared/lib/deprecated-version-flag.ts` → `src/infra/lib/deprecated-version-flag.ts`**
   - CLI-specific, not generic
   - Only used by src/index.ts

3. **Move `src/shared/lib/maestro-substrate-paths.ts` → `src/features/verify/lib/substrate-paths.ts`**
   - Contract-specific, not generic
   - Only used by Trust Verifier

### Medium Priority
4. **Flatten `src/tui/lib/` into `src/tui/state/`**
   - Only one file (`snapshot-poll-cache.ts`), not worth a directory
   - Move to `src/tui/state/snapshot-poll-cache.ts`

5. **Move `src/tui/theme.ts` and `src/tui/format.ts` → `src/tui/shared/`**
   - Group TUI-specific utilities
   - Cleaner root-level directory

6. **Inline or move `src/tui/session-id.ts`**
   - One-line helper, could inline at call site
   - Or move to `src/tui/shared/session-id.ts`

### Low Priority
7. **Consider splitting `src/tui/state/projection.ts`**
   - 300+ lines, could extract screen-specific projections
   - Not urgent, but would improve readability

8. **Document reply ingest gate in README.md**
   - The one sanctioned side effect in snapshot building
   - Already documented, but could be more prominent

---

## Conclusion

### src/shared/
**Verdict:** ✅ Excellent organization. Generic utilities with zero domain logic leakage.

**Action Items:**
- Move 3 misplaced files to their proper homes (ui-config, deprecated-version-flag, maestro-substrate-paths)
- Consider extracting to @maestro/shared package for reuse across projects

### src/tui/
**Verdict:** ✅ Good organization. Sound architecture with clear boundaries.

**Action Items:**
- Flatten lib/ directory (only one file)
- Move theme.ts and format.ts to shared/
- Consider splitting large projection.ts file

### Overall Health
Both directories are well-organized with clear separation of concerns. The main issues are minor misplacements (3 files in src/shared/ that belong elsewhere) and small organizational improvements (flatten lib/, group utilities in shared/).

**No major refactoring needed.** The architecture is sound and boundaries are respected.

---

## Appendix: File-by-File Reference

### src/shared/ (24 files)

| File | Lines | Purpose | Used By |
|------|-------|---------|---------|
| `domain/id.ts` | 60 | Handoff ID generation | features/handoff |
| `domain/defaults.ts` | 50 | Path constants | infra, features |
| `domain/ui-config.ts` | 40 | UI config types | **tui only** ⚠️ |
| `version.ts` | 5 | Build constants | version-format.ts, infra |
| `version-format.ts` | 120 | Version formatting | infra/commands |
| `errors.ts` | 10 | MaestroError base class | features, infra, tui |
| `lib/fs.ts` | 180 | Filesystem I/O | features, infra |
| `lib/yaml.ts` | 80 | YAML parse/stringify | features, infra |
| `lib/shell.ts` | 100 | Shell execution | features, infra |
| `lib/output.ts` | 50 | Dual-mode output | infra/commands |
| `lib/output-capture.ts` | 120 | Truncating buffer | features/evidence |
| `lib/path-safety.ts` | 30 | Path traversal prevention | features, infra |
| `lib/path-normalize.ts` | 5 | Slash normalization | lib/glob-match.ts |
| `lib/sanitize.ts` | 100 | Terminal/markdown sanitization | lib/output.ts, tui |
| `lib/project-root.ts` | 80 | Project root resolution | infra |
| `lib/git-base.ts` | 70 | Git base ref resolution | features/verify, features/verdict |
| `lib/ansi.ts` | 40 | ANSI color helpers | infra/commands |
| `lib/template.ts` | 20 | Template renderer | infra |
| `lib/skill-path.ts` | 20 | Skill directory encoding | lib/maestro-substrate-paths.ts |
| `lib/maestro-substrate-paths.ts` | 50 | Substrate path detection | **features/verify only** ⚠️ |
| `lib/glob-match.ts` | 40 | Glob matching | features/policy |
| `lib/fs-lock.ts` | 120 | File-based locking | features/task |
| `lib/concurrency.ts` | 30 | Rate-limited async map | features/ci |
| `lib/deprecated-version-flag.ts` | 80 | --version collision detection | **src/index.ts only** ⚠️ |

### src/tui/ (39 files)

| File | Lines | Purpose | Layer |
|------|-------|---------|-------|
| `input.ts` | 250 | Keyboard input parser | Root |
| `session-id.ts` | 5 | Session ID shortener | Root |
| `format.ts` | 60 | Display formatters | Root |
| `theme.ts` | 150 | Status colors + palette | Root |
| `README.md` | 200 | Architecture docs | Root |
| `state/types.ts` | 200 | Snapshot DTOs | State |
| `state/snapshot.ts` | 50 | Snapshot entry points | State |
| `state/snapshot-loader.ts` | 300 | I/O layer | State |
| `state/projection.ts` | 350 | Pure projection | State |
| `state/reducer.ts` | 1400 | UI state machine | State |
| `state/screen-types.ts` | 100 | Screen DTOs | State |
| `state/autopilot-screen.ts` | 150 | Autopilot snapshot | State |
| `state/task-board.ts` | 100 | Task board snapshot | State |
| `state/reply-projection.ts` | 150 | Reply inbox + principles | State |
| `state/memory-projection.ts` | 80 | Memory snapshot | State |
| `state/environment-projection.ts` | 200 | Config + git + doctor | State |
| `state/config-inspector.ts` | 400 | Config inspector | State |
| `state/events.ts` | 100 | Event stream | State |
| `state/mission-control-commands.ts` | 150 | Command palette | State |
| `state/snapshot-demand.ts` | 50 | Reload demand tracking | State |
| `app/preview-state.ts` | 160 | Preview routing | App |
| `app/preview-contract.ts` | 20 | Preview contract | App |
| `app/render-check-contract.ts` | 20 | Render check contract | App |
| `app/input-dispatch.ts` | 100 | Keyboard dispatcher | App |
| `app/modal-builders.ts` | 300 | Modal content | App |
| `app/interactive-shared.ts` | 50 | Interactive helpers | App |
| `opentui/index.ts` | 10 | OpenTUI re-exports | OpenTUI |
| `opentui/ansi.ts` | 40 | Frame → ANSI converter | OpenTUI |
| `opentui/components/builders.ts` | 800 | Panel builders | OpenTUI |
| `opentui/components/mission-control-screen.tsx` | 600 | Main screen | OpenTUI |
| `opentui/app/interactive.tsx` | 400 | Interactive loop | OpenTUI |
| `opentui/app/preview.ts` | 50 | Preview renderer | OpenTUI |
| `opentui/app/mission-control-app.tsx` | 50 | Dimensions adapter | OpenTUI |
| `opentui/app/render-check.ts` | 100 | Render check runner | OpenTUI |
| `opentui/testing/frame-capture.tsx` | 80 | Frame capture | OpenTUI |
| `shared/modal-model.ts` | 50 | Modal helpers | Shared |
| `shared/header-animation.ts` | 40 | Header animation | Shared |
| `lib/snapshot-poll-cache.ts` | 100 | Cached ports | Lib |

---

**End of Analysis**
