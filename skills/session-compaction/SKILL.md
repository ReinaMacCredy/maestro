---
name: session-compaction
version: 1.0.0
description: Compress session context using anchored iterative summarization. Use when context is getting long, before ending a session, or when you need to create a resumable checkpoint.
triggers:
  - compact
  - /compact
  - session compact
  - compress context
---

# Session Compaction

Compress current session context using anchored iterative summarization.

## Usage

```
compact              → compress + save to history/threads/YYYY-MM-DD-HH-MM-compact.md
compact path/to.md   → compress + save to custom path
compact --verbose    → show full quality scores
```

## Behavior

1. **Analyze** the current session for key information
2. **Generate** a structured summary using the format below
3. **Self-evaluate** with 4 probe questions (recall, artifact, continuation, decision)
4. **Check threshold** - warn if quality score < 3.5
5. **Save** to file (default: `history/threads/YYYY-MM-DD-HH-MM-compact.md`)
6. **Display** summary to user

## Output Format

Generate this exact structure:

```markdown
# Session Compact - YYYY-MM-DD HH:MM

## Intent
[1-2 sentences: What we're trying to accomplish and why]

## Artifacts

### Modified
| File | Change |
|------|--------|
| `path/to/file.ts` | Brief description of changes |

### Read  
| File | Reason |
|------|--------|
| `path/to/file.ts` | Why it was examined |

## Decisions
1. **Decision title** - reasoning behind the choice

## State
[Current progress: what works, what's broken, where we are in the task]

## Next
1. [Immediate next action]
2. [Following action]

## Blockers
[Any issues preventing progress, or "None"]

## Quality: X.X/5.0
[Only shown in --verbose mode or when below 3.5 threshold]
```

## Section Requirements

Each section must be populated or explicitly marked empty:

| Section | Required Content |
|---------|------------------|
| **Intent** | Goal + motivation, not just task description |
| **Modified** | Files changed this session (use git diff if available) |
| **Read** | Files examined, why they mattered |
| **Decisions** | Choices made, with reasoning |
| **State** | What works now, what's broken, current position |
| **Next** | Actionable steps, specific enough to execute |
| **Blockers** | What's stopping progress, or explicit "None" |

## Self-Evaluation

After generating the summary, evaluate its quality:

### Generate 4 Probes

Create one question for each probe type based on session content:

| Type | Purpose | Example |
|------|---------|---------|
| **Recall** | Test factual retention | "What was the original error message?" |
| **Artifact** | Test file tracking | "Which files have we modified?" |
| **Continuation** | Test next-step clarity | "What should we do next?" |
| **Decision** | Test reasoning preservation | "Why did we choose approach X over Y?" |

### Answer and Grade

1. Answer each probe using only the compacted summary
2. Grade each dimension on a 1-5 scale
3. Compute overall score as unweighted average of 6 dimensions

### Quality Threshold

- **Score ≥ 3.5**: Save and display summary (hide quality score unless --verbose)
- **Score < 3.5**: Display warning with lowest-scoring dimension
  ```
  ⚠️ Low [dimension] (X.X) - [actionable suggestion]
  ```

### Actionable Suggestions by Dimension

| Dimension | Suggestion |
|-----------|------------|
| Accuracy | "verify file paths and technical details" |
| Context Awareness | "re-read recent messages to capture current state" |
| Artifact Trail | "run `git status` to find all modified files" |
| Continuity | "check for unresolved TODOs and open questions" |
| Completeness | "review earlier conversation for missed topics" |
| Instruction Following | "ensure all sections are populated" |

## Anchored Iterative Summarization

Key principles from Factory.ai research:

1. **Structure forces preservation** - Each section acts as a checklist
2. **Explicit > implicit** - Empty sections marked "None" not omitted
3. **Merge don't regenerate** - When re-compacting, update existing sections
4. **Specificity over brevity** - Include paths, names, values exactly

## Implementation Notes

- Use `git diff --name-only` to detect modified files when in a git repo
- Track files read from conversation context (tool calls, mentions)
- Generate probes that reference specific facts from the session
- Ensure `history/threads/` directory exists before saving
- Timestamp format: YYYY-MM-DD-HH-MM (24-hour, no seconds)
