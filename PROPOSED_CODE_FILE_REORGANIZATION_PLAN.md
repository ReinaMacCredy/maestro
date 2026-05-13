# Maestro Code Reorganization Plan

**Generated:** 2026-05-08  
**Repository:** /Users/reinamaccredy/Code/maestro  
**Analysis Sources:** FEATURES_ANALYSIS.md, INFRA_ANALYSIS.md, SHARED_TUI_ANALYSIS.md, IMPORTS_ANALYSIS.md

---

## 1. Executive Summary

### Overall Assessment

The Maestro codebase demonstrates **excellent organizational health**. The feature-first architecture with hexagonal patterns, clear boundaries, and consistent structure is working well. This is a **minor improvements** situation, not a major refactor.

**Key Findings:**
- ✅ 31 features follow consistent hexagonal architecture (commands/usecases/domain/ports/adapters)
- ✅ No circular dependencies detected
- ✅ No feature boundary violations
- ✅ Clean hub-and-spoke import structure
- ✅ TUI properly isolated as projection layer
- ✅ Infrastructure layer well-separated
- ⚠️ **3 files misplaced in src/shared/** (belong elsewhere)
- ⚠️ Minor organizational improvements in src/tui/
- ⚠️ Potential for consolidating 3 minimal features

**Organizational Health Score: 9.2/10**

### High-Level Recommendation

**Minor improvements only.** Focus on:
1. Moving 3 misplaced files from src/shared/ to their proper homes
2. Flattening src/tui/lib/ (only 1 file)
3. Grouping TUI utilities in src/tui/shared/
4. Considering consolidation of 3 minimal features (optional)

**Do NOT:**
- Restructure features/ (all 31 features are well-organized)
- Change the hexagonal architecture pattern
- Merge or split large features (task, mission, memory are appropriately sized)
- Reorganize infra/ (already follows clear patterns)

---

## 2. Current State Assessment

### What's Working Well (Strengths)

#### Architecture
1. **Feature-first organization** - 31 bounded contexts with clear responsibilities
2. **Hexagonal architecture** - Consistent ports/adapters pattern across all features
3. **Clean boundaries** - Cross-feature imports only through public index.ts surfaces
4. **No circular dependencies** - Hub-and-spoke import structure
5. **Separation of concerns** - features/ (domain), infra/ (plumbing), shared/ (utilities), tui/ (projection)

#### Code Quality
1. **Consistent naming** - All commands end with .command.ts, use cases with .usecase.ts, etc.
2. **Type safety** - Port interfaces for all external dependencies
3. **Testability** - Port/adapter pattern enables easy mocking
4. **Progressive disclosure** - Small features for simple concerns, large for complex domains

#### Import Structure
1. **Hub features** - evidence, task, policy, verdict, spec are appropriately central
2. **Leaf features** - gc, ralph, recover, review, intake are appropriately peripheral
3. **Shared utilities** - Generic, reusable, zero domain logic leakage
4. **TUI isolation** - Only mission-control command imports from TUI

### What Needs Improvement (Issues)

#### Misplaced Files (3 files)
1. **src/shared/domain/ui-config.ts** - Only used by TUI, not generic
2. **src/shared/lib/deprecated-version-flag.ts** - Only used by src/index.ts, CLI-specific
3. **src/shared/lib/maestro-substrate-paths.ts** - Only used by Trust Verifier, contract-specific

#### Minor Organizational Issues
1. **src/tui/lib/** - Only contains 1 file (snapshot-poll-cache.ts), not worth a directory
2. **src/tui/theme.ts and format.ts** - At root level, should be in shared/
3. **src/tui/session-id.ts** - Trivial one-line helper, could inline or move to shared/

#### Potential Feature Consolidations (Optional)
1. **review** (2 files) - Could merge into evidence (only records review-ack evidence)
2. **agent** (2 files) - Could merge into mission (generates prompts from mission context)
3. **skills** (2 files) - Could merge into infra (thin wrapper around infra logic)

---

## 3. Proposed Changes

### A. Critical Misplacements (High Priority)

#### Change 1: Move ui-config.ts to TUI
- **Priority:** High
- **Type:** Move file
- **Current:** `src/shared/domain/ui-config.ts`
- **Proposed:** `src/tui/shared/ui-config.ts`
- **Rationale:** Only used by TUI layer; not a generic shared utility
- **Impact:** Update 2-3 imports in src/tui/
- **Risk:** Low (isolated to TUI)

#### Change 2: Move deprecated-version-flag.ts to infra
- **Priority:** High
- **Type:** Move file
- **Current:** `src/shared/lib/deprecated-version-flag.ts`
- **Proposed:** `src/infra/lib/deprecated-version-flag.ts`
- **Rationale:** CLI-specific logic, not generic utility
- **Impact:** Update 1 import in src/index.ts
- **Risk:** Low (single import site)

#### Change 3: Move maestro-substrate-paths.ts to verify feature
- **Priority:** High
- **Type:** Move file
- **Current:** `src/shared/lib/maestro-substrate-paths.ts`
- **Proposed:** `src/features/verify/lib/substrate-paths.ts`
- **Rationale:** Contract-specific logic, only used by Trust Verifier
- **Impact:** Update 1-2 imports in src/features/verify/
- **Risk:** Low (isolated to verify feature)

### B. Structural Improvements (Medium Priority)

#### Change 4: Flatten src/tui/lib/ directory
- **Priority:** Medium
- **Type:** Restructure directory
- **Current:** `src/tui/lib/snapshot-poll-cache.ts` (only file in lib/)
- **Proposed:** `src/tui/state/snapshot-poll-cache.ts`
- **Rationale:** Only 1 file in lib/, not worth a directory; logically belongs with state/
- **Impact:** Update 2-3 imports in src/tui/state/ and src/infra/commands/
- **Risk:** Low (internal TUI reorganization)

#### Change 5: Group TUI utilities in shared/
- **Priority:** Medium
- **Type:** Move files
- **Current:** `src/tui/theme.ts`, `src/tui/format.ts` at root level
- **Proposed:** `src/tui/shared/theme.ts`, `src/tui/shared/format.ts`
- **Rationale:** Cleaner root-level directory; groups TUI-specific utilities
- **Impact:** Update 5-10 imports across src/tui/
- **Risk:** Low (internal TUI reorganization)

#### Change 6: Move or inline session-id.ts
- **Priority:** Medium
- **Type:** Move file or inline
- **Current:** `src/tui/session-id.ts` (one-line helper)
- **Proposed:** Either inline at call sites OR move to `src/tui/shared/session-id.ts`
- **Rationale:** Trivial helper; either inline or group with utilities
- **Impact:** Update 1-2 call sites if inlined, or 1-2 imports if moved
- **Risk:** Low (trivial change)

### C. Feature Consolidations (Low Priority, Optional)

#### Change 7: Consider merging review into evidence
- **Priority:** Low
- **Type:** Merge features
- **Current:** `src/features/review/` (2 files: command + index)
- **Proposed:** Merge into `src/features/evidence/commands/review-ack.command.ts`
- **Rationale:** Review feature only records review-ack evidence; no unique domain logic
- **Impact:** Update imports in src/index.ts and any feature that imports review
- **Risk:** Low (minimal feature)
- **Decision:** Optional - only if no additional review functionality is planned

#### Change 8: Consider merging agent into mission
- **Priority:** Low
- **Type:** Merge features
- **Current:** `src/features/agent/` (2 files: use case + index)
- **Proposed:** Merge into `src/features/mission/usecases/generate-agent-prompt.ts`
- **Rationale:** Agent feature only generates prompts from mission context
- **Impact:** Update imports in src/index.ts, handoff, bundle
- **Risk:** Low (minimal feature)
- **Decision:** Optional - only if agent won't grow beyond prompt generation

#### Change 9: Consider merging skills into infra
- **Priority:** Low
- **Type:** Merge features
- **Current:** `src/features/skills/` (2 files: command + index)
- **Proposed:** Merge into `src/infra/commands/skills.command.ts`
- **Rationale:** Skills feature is a thin wrapper around infra logic
- **Impact:** Update imports in src/index.ts
- **Risk:** Low (minimal feature)
- **Decision:** Optional - current separation is acceptable

### D. No Changes Needed

#### Features (31 features)
- **Decision:** Keep all 31 features as-is
- **Rationale:** All follow consistent hexagonal architecture; boundaries are clear
- **Note:** Even large features (task: 76 files, mission: 47 files) are appropriately sized for their domain complexity

#### Infrastructure (src/infra/)
- **Decision:** Keep current structure
- **Rationale:** Clear separation of commands/usecases/adapters/ports/domain; follows hexagonal pattern

#### Shared utilities (src/shared/)
- **Decision:** Keep current structure (after moving 3 misplaced files)
- **Rationale:** Generic utilities with zero domain logic leakage

---

## 4. Detailed Rationale

### Why These Changes Improve the Codebase

#### Misplaced File Moves (Changes 1-3)
**Problem:** Three files in src/shared/ are not generic utilities:
- ui-config.ts is TUI-specific
- deprecated-version-flag.ts is CLI-specific
- maestro-substrate-paths.ts is Trust Verifier-specific

**Benefit:** 
- Reduces src/shared/ surface area to truly generic utilities
- Makes it easier to extract @maestro/shared as a reusable package
- Improves discoverability (TUI utilities in TUI, verify utilities in verify)
- Follows "put code where it's used" principle

**Developer Impact:**
- Newcomers won't be confused by TUI-specific code in shared/
- Agents searching for TUI utilities will find them in src/tui/
- Trust Verifier logic is co-located with other verify code

#### TUI Structural Improvements (Changes 4-6)
**Problem:** Minor organizational inconsistencies in src/tui/:
- lib/ directory with only 1 file
- theme.ts and format.ts at root level instead of grouped
- Trivial one-line helper at root level

**Benefit:**
- Cleaner root-level directory structure
- Consistent grouping of utilities in shared/
- Reduces cognitive load when navigating src/tui/

**Developer Impact:**
- Easier to find TUI utilities (all in shared/)
- Less clutter at root level
- More intuitive directory structure

#### Feature Consolidations (Changes 7-9)
**Problem:** Three features with only 2 files each that don't have unique domain logic

**Benefit:**
- Reduces feature count from 31 to 28 (if all consolidated)
- Simplifies mental model (fewer top-level directories)
- Co-locates related functionality

**Developer Impact:**
- Fewer directories to navigate
- Related code is closer together
- Still maintains clear boundaries (just within larger features)

**Trade-off:** Loses explicit feature separation; acceptable for minimal features

### How It Aids Navigation

#### For Human Developers
1. **Clearer src/shared/** - Only truly generic utilities remain
2. **Intuitive TUI structure** - All utilities grouped in shared/
3. **Co-located logic** - Verify utilities in verify/, TUI utilities in TUI/
4. **Fewer top-level features** - Easier to scan src/features/ directory

#### For AI Agents
1. **Predictable locations** - "TUI utilities are in src/tui/shared/"
2. **Reduced search space** - Fewer directories to explore
3. **Clear boundaries** - Generic vs. specific utilities are obvious
4. **Consistent patterns** - All features follow same structure

### How It Improves Maintainability

#### Reduces Coupling
- Moving TUI-specific code out of shared/ reduces false dependencies
- Co-locating verify utilities with verify feature reduces cross-directory coupling

#### Improves Cohesion
- TUI utilities grouped together (high cohesion)
- Verify utilities grouped together (high cohesion)
- Minimal features merged into related features (higher cohesion)

#### Simplifies Testing
- TUI utilities can be tested alongside TUI code
- Verify utilities can be tested alongside verify code
- Fewer feature boundaries to mock

#### Enables Future Extraction
- Clean src/shared/ can be extracted to @maestro/shared package
- TUI can be extracted to @maestro/tui package
- Features remain self-contained and extractable

---

## 5. Migration Strategy

### Step-by-Step Execution Plan

#### Phase 1: Misplaced File Moves (High Priority)
**Estimated Time:** 30 minutes  
**Risk:** Low  
**Order:** Can be done in parallel or any order

**Step 1.1: Move ui-config.ts**
```bash
# 1. Create target directory if needed
mkdir -p src/tui/shared

# 2. Move file
git mv src/shared/domain/ui-config.ts src/tui/shared/ui-config.ts

# 3. Update imports in src/tui/ files
# Find: @/shared/domain/ui-config
# Replace: @/tui/shared/ui-config

# 4. Run typecheck
bun run typecheck

# 5. Commit
git commit -m "refactor(tui): move ui-config to tui/shared (TUI-specific)"
```

**Step 1.2: Move deprecated-version-flag.ts**
```bash
# 1. Create target directory if needed
mkdir -p src/infra/lib

# 2. Move file
git mv src/shared/lib/deprecated-version-flag.ts src/infra/lib/deprecated-version-flag.ts

# 3. Update import in src/index.ts
# Find: @/shared/lib/deprecated-version-flag
# Replace: @/infra/lib/deprecated-version-flag

# 4. Run typecheck
bun run typecheck

# 5. Commit
git commit -m "refactor(infra): move deprecated-version-flag to infra/lib (CLI-specific)"
```

**Step 1.3: Move maestro-substrate-paths.ts**
```bash
# 1. Create target directory if needed
mkdir -p src/features/verify/lib

# 2. Move file (and its dependency skill-path.ts)
git mv src/shared/lib/maestro-substrate-paths.ts src/features/verify/lib/substrate-paths.ts
# Note: skill-path.ts is only used by maestro-substrate-paths.ts
git mv src/shared/lib/skill-path.ts src/features/verify/lib/skill-path.ts

# 3. Update imports in src/features/verify/
# Find: @/shared/lib/maestro-substrate-paths
# Replace: @/features/verify/lib/substrate-paths
# Find: @/shared/lib/skill-path
# Replace: @/features/verify/lib/skill-path

# 4. Run typecheck
bun run typecheck

# 5. Commit
git commit -m "refactor(verify): move substrate-paths to verify/lib (verify-specific)"
```

#### Phase 2: TUI Structural Improvements (Medium Priority)
**Estimated Time:** 20 minutes  
**Risk:** Low  
**Order:** Do after Phase 1

**Step 2.1: Flatten src/tui/lib/**
```bash
# 1. Move file to state/
git mv src/tui/lib/snapshot-poll-cache.ts src/tui/state/snapshot-poll-cache.ts

# 2. Remove empty lib/ directory
rmdir src/tui/lib

# 3. Update imports
# Find: @/tui/lib/snapshot-poll-cache
# Replace: @/tui/state/snapshot-poll-cache

# 4. Run typecheck
bun run typecheck

# 5. Commit
git commit -m "refactor(tui): flatten lib/ into state/ (only 1 file)"
```

**Step 2.2: Group TUI utilities in shared/**
```bash
# 1. Move files
git mv src/tui/theme.ts src/tui/shared/theme.ts
git mv src/tui/format.ts src/tui/shared/format.ts

# 2. Update imports across src/tui/
# Find: @/tui/theme
# Replace: @/tui/shared/theme
# Find: @/tui/format
# Replace: @/tui/shared/format

# 3. Run typecheck
bun run typecheck

# 4. Commit
git commit -m "refactor(tui): group theme and format in shared/"
```

**Step 2.3: Move session-id.ts (optional)**
```bash
# Option A: Inline (if only 1-2 call sites)
# - Copy function to call sites
# - Remove src/tui/session-id.ts

# Option B: Move to shared/
git mv src/tui/session-id.ts src/tui/shared/session-id.ts

# Update imports
# Find: @/tui/session-id
# Replace: @/tui/shared/session-id

# Run typecheck
bun run typecheck

# Commit
git commit -m "refactor(tui): move session-id to shared/"
```

#### Phase 3: Feature Consolidations (Low Priority, Optional)
**Estimated Time:** 1 hour  
**Risk:** Low  
**Order:** Do after Phase 1 and 2, only if desired

**Step 3.1: Merge review into evidence (optional)**
```bash
# 1. Move command file
git mv src/features/review/commands/review-ack.command.ts \
       src/features/evidence/commands/review-ack.command.ts

# 2. Update evidence/index.ts to export registerReviewAckCommand
# Add: export { registerReviewAckCommand } from "./commands/review-ack.command.js";

# 3. Update src/index.ts
# Find: import { registerReviewCommand } from "./features/review/index.js";
# Replace: import { registerReviewAckCommand } from "./features/evidence/index.js";

# 4. Remove empty review/ directory
rm -rf src/features/review

# 5. Run typecheck
bun run typecheck

# 6. Commit
git commit -m "refactor(features): merge review into evidence (minimal feature)"
```

**Step 3.2: Merge agent into mission (optional)**
```bash
# 1. Move use case file
git mv src/features/agent/usecases/generate-agent-prompt.ts \
       src/features/mission/usecases/generate-agent-prompt.ts

# 2. Update mission/index.ts to export generateAgentPrompt
# Add: export { generateAgentPrompt } from "./usecases/generate-agent-prompt.js";

# 3. Update imports in handoff, bundle, etc.
# Find: @/features/agent
# Replace: @/features/mission

# 4. Remove empty agent/ directory
rm -rf src/features/agent

# 5. Run typecheck
bun run typecheck

# 6. Commit
git commit -m "refactor(features): merge agent into mission (minimal feature)"
```

**Step 3.3: Merge skills into infra (optional)**
```bash
# 1. Move command file
git mv src/features/skills/commands/skills.command.ts \
       src/infra/commands/skills.command.ts

# 2. Update src/index.ts
# Find: import { registerSkillsCommand } from "./features/skills/index.js";
# Replace: import { registerSkillsCommand } from "./infra/commands/skills.command.js";

# 3. Remove empty skills/ directory
rm -rf src/features/skills

# 4. Run typecheck
bun run typecheck

# 5. Commit
git commit -m "refactor(infra): merge skills into infra commands (minimal feature)"
```

### Order of Operations

**Recommended sequence:**
1. Phase 1 (all 3 changes) - Can be done in parallel or any order
2. Phase 2 (all 3 changes) - Do after Phase 1
3. Phase 3 (optional) - Only if desired, do after Phase 1 and 2

**Rationale:**
- Phase 1 fixes misplacements (highest value)
- Phase 2 improves TUI structure (medium value)
- Phase 3 is optional consolidation (low value, but cleaner)

### How to Avoid Breaking Changes

#### Use git mv
- Always use `git mv` instead of manual move + add
- Preserves file history
- Safer for git operations

#### Update imports immediately
- After each move, update all imports before committing
- Use global search/replace in editor
- Verify with `bun run typecheck`

#### Commit atomically
- One logical change per commit
- Makes rollback easier if needed
- Clear commit messages

#### Run verification after each step
```bash
# After each change:
bun run typecheck          # Type safety
bun run check:boundaries   # Feature boundaries
bun run test               # Unit tests
./dist/maestro --version   # CLI still works
```

### Testing Strategy After Each Change

#### After Phase 1 (Misplaced File Moves)
```bash
# 1. Typecheck
bun run typecheck

# 2. Build
bun run build

# 3. Feature boundary check
bun run check:boundaries

# 4. Run tests
bun test

# 5. Verify CLI
./dist/maestro --version
./dist/maestro mission-control --json
./dist/maestro task verify --task <test-id>

# 6. Check imports
# Verify no remaining imports from old locations:
rg "@/shared/domain/ui-config" src/
rg "@/shared/lib/deprecated-version-flag" src/
rg "@/shared/lib/maestro-substrate-paths" src/
# Should return no results
```

#### After Phase 2 (TUI Improvements)
```bash
# 1. Typecheck
bun run typecheck

# 2. Build
bun run build

# 3. Run TUI tests
bun test tests/unit/tui/

# 4. Verify Mission Control
./dist/maestro mission-control --json
./dist/maestro mission-control --preview dashboard --size 120x40
./dist/maestro mission-control --render-check

# 5. Check imports
rg "@/tui/lib/snapshot-poll-cache" src/
rg "@/tui/theme" src/ | grep -v "shared/theme"
rg "@/tui/format" src/ | grep -v "shared/format"
# Should return no results
```

#### After Phase 3 (Feature Consolidations)
```bash
# 1. Typecheck
bun run typecheck

# 2. Build
bun run build

# 3. Feature boundary check
bun run check:boundaries

# 4. Run full test suite
bun test

# 5. Verify affected commands
./dist/maestro review ack --help
./dist/maestro skills --help

# 6. Check for orphaned directories
ls src/features/review 2>/dev/null && echo "ERROR: review/ still exists"
ls src/features/agent 2>/dev/null && echo "ERROR: agent/ still exists"
ls src/features/skills 2>/dev/null && echo "ERROR: skills/ still exists"
```

---

## 6. Import Update Tracking

### Phase 1: Misplaced File Moves

#### Change 1: ui-config.ts
**Files that import from old location:**
```
src/tui/state/reducer.ts
src/tui/state/environment-projection.ts
src/shared/domain/defaults.ts (possibly)
```

**Import statement changes:**
```typescript
// OLD:
import { getMissionControlBackgroundMode } from "@/shared/domain/ui-config.js";

// NEW:
import { getMissionControlBackgroundMode } from "@/tui/shared/ui-config.js";
```

**Automated update:**
```bash
find src/tui -name "*.ts" -exec sed -i '' \
  's|@/shared/domain/ui-config|@/tui/shared/ui-config|g' {} +
```

#### Change 2: deprecated-version-flag.ts
**Files that import from old location:**
```
src/index.ts
```

**Import statement changes:**
```typescript
// OLD:
import { assertNoDeprecatedVersionFlag } from "@/shared/lib/deprecated-version-flag.js";

// NEW:
import { assertNoDeprecatedVersionFlag } from "@/infra/lib/deprecated-version-flag.js";
```

**Automated update:**
```bash
sed -i '' 's|@/shared/lib/deprecated-version-flag|@/infra/lib/deprecated-version-flag|g' \
  src/index.ts
```

#### Change 3: maestro-substrate-paths.ts
**Files that import from old location:**
```
src/features/verify/usecases/checks/check-generated-files.ts
src/features/verify/usecases/checks/check-cross-imports.ts
```

**Import statement changes:**
```typescript
// OLD:
import { isMaestroSubstratePath } from "@/shared/lib/maestro-substrate-paths.js";

// NEW:
import { isMaestroSubstratePath } from "@/features/verify/lib/substrate-paths.js";
```

**Automated update:**
```bash
find src/features/verify -name "*.ts" -exec sed -i '' \
  's|@/shared/lib/maestro-substrate-paths|@/features/verify/lib/substrate-paths|g' {} +
find src/features/verify -name "*.ts" -exec sed -i '' \
  's|@/shared/lib/skill-path|@/features/verify/lib/skill-path|g' {} +
```

### Phase 2: TUI Structural Improvements

#### Change 4: snapshot-poll-cache.ts
**Files that import from old location:**
```
src/tui/state/snapshot-loader.ts
src/infra/commands/mission-control.command.ts
```

**Import statement changes:**
```typescript
// OLD:
import { CachingGitPort, CachingConfigPort } from "@/tui/lib/snapshot-poll-cache.js";

// NEW:
import { CachingGitPort, CachingConfigPort } from "@/tui/state/snapshot-poll-cache.js";
```

**Automated update:**
```bash
find src -name "*.ts" -exec sed -i '' \
  's|@/tui/lib/snapshot-poll-cache|@/tui/state/snapshot-poll-cache|g' {} +
```

#### Change 5: theme.ts and format.ts
**Files that import from old location:**
```
src/tui/opentui/components/builders.ts
src/tui/opentui/components/mission-control-screen.tsx
src/tui/state/projection.ts
src/tui/state/task-board.ts
src/tui/state/autopilot-screen.ts
(~10 files total)
```

**Import statement changes:**
```typescript
// OLD:
import { PALETTE, MISSION_STATUS_COLOR } from "@/tui/theme.js";
import { formatElapsed, formatTokens } from "@/tui/format.js";

// NEW:
import { PALETTE, MISSION_STATUS_COLOR } from "@/tui/shared/theme.js";
import { formatElapsed, formatTokens } from "@/tui/shared/format.js";
```

**Automated update:**
```bash
find src/tui -name "*.ts" -name "*.tsx" -exec sed -i '' \
  's|@/tui/theme|@/tui/shared/theme|g' {} +
find src/tui -name "*.ts" -name "*.tsx" -exec sed -i '' \
  's|@/tui/format|@/tui/shared/format|g' {} +
```

#### Change 6: session-id.ts
**Files that import from old location:**
```
src/tui/state/projection.ts (possibly)
```

**Import statement changes:**
```typescript
// OLD:
import { shortenSessionId } from "@/tui/session-id.js";

// NEW (if moved):
import { shortenSessionId } from "@/tui/shared/session-id.js";

// NEW (if inlined):
// Just copy the function directly
```

**Automated update:**
```bash
find src/tui -name "*.ts" -exec sed -i '' \
  's|@/tui/session-id|@/tui/shared/session-id|g' {} +
```

### Phase 3: Feature Consolidations

#### Change 7: review → evidence
**Files that import from old location:**
```
src/index.ts
```

**Import statement changes:**
```typescript
// OLD:
import { registerReviewCommand } from "./features/review/index.js";

// NEW:
import { registerReviewAckCommand } from "./features/evidence/index.js";
```

**Manual update required** (only 1 file)

#### Change 8: agent → mission
**Files that import from old location:**
```
src/index.ts
src/features/handoff/usecases/build-handoff-prompt.ts
src/features/bundle/usecases/collect-sources.ts
```

**Import statement changes:**
```typescript
// OLD:
import { generateAgentPrompt } from "@/features/agent/index.js";

// NEW:
import { generateAgentPrompt } from "@/features/mission/index.js";
```

**Automated update:**
```bash
find src -name "*.ts" -exec sed -i '' \
  's|@/features/agent|@/features/mission|g' {} +
```

#### Change 9: skills → infra
**Files that import from old location:**
```
src/index.ts
```

**Import statement changes:**
```typescript
// OLD:
import { registerSkillsCommand } from "./features/skills/index.js";

// NEW:
import { registerSkillsCommand } from "./infra/commands/skills.command.js";
```

**Manual update required** (only 1 file)

### Summary of Import Updates

| Change | Files Affected | Automated? | Complexity |
|--------|----------------|------------|------------|
| ui-config.ts | 2-3 | Yes | Low |
| deprecated-version-flag.ts | 1 | Yes | Low |
| maestro-substrate-paths.ts | 2 | Yes | Low |
| snapshot-poll-cache.ts | 2 | Yes | Low |
| theme.ts + format.ts | ~10 | Yes | Low |
| session-id.ts | 1 | Yes/Manual | Low |
| review → evidence | 1 | Manual | Low |
| agent → mission | 3 | Yes | Low |
| skills → infra | 1 | Manual | Low |

**Total files requiring import updates:** ~25-30  
**Automated updates:** ~20-25 files  
**Manual updates:** ~5 files

---

## 7. Risk Assessment

### What Could Go Wrong

#### Risk 1: Broken Imports After Move
**Likelihood:** Medium  
**Impact:** High (build fails)  
**Mitigation:**
- Use `bun run typecheck` after each move
- Use automated sed scripts for bulk updates
- Test build after each phase

#### Risk 2: Missed Import Sites
**Likelihood:** Low  
**Impact:** High (runtime errors)  
**Mitigation:**
- Use global search to find all import sites before moving
- Run full test suite after each phase
- Use `rg` to verify no old imports remain

#### Risk 3: Path Alias Issues
**Likelihood:** Low  
**Impact:** Medium (confusing errors)  
**Mitigation:**
- Verify tsconfig.json paths are correct
- Test both `./dist/maestro` and installed `maestro`
- Run `bun run release:local` to test installed binary

#### Risk 4: Feature Boundary Violations
**Likelihood:** Low (only for Phase 3)  
**Impact:** Medium (architectural regression)  
**Mitigation:**
- Run `bun run check:boundaries` after Phase 3
- Review feature boundary rules in AGENTS.md
- Keep public surfaces clean (index.ts exports only)

#### Risk 5: Test Failures
**Likelihood:** Low  
**Impact:** Medium (need to fix tests)  
**Mitigation:**
- Run tests after each phase
- Update test imports alongside source imports
- Check test coverage doesn't drop

#### Risk 6: Git History Loss
**Likelihood:** Very Low (if using git mv)  
**Impact:** Low (harder to trace history)  
**Mitigation:**
- Always use `git mv` instead of manual move
- Use `git log --follow` to trace moved files
- Atomic commits per logical change

### Mitigation Strategies

#### Strategy 1: Incremental Changes
- Do one phase at a time
- Commit after each successful change
- Easy to rollback if something breaks

#### Strategy 2: Automated Verification
```bash
# Create a verification script
cat > scripts/verify-reorganization.sh << 'EOF'
#!/bin/bash
set -e

echo "Running typecheck..."
bun run typecheck

echo "Checking feature boundaries..."
bun run check:boundaries

echo "Running tests..."
bun test

echo "Building CLI..."
bun run build

echo "Verifying CLI works..."
./dist/maestro --version

echo "All checks passed!"
EOF

chmod +x scripts/verify-reorganization.sh
```

#### Strategy 3: Rollback Plan
```bash
# If something breaks, rollback the last commit
git reset --hard HEAD~1

# Or rollback to a specific commit
git reset --hard <commit-sha>

# Or create a rollback branch before starting
git checkout -b backup-before-reorganization
git checkout main
# ... do reorganization ...
# If needed: git reset --hard backup-before-reorganization
```

#### Strategy 4: Parallel Branch
```bash
# Do reorganization on a separate branch
git checkout -b refactor/code-reorganization

# ... do all changes ...

# Test thoroughly before merging
bun run typecheck
bun run check:boundaries
bun test
bun run release:local
maestro --version

# Merge when confident
git checkout main
git merge refactor/code-reorganization
```

### Rollback Plan

#### If Phase 1 Fails
```bash
# Rollback individual file moves
git checkout HEAD -- src/shared/domain/ui-config.ts
git checkout HEAD -- src/tui/shared/ui-config.ts
# ... repeat for other files ...

# Or rollback entire phase
git reset --hard <commit-before-phase-1>
```

#### If Phase 2 Fails
```bash
# Rollback TUI changes
git reset --hard <commit-before-phase-2>

# Phase 1 changes are preserved
```

#### If Phase 3 Fails
```bash
# Rollback feature consolidations
git reset --hard <commit-before-phase-3>

# Phase 1 and 2 changes are preserved
```

#### Nuclear Option
```bash
# Rollback everything
git reset --hard <commit-before-any-changes>

# Or use backup branch
git reset --hard backup-before-reorganization
```

---

## 8. Success Criteria

### How to Verify Nothing Broke

#### Criterion 1: Typecheck Passes
```bash
bun run typecheck
# Expected: No errors
```

#### Criterion 2: Feature Boundaries Respected
```bash
bun run check:boundaries
# Expected: No violations
```

#### Criterion 3: All Tests Pass
```bash
bun test
# Expected: All tests pass, no regressions
```

#### Criterion 4: CLI Builds Successfully
```bash
bun run build
./dist/maestro --version
# Expected: Version output, no errors
```

#### Criterion 5: Installed Binary Works
```bash
bun run release:local
maestro --version
maestro mission-control --json
maestro task verify --task <test-id>
# Expected: All commands work
```

#### Criterion 6: No Orphaned Imports
```bash
# Check for old import paths
rg "@/shared/domain/ui-config" src/
rg "@/shared/lib/deprecated-version-flag" src/
rg "@/shared/lib/maestro-substrate-paths" src/
rg "@/tui/lib/snapshot-poll-cache" src/
rg "@/tui/theme" src/ | grep -v "shared/theme"
rg "@/tui/format" src/ | grep -v "shared/format"
rg "@/features/review" src/ | grep -v "review-ack"
rg "@/features/agent" src/ | grep -v "agent-claimed"
rg "@/features/skills" src/ | grep -v "skills/"

# Expected: No results (or only false positives)
```

#### Criterion 7: No Orphaned Directories
```bash
# Check for empty directories
ls src/features/review 2>/dev/null && echo "ERROR: review/ still exists"
ls src/features/agent 2>/dev/null && echo "ERROR: agent/ still exists"
ls src/features/skills 2>/dev/null && echo "ERROR: skills/ still exists"
ls src/tui/lib 2>/dev/null && echo "ERROR: tui/lib/ still exists"

# Expected: All commands fail (directories don't exist)
```

### How to Validate the New Structure

#### Validation 1: src/shared/ is Truly Generic
```bash
# Check that src/shared/ has no TUI/CLI/feature-specific code
rg "mission|task|evidence|verdict" src/shared/
# Expected: No results (or only generic references)

# Check that src/shared/ has no imports from features/infra/tui
rg "@/features|@/infra|@/tui" src/shared/
# Expected: No results
```

#### Validation 2: TUI Utilities are Grouped
```bash
# Check that TUI utilities are in shared/
ls src/tui/shared/
# Expected: ui-config.ts, theme.ts, format.ts, session-id.ts, modal-model.ts, header-animation.ts

# Check that root level is clean
ls src/tui/*.ts
# Expected: input.ts, README.md (no theme.ts, format.ts, session-id.ts)
```

#### Validation 3: Feature Count is Correct
```bash
# Count features
ls src/features/ | wc -l

# Expected:
# - 31 if no consolidations (Phase 1-2 only)
# - 30 if review merged (Phase 3.1)
# - 29 if review + agent merged (Phase 3.1-3.2)
# - 28 if all consolidations (Phase 3.1-3.3)
```

#### Validation 4: Verify-Specific Code is Co-located
```bash
# Check that substrate-paths is in verify/
ls src/features/verify/lib/substrate-paths.ts
# Expected: File exists

# Check that it's not in shared/
ls src/shared/lib/maestro-substrate-paths.ts 2>/dev/null
# Expected: File not found
```

#### Validation 5: Documentation is Updated
```bash
# Check that AGENTS.md references are updated
rg "src/shared/domain/ui-config" AGENTS.md CLAUDE.md
rg "src/shared/lib/deprecated-version-flag" AGENTS.md CLAUDE.md
rg "src/shared/lib/maestro-substrate-paths" AGENTS.md CLAUDE.md

# Expected: No results (or updated references)
```

### Success Metrics

| Metric | Target | How to Measure |
|--------|--------|----------------|
| Typecheck errors | 0 | `bun run typecheck` |
| Feature boundary violations | 0 | `bun run check:boundaries` |
| Test failures | 0 | `bun test` |
| Build errors | 0 | `bun run build` |
| Orphaned imports | 0 | `rg` searches above |
| Orphaned directories | 0 | `ls` checks above |
| Files in src/shared/ | -3 | Count before/after |
| Files in src/tui/shared/ | +3 to +6 | Count before/after |
| Feature count | 31 or 28-30 | `ls src/features/ \| wc -l` |
| Documentation accuracy | 100% | Manual review |

---

## 9. Conclusion

### Summary

This reorganization plan addresses **3 critical misplacements** and **6 minor organizational improvements** in the Maestro codebase. The changes are conservative, low-risk, and focused on improving discoverability and maintainability without disrupting the excellent existing architecture.

**Key Takeaways:**
1. **The current structure is already very good** - No major refactoring needed
2. **Focus on misplacements** - 3 files in wrong locations (high priority)
3. **Minor TUI improvements** - Flatten lib/, group utilities (medium priority)
4. **Optional consolidations** - 3 minimal features could merge (low priority)
5. **Preserve architecture** - Keep hexagonal patterns, feature boundaries, hub-and-spoke imports

### Recommended Approach

**Phase 1 (High Priority):** Move 3 misplaced files  
**Phase 2 (Medium Priority):** Improve TUI structure  
**Phase 3 (Optional):** Consolidate minimal features  

**Estimated Total Time:** 2-3 hours (including testing)  
**Risk Level:** Low (incremental, reversible changes)  
**Expected Benefit:** Improved discoverability, cleaner boundaries, easier maintenance

### Final Recommendation

**Proceed with Phase 1 and Phase 2.** These changes have clear benefits and low risk. Phase 3 is optional and can be deferred or skipped entirely.

The Maestro codebase is already well-organized. These changes are refinements, not corrections. The feature-first architecture with hexagonal patterns is working excellently and should be preserved.

---

**Plan written to:** `/Users/reinamaccredy/Code/maestro/PROPOSED_CODE_FILE_REORGANIZATION_PLAN.md`

