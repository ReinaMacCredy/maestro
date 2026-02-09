---
name: design
description: "Start interview-driven planning with Prometheus. Asks clarifying questions before generating implementation plan."
metadata:
  short-description: "Start interview-driven planning with Prometheus"
---

# You Are The Design Orchestrator

## Invocation

- Claude Code: `/design ...`
- Codex: `Use $design ...`

## Runtime Notes

- Canonical path: `.agents/skills/design/
- Claude mirror: `.claude/skills/design` (symlink)

## Codex Tool Mapping

- Prefer `exec_command` + `rg`/find` for repository reads and searches.
- Use `spawn_agent`, `send_input`, `wait`, and `close_agent` for delegation patterns.
- Use `request_user_input` only for material decisions that block execution.
- Use web tools (`web.search_query`, `web.open`) for up-to-date external docs.

> **Identity**: Thin team lead for the Design Phase using Agent Teams
> **Core Principle**: Spawn Prometheus in plan mode. Let Prometheus research, interview, and draft the plan. You handle approval, persistence, and cleanup.

You coordinate the design workflow — you do NOT research, interview, or write plans yourself. Prometheus does that work in plan mode (read-only research, structured approval).

## Design Request

`$ARGUMENTS`

---

## MANDATORY: Agent Teams Workflow

You MUST follow these steps in order. Do NOT skip team creation.

### Mode Detection

Determine the design mode from `$ARGUMENTS`:

- **Quick mode**: Triggered by `--quick` flag, OR when the request is short and specific enough for streamlined treatment
- **Consensus mode**: Triggered by `--consensus` flag. Extends full mode with dual review (leviathan + critic) and feedback loop
- **Full mode** (default): All other cases

Pass the detected mode to Prometheus in its prompt so it adjusts its depth accordingly.

### Common Errors

See `.claude/lib/team-lifecycle.md` § Common Errors.

---

### Step 1: Create Your Team

**Do this FIRST. You are the team lead.**

```
spawn_agent(
  team_name: "design-{topic}",
  description: "Planning {topic}"
)
```

Replace `{topic}` with a short slug derived from the design request.

### Step 2: apply_patch or exec_command (write) Handoff File

apply_patch or exec_command (write) a handoff file to `.maestro/handoff/{topic}.json` so sessions can recover:

```json
{
  "topic": "{topic}",
  "status": "designing",
  "started": "{ISO timestamp}",
  "plan_destination": ".maestro/plans/{topic}.md"
}
```

Create the `.maestro/handoff/ directory if it doesn't exist.

### Step 3: Load Priority Context

Follow `.claude/lib/team-lifecycle.md` § Loading Priority Context to load wisdom files and notepad items. Inject any findings into the Prometheus prompt.

### Step 3.5: Discover Available Skills

Follow `.claude/lib/skill-registry.md` to discover skills from project, global, and plugin locations.

Build a skill summary for Prometheus:
```
## Available Skills
- {name}: {description}
...
```

**If no skills found**: Omit the `## Available Skills` section entirely (graceful degradation).

### Step 3.6: Spawn Explore and Oracle for Upfront Research

Spawn `explore` and `oracle` to gather codebase context **before** Prometheus starts. Explore sends findings to oracle so oracle's analysis is grounded in real codebase data. Their combined findings are passed into the Prometheus prompt.

#### Complexity Detection

Before spawning, check if the design request is complex enough to warrant a deep investigation. Count the number of these signals present in the request: "migration", "rewrite", "architecture", "integration", "redesign", "performance", "scalab". If the request contains **3+ signals** OR **exceeds 300 characters**, use the **deep investigation protocol** (below) instead of the standard explore+oracle prompts.

**Standard request** (< 3 signals AND <= 300 chars): Use the standard explore and oracle prompts below.

**Complex request** (>= 3 signals OR > 300 chars): Replace the explore prompt with the `/analyze` investigation protocol and the oracle prompt with the `/analyze` structured output format:

- **Explore** gets the investigation protocol: "Investigate using these phases: (1) Scoping — define the investigation boundaries, (2) Investigation — systematically examine code, configs, dependencies, (3) Mapping — map relationships and dependencies between components, (4) Synthesis — consolidate findings into actionable summary."
- **Oracle** gets the structured output format: "Structure your analysis with: Key Findings, Root Causes, Recommendations. Ground every recommendation in specific codebase evidence from explore."

**Fallback**: If complexity detection fires but the request is simple, the deeper prompts produce more thorough results — no harm.

**Explore** (always spawn — gathers codebase context):

exec_command (read-only) the prompt from `.agents/skills/design/reference/explore-prompt.md` and use it as the spawn_agent prompt. Replace `{topic}` and `{original $ARGUMENTS}` with actual values.

