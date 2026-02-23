# Phase Instructions — Feature Planning Pipeline

Detailed instructions for each phase of the planning pipeline. Load this file when executing a phase.

## Phase 0: Session Setup (/design-compatible)

Before discovery, initialize planning session state exactly like `/design`:

1. Derive topic slug from request (`kebab-case`, max 4 words).
2. Create `.maestro/handoff/{topic}.json` with:

```json
{
  "topic": "{topic}",
  "status": "designing",
  "started": "{ISO timestamp}",
  "plan_destination": ".maestro/plans/{topic}.md"
}
```

3. Ensure `.maestro/handoff/` and `.maestro/plans/` directories exist.

## Phase 1: Discovery (Parallel Exploration)

Launch parallel discovery using Amp tools:

```
parallel(...) → `finder`: architecture and pattern discovery
parallel(...) → `shell_command`: constraints scan (`rg`, `cat package.json`, tsconfig)
`librarian` → External/reference implementations
`web_search` + `read_web_page` → library docs when integrating external APIs
`handoff` (optional) → isolate deep discovery in separate threads
```

Save the report to `.maestro/drafts/{topic}-discovery.md`. See [templates.md](templates.md) for the Discovery Report template.

## Phase 2: Synthesis (Amp Review)

Synthesize the discovery report in the main thread. If the feature is complex or high risk, run a second review pass in a `handoff` thread and merge findings.

Synthesis output must include:

1. **Gap Analysis** — What exists vs what's needed
2. **Approach Options** — 1-3 strategies with tradeoffs
3. **Risk Assessment** — LOW / MEDIUM / HIGH per component

### Risk Classification

| Level  | Criteria                      | Verification                 |
| ------ | ----------------------------- | ---------------------------- |
| LOW    | Pattern exists in codebase    | Proceed                      |
| MEDIUM | Variation of existing pattern | Interface sketch, type-check |
| HIGH   | Novel or external integration | Spike required               |

### Risk Indicators

```
Pattern exists in codebase? ─── YES → LOW base
                            └── NO  → MEDIUM+ base

External dependency? ─── YES → HIGH
                     └── NO  → Check blast radius

Blast radius >5 files? ─── YES → HIGH
                       └── NO  → MEDIUM
```

Save to `.maestro/drafts/{topic}-approach.md`. See [templates.md](templates.md) for the Approach Document template.

## Phase 3: Verification (Risk-Based)

### For HIGH Risk Items → Create Spike Beads

Spikes are mini-plans executed with `handoff` workers plus beads CLI:

```bash
br create "Spike: <question to answer>" -t epic -p 0
br create "Spike: Test X" -t task --blocks <spike-epic>
br create "Spike: Verify Y" -t task --blocks <spike-epic>
```

### Execute Spikes

Use parallel `handoff` workers where spikes are independent:

1. `bv -robot-plan` to parallelize spikes
2. `handoff(goal: "Execute spike <id> in 30 minutes and write findings")` per spike
3. Workers write to `.spikes/<feature>/<spike-id>/`
4. Close with learnings: `br close <id> --reason "<result>"`

### Aggregate Spike Results

Use main-thread synthesis to merge spike findings into `.maestro/drafts/{topic}-approach.md`. Update approach.md with validated learnings.

See [templates.md](templates.md) for the Spike Bead template.

## Phase 4: Decomposition (Beads)

Create beads with embedded learnings. If `file-beads` skill exists, load it; otherwise use `br create` directly.

```bash
# Optional:
skill("file-beads")

# Always valid:
br create "<bead title>" -t task --description "<context + acceptance criteria>"
```

### Bead Requirements

Each bead MUST include:

- **Spike learnings** embedded in description (if applicable)
- **Reference to .spikes/ code** for HIGH risk items
- **Clear acceptance criteria**
- **File scope** for track assignment

See [templates.md](templates.md) for the Bead with Learnings template.

## Phase 5: Validation

### Run bv Analysis

```bash
bv -robot-suggest   # Find missing dependencies
bv -robot-insights  # Detect cycles, bottlenecks
bv -robot-priority  # Validate priorities
```

### Fix Issues

```bash
br dep add <from> <to>      # Add missing deps
br dep remove <from> <to>   # Break cycles
br update <id> --priority X  # Adjust priorities
```

### Final Review Pass

Run a final clarity/completeness pass in-thread. For difficult plans, run a `handoff` reviewer thread and merge feedback before finalizing.

## Phase 6: Track Planning

This phase creates an **execution-ready plan** so the orchestrator can spawn workers immediately without re-analyzing beads.

### Step 1: Get Parallel Tracks

```bash
bv -robot-plan 2>/dev/null | jq '.plan.tracks'
```

### Step 2: Assign File Scopes

For each track, determine the file scope based on beads in that track:

```bash
# For each bead, check which files it touches
br show <bead-id>  # Look at description for file hints
```

**Rules:**

- File scopes MUST NOT overlap between tracks
- Use glob patterns: `packages/sdk/**`, `apps/server/**`
- If overlap is unavoidable, merge into a single track

### Step 3: Generate Agent Names

Assign unique adjective+noun names to each track:

- BlueLake, GreenCastle, RedStone, PurpleBear, etc.
- Names are memorable identifiers, NOT role descriptions

### Step 4: Create Execution Plan

Save to `.maestro/drafts/{topic}-execution-plan.md`. See [templates.md](templates.md) for the Execution Plan template.

### Validation

Before finalizing, verify:

```bash
# No cycles in the graph
bv -robot-insights 2>/dev/null | jq '.Cycles'

# All beads assigned to tracks
bv -robot-plan 2>/dev/null | jq '.plan.unassigned'
```

## Phase 7: Approve and Save Plan (/design-compatible)

This phase mirrors `/design`: review, approve, persist plan, update handoff, and hand off to `/work`.

### Step 1: Present Plan Summary

Present a concise summary from `.maestro/drafts/{topic}-execution-plan.md`:

- Title
- Objective
- Track count and assigned beads
- Cross-track dependencies
- Verification status (`Cycles`, `unassigned`)

### Step 2: Approval Gate

Ask for one decision: `Approve`, `Revise`, or `Cancel`.

- `Approve` → proceed to Step 3.
- `Revise` → apply requested changes to `.maestro/drafts/{topic}-approach.md`, beads, and `.maestro/drafts/{topic}-execution-plan.md`; then repeat Step 1.
- `Cancel` → set handoff status to `cancelled` and stop.

### Step 3: Save Final Plan to `.maestro/plans`

Write final plan to:

```text
.maestro/plans/{topic}.md
```

Plan must include:

- Feature goal and scope
- Approach summary + risk map
- Ordered beads by track
- Cross-track dependencies
- Verification commands
- Key spike learnings

### Step 4: Auto-Capture Decisions to Notepad

Append up to 5 key decisions to `.maestro/notepad.md` under `## Working Memory`:

```text
- [{ISO date}] [planning:{topic}] {decision}
```

Create file/section if missing.

### Step 5: Update Handoff Status

Update `.maestro/handoff/{topic}.json`:

```json
{
  "topic": "{topic}",
  "status": "complete",
  "started": "{original timestamp}",
  "completed": "{ISO timestamp}",
  "plan_destination": ".maestro/plans/{topic}.md"
}
```

### Step 6: Handoff to Execution

Report:

```text
Plan saved to: .maestro/plans/{topic}.md

To execute:
  /work
```
