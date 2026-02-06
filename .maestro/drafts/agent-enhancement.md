# Draft: Agent Enhancement

## Confirmed Requirements

- **Goal**: Enhance all Maestro agents with better coordination, accuracy, and efficiency
- **Focus Areas**:
  1. Context sharing via `.maestro/handoff/` directory
  2. Task complexity-based delegation rules
  3. Verification enforcement (read files + run tests after each task)

## Technical Decisions

### 1. Context Sharing — Handoff Directory

Location: `.maestro/handoff/`

Agents write findings to files that persist across the team:
- `research.md` — Explore agent findings (file paths, patterns)
- `decisions.md` — Oracle recommendations, architectural choices
- `progress.md` — Completed work, what's been changed

Each agent reads handoff before starting, writes handoff after completing.

### 2. Delegation Heuristics

Orchestrator uses task complexity rules:

| Complexity | Agent | Trigger Keywords/Patterns |
|------------|-------|---------------------------|
| Research | `explore` | "find", "search", "locate", "what is" |
| Architecture | `oracle` | "design", "tradeoff", "recommend", "complex decision" |
| New feature | `kraken` | "implement", "add", "create", multi-file, needs tests |
| Simple fix | `spark` | "fix", "update", "change", single-file, config |

### 3. Verification Enforcement

After EACH task completion:
1. Orchestrator reads all files agent claims to have modified
2. Orchestrator runs build/lint/test commands
3. Only marks task complete if verification passes
4. Update verification-injector.sh to be more prescriptive

## Scope

**In**:
- All 6 agent definitions
- Handoff directory structure
- Orchestrator delegation logic
- Verification hook improvements

**Out**:
- New agents (using existing 6)
- Wisdom accumulation (future enhancement)
- Automated test scenarios (manual testing only)

## Testing Strategy

Manual testing by running `/design` and `/work` workflows

## Research Findings

(See full researcher report — 6 agents, 2 hooks, team-based workflows)