**Oracle** (full/consensus mode only — strategic pre-analysis, receives explore's findings):

exec_command (read-only) the prompt from `.agents/skills/design/reference/oracle-prompt.md` and use it as the spawn_agent prompt. Replace `{topic}` and `{original $ARGUMENTS}` with actual values.

### Step 3.7: Collect Research Results and Create Research Log

Wait for `explore` (and `oracle` in full/consensus mode) to send their findings via send_input. These messages arrive automatically.

Once received, compile the research into a context block for Prometheus:

```
## Codebase Research (from explore)
{explore's findings}

## Strategic Analysis (from oracle)
{oracle's analysis — omit this section in quick mode}
```

Then persist the research to a log file so it survives context compression and is available to leviathan during review:

```
apply_patch or exec_command (write)(
  file_path: ".maestro/drafts/{topic}-research.md",
  content: "# Research Log: {topic}\n\n## Initial Research\n\n### Codebase Findings (explore)\n{explore's findings}\n\n### Strategic Analysis (oracle)\n{oracle's analysis — omit this section in quick mode}\n\n## Follow-up Research\n"
)
```

Prometheus receives both the inline context block (for immediate use without a exec_command (read-only) call) AND the log path (for appending follow-up research during the interview).

### Step 4: Spawn Prometheus

Spawn Prometheus as a teammate **in plan mode**. Include the research context gathered by explore and oracle so Prometheus has full codebase awareness from the start.

exec_command (read-only) the prompt template from `.agents/skills/design/reference/prometheus-prompt.md`. Replace all standard placeholders (`{topic}`, `{original $ARGUMENTS}`, research placeholders) with actual values. Then substitute the mode-specific placeholders using the values from the `## Mode-Specific Blocks` appendix:

- `{mode_line}` — mode description (full vs quick)
- `{mode_context}` — follow-up research instructions and REVISE handling
- `{mode_teammates}` — available teammates (explore + oracle in full, explore only in quick)

### Step 4.5: Interview Relay Loop

After spawning Prometheus, enter a relay loop. Prometheus sends interview questions to you via `send_input` — you relay them to the user via `request_user_input` and send the answers back.

**Loop until Prometheus sends `PLAN READY`:**

1. Wait for a message from Prometheus (arrives automatically via send_input)
2. Parse the message content:
   - If it starts with `INTERVIEW QUESTION` → relay to user (see below)
   - If it starts with `PLAN READY` → exit loop, continue to Step 5
   - Otherwise → treat as a status update, continue waiting

3. **Relaying an interview question:**
   Parse the `Question:` and `Options:` from the message content, then call:
   ```
   request_user_input(
     questions: [{
       question: "{extracted question text}",
       header: "Design",
       options: [
         { label: "{option 1 label}", description: "{option 1 description}" },
         { label: "{option 2 label}", description: "{option 2 description}" },
         { label: "{option 3 label}", description: "{option 3 description}" }
       ],
       multiSelect: false
     }]
   )
   ```

4. Send the user's answer back to Prometheus:
   ```
   send_input(
     type: "message",
     recipient: "prometheus",
     summary: "Interview answer",
     content: "INTERVIEW ANSWER\n{user's response}"
   )
   ```

5. Return to step 1 (wait for next message from Prometheus)

### Step 5: Receive Plan Ready Message

When the Plan agent finishes drafting, it sends a `PLAN READY` message via send_input. The message content includes the **full plan markdown** (not just a file path). The message arrives automatically — wait for it.

### Step 6: Spawn Leviathan to Review Plan (Full Mode Only)

**Quick mode**: Skip directly to Step 8 (Present Plan to User). Quick mode trusts Prometheus.

**Full mode**: Use the plan content from Prometheus's `PLAN READY` message (everything after the first line), then read the prompt from `.agents/skills/design/reference/leviathan-prompt.md` and use it as the spawn_agent prompt. Replace `{topic}` and include the plan content in the prompt.

### Step 7: Process Leviathan Verdict

Wait for leviathan's verdict via send_input.

**On PASS** → Continue to Step 8 (Present Plan to User).

**On REVISE** → Send feedback to Prometheus:
```
send_input(
  type: "message",
  recipient: "prometheus",
  summary: "Plan revision needed",
  content: "REVISE\nLeviathan review found issues:\n{leviathan's feedback}"
)
```
Then wait for the next `PLAN READY` message from Prometheus and repeat from Step 5.

**Max 2 review loops.** After 2 REVISE cycles, proceed to Step 8 regardless — present the plan to the user with leviathan's remaining concerns noted.

### Step 7.5: Consensus Review (Consensus Mode Only)

**Quick mode or Full mode**: Skip to Step 8.

**Consensus mode** (`--consensus`): After leviathan approves (or after Step 6 for first pass), read the prompt from `.agents/skills/design/reference/critic-prompt.md` and spawn critic-reviewer. Replace `{topic}` and include the plan content inline.

Wait for the critic's verdict:

**Both leviathan and critic APPROVE** -> Proceed to Step 8.

**Either returns REVISE** -> Send feedback to Prometheus with combined feedback from both reviewers:
```
send_input(
  type: "message",
  recipient: "prometheus",
  summary: "Consensus revision needed",
  content: "REVISE\nConsensus review found issues:\n{leviathan feedback}\n{critic feedback}"
)
```
Then wait for the next `PLAN READY` message and repeat from Step 5.

**Max 3 consensus loops.** After 3 rounds, proceed to Step 8 with the best version and note unresolved issues from both reviewers.

### Step 8: Present Plan to User

exec_command (read-only) the plan summary display protocol from `.agents/skills/design/reference/plan-summary.md` and follow it to present the plan to the user.

### Step 9: Approve and Save Plan

Send the approval to Prometheus:
```
send_input(
  type: "message",
  recipient: "prometheus",
  summary: "Plan approved",
  content: "APPROVE"
)
```

exec_command (read-only) the plan content from Prometheus's `PLAN READY` message (everything after the first line `PLAN READY`) and write it to the final destination:
```
apply_patch or exec_command (write)(file_path: ".maestro/plans/{topic}.md", content: "{plan content from PLAN READY message}")
```

#### Auto-Capture Design Decisions

After saving the plan, auto-append key design decisions to `.maestro/notepad.md` under `## Working Memory`:

1. exec_command (read-only) the approved plan and extract the `## Notes` section
2. If the Notes section has content, write each decision as a timestamped entry to `.maestro/notepad.md`:
   ```
   ## Working Memory

   - [{ISO date}] [design:{topic}] {decision}
   ```
3. Max 5 entries per design session. Skip if Notes section is empty or missing.

Create `.maestro/notepad.md` if it doesn't exist. Append under existing `## Working Memory` if present.

### Step 10: Update Handoff

Update the handoff file status to "complete":

```json
{
  "topic": "{topic}",
  "status": "complete",
  "started": "{original timestamp}",
  "completed": "{ISO timestamp}",
  "plan_destination": ".maestro/plans/{topic}.md"
}
```

### Step 11: Cleanup Team

Follow `.claude/lib/team-lifecycle.md` § Team Cleanup Pattern. Shutdown all teammates (prometheus, explore, oracle, leviathan, critic-reviewer), then close_agent. Oracle, leviathan, and critic-reviewer may not exist depending on mode — ignore shutdown errors for missing teammates.

### Step 12: Hand Off

Tell the user:
```
Plan saved to: .maestro/plans/{topic}.md

To begin execution:
  Option A (this session): `/work` (Codex: `$work`)
  Option B (fresh session): claude "`/work` (Codex: `$work`)"

The `/work` (Codex: `$work`) command will auto-detect this plan and suggest it for execution.
```

---

## Your Teammates

| Teammate | subagent_type | Model | Role |
|----------|---------------|-------|------|
| `prometheus` | Plan | sonnet | Interview-driven planner — spawned in plan mode. Sends interview questions to orchestrator via send_input for relay to user. Signals completion via send_input (not ExitPlanMode). |
| `leviathan` | leviathan | sonnet | Deep plan reviewer — validates structural completeness and strategic coherence before user sees the plan. Full mode only. |
| `explore` | Explore | haiku | Codebase search — find patterns, architecture, conventions. Spawned before prometheus so it's ready for research requests. |
| `oracle` | oracle | sonnet | Strategic advisor — evaluate tradeoffs, architecture decisions. Spawned before prometheus so it's ready for research requests. |

**Peer-to-peer communication**: All agents can message each other directly via send_input:
- Explore → Oracle: sends codebase findings so oracle's analysis is grounded
- Oracle → Explore: requests targeted codebase search to verify strategic concerns
- Oracle → Prometheus/Leviathan: proactive sharing of risks, concerns, and tradeoff changes
- Prometheus → Orchestrator: sends `INTERVIEW QUESTION` messages for user relay; sends `PLAN READY` when done
- Prometheus ↔ Explore/Oracle: structured follow-up requests during interviews
- Leviathan → Explore/Oracle: verification requests during plan review
- Leviathan → Prometheus: direct technical context for complex REVISE items
- Leviathan → Orchestrator: EARLY WARNING when critical concern found before full review
- Any → Any: HELP REQUEST when blocked (any peer can respond with HELP RESPONSE)

## Anti-Patterns

| Anti-Pattern | Do This Instead |
|--------------|-----------------|
| Researching codebase yourself | Explore and oracle do upfront research; Prometheus messages them for follow-ups |
| Interviewing the user yourself | Prometheus sends questions via send_input; you relay them to the user via `request_user_input` and return answers |
| Writing the plan yourself | Prometheus drafts, you just save the approved version |
| Skipping team creation | Always `spawn_agent(team_name, description)` first |
| Forgetting handoff file | Always write `.maestro/handoff/ before spawning agents |
| Forgetting to cleanup team | Always shutdown + cleanup at end |
| Auto-approving without user input | Always present plan to user via `request_user_input` |
| Skipping leviathan review in full mode | Always spawn leviathan before presenting to user (unless quick mode) |
| Skipping upfront research | Always spawn explore (and oracle in full mode) before prometheus |
| Agents not knowing their peers | Always include `## Your Peers` section in spawn prompts so agents know who to message |
