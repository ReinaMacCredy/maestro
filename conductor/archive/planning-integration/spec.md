# Specification: Planning Pipeline + Orchestrator Enhancement

Track ID: planning-integration
Created: 2026-01-03
Source: design.md

---

## Overview

Integrate dual routing (`ds` vs `pl`) for design/planning workflows and enhance orchestrator with track thread pattern for bead-to-bead context preservation.

## Functional Requirements

### FR1: Dual Routing (ds vs pl)

| ID | Requirement |
|----|-------------|
| FR1.1 | `pl` trigger routes to planning pipeline |
| FR1.2 | `ds` continues to route to Double Diamond |
| FR1.3 | Intent detection suggests `ds` or `pl` for ambiguous requests |
| FR1.4 | Users can switch between modes with preserved progress |

### FR2: Planning Pipeline (pl mode)

| ID | Requirement |
|----|-------------|
| FR2.1 | Phase 1 (Discovery): Parallel agents gather codebase intelligence |
| FR2.2 | Phase 2 (Synthesis): Oracle produces gap analysis + risk map |
| FR2.3 | Phase 3 (Verification): HIGH risk items create spikes |
| FR2.4 | Phase 4 (Decomposition): File beads with spike learnings |
| FR2.5 | Phase 5 (Validation): bv validates dependency graph |
| FR2.6 | Phase 6 (Track Planning): Generate Track Assignments |

### FR3: Unified design.md

| ID | Requirement |
|----|-------------|
| FR3.1 | design.md contains sections: Problem, Discovery, Approach, Design, Spike Results, Track Planning |
| FR3.2 | design.md generated from either `ds` or `pl` path |
| FR3.3 | metadata.json tracks which path (`design_path: ds | pl`) |

### FR4: Spike Workflow

| ID | Requirement |
|----|-------------|
| FR4.1 | Spikes stored in `conductor/spikes/<track-id>/spike-xxx/` |
| FR4.2 | Each spike has README.md with question, criteria, result |
| FR4.3 | Spike learnings embedded in beads |
| FR4.4 | Spike code referenced in worker prompts |

### FR5: Track Thread Protocol

| ID | Requirement |
|----|-------------|
| FR5.1 | Epic thread for cross-track coordination |
| FR5.2 | Track thread (`track:<agent>:<epic>`) for bead-to-bead context |
| FR5.3 | Workers read track thread before starting bead |
| FR5.4 | Workers write structured context after completing bead |
| FR5.5 | Context includes: Learnings, Gotchas, Spike References, Next Bead Notes |

### FR6: Orchestrator Integration

| ID | Requirement |
|----|-------------|
| FR6.1 | Orchestrator detects `design_path: pl` in metadata |
| FR6.2 | Spike learnings injected into worker prompts |
| FR6.3 | Worker prompt template includes Spike Learnings section |
| FR6.4 | File reservation per-bead with explicit reserve/release |

## Non-Functional Requirements

| ID | Requirement |
|----|-------------|
| NFR1 | Backward compatible with existing `ds` workflow |
| NFR2 | No breaking changes to existing beads format |
| NFR3 | Track thread messages summarized to prevent context overflow |
| NFR4 | Validation gates HALT on unresolved HIGH risk spikes |

## Acceptance Criteria

- [ ] `pl` trigger works in maestro-core routing
- [ ] Discovery phase spawns parallel Task() agents
- [ ] Oracle produces risk map with HIGH/MEDIUM/LOW classification
- [ ] Spike created in `conductor/spikes/<track>/` for HIGH risk items
- [ ] Spike learnings appear in bead descriptions
- [ ] Workers read track thread at bead start
- [ ] Workers write structured context at bead complete
- [ ] Orchestrator injects spike learnings into worker prompts
- [ ] metadata.json tracks planning state machine

## Out of Scope

- Changing beads storage format (.beads/*.md)
- Modifying bd CLI behavior
- Agent Mail protocol changes
- Double Diamond phase modifications
