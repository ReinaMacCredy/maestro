# LEARNINGS: Planning Pipeline + Orchestrator Enhancement

Track: planning-integration
Completed: 2026-01-03

---

## Commands

- `register_agent(name="PurpleMountain", ...)` - Manual agent registration when auto-generation fails
- `bd update my-workflow:3-ktgt --status in_progress` - Update epic status for orchestration
- `bd close <spike-id> --reason "YES: <approach>"` - Close spike with structured result
- `bd close <epic-id> --reason "All beads complete"` - Close epic with summary
- `macro_start_session()` - Initialize orchestrator/worker session (may fail on unique name)

## Gotchas

- Agent registration may fail on unique name generation - fallback to manual `register_agent()` with explicit name
- Worker completion messages can fail silently if name generation fails - bead still closes via `bd` command
- Track thread format: `track:<agent>:<epic>` - colon-delimited, not slash
- Spike learnings injection uses `{SPIKE_LEARNINGS}` placeholder in worker prompts

## Patterns

- **Two-Thread Architecture**: Epic thread for coordination, track thread for bead-to-bead context
- **Per-Bead Loop**: START (read track thread) → WORK → COMPLETE (write context) → NEXT
- **Track Thread Context Structure**: Learnings, Gotchas, Next Notes sections
- **Spike Workflow**: Create spike → Execute via Task() → Close with result → Update design.md Section 5
- **Spike Learnings Capture**: Embed in bead descriptions, reference spike code path
- **4-Step Worker Protocol**: register → claim/work/close → send_message → release_file_reservations
- **Parallel Track Execution**: File-scope based grouping enables non-overlapping parallel execution

## Files Changed

### New Files
- `skills/conductor/references/planning/pipeline.md` - 6-phase planning flow
- `skills/conductor/references/planning/design-template.md` - Unified design.md format
- `skills/conductor/references/planning/spikes.md` - Spike workflow
- `skills/orchestrator/references/track-threads.md` - Track thread protocol
- `conductor/spikes/` - Spike storage directory

### Modified Files
- `skills/maestro-core/SKILL.md` - Added `pl` routing
- `skills/maestro-core/references/routing-table.md` - Added `pl` trigger
- `skills/maestro-core/references/workflow-chain.md` - Dual path diagram
- `skills/orchestrator/SKILL.md` - Track thread reference
- `skills/orchestrator/references/workflow.md` - Option C (planning pipeline source)
- `skills/orchestrator/references/worker-prompt.md` - Spike learnings section
- `skills/conductor/SKILL.md` - Planning state reference
- `skills/conductor/references/schemas/metadata.schema.json` - Planning section
- `conductor/CODEMAPS/overview.md` - Planning pipeline + spikes directory
