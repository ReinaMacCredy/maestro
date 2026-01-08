# Spike Workflow

Spikes validate HIGH risk components before implementation.

## When to Create Spikes

| Risk Level | Criteria | Action |
|------------|----------|--------|
| LOW | Pattern exists in codebase | Proceed |
| MEDIUM | Variation of existing pattern | Interface sketch |
| HIGH | Novel or external integration | Spike required |

### Risk Indicators

```
Pattern exists in codebase? ─── YES → LOW base
                            └── NO  → MEDIUM+ base

External dependency? ─── YES → HIGH
                     └── NO  → Check blast radius

Blast radius >5 files? ─── YES → HIGH
                       └── NO  → MEDIUM
```

## Spike Creation Flow

1. **Create Spike Bead**
   ```bash
   bd create "Spike: <question to answer>" -t epic -p 0
   bd create "Spike: Test X" -t task --blocks <spike-epic>
   ```

2. **Create Spike Directory**
   ```bash
   mkdir -p conductor/spikes/<track-id>/spike-xxx/
   ```

3. **Create README.md**
   ```markdown
   # Spike: <specific question>

   **Time-box**: 30 minutes
   **Output location**: conductor/spikes/<track-id>/spike-xxx/

   ## Question
   Can we <specific technical question>?

   ## Success Criteria
   - [ ] Working throwaway code exists
   - [ ] Answer documented (yes/no + details)
   - [ ] Learnings captured for main plan

   ## On Completion
   Close with: `bd close <id> --reason "YES: <approach>" or "NO: <blocker>"`
   ```

## Spike Execution

### Via Task() Subagent

Spikes are executed via Task() with time-box:

```python
Task(
  description="Execute spike: <question>",
  context="""
  Time-box: 30 minutes
  Output location: conductor/spikes/<track>/<spike-id>/
  Success criteria:
  - Working throwaway code
  - Answer documented
  - Learnings captured
  """
)
```

### Time-box Policy

| Duration | Use Case |
|----------|----------|
| 15 min | Simple API test |
| 30 min | Integration test (default) |
| 60 min | Complex external integration |

### On Timeout

```bash
bd close <id> --reason "TIMEOUT: <partial findings>"
```

Timeout spikes escalate to user for decision.

## Spike Result Aggregation

After all spikes complete, aggregate via Oracle:

```python
oracle(
  task="Synthesize spike results and update approach",
  context="Spikes completed. Results: ...",
  files=["conductor/tracks/<id>/design.md"]
)
```

Oracle updates:
1. design.md Section 5 with detailed results
2. design.md Section 3 with revised approach (if needed)
3. Risk map (downgrade verified items)

## Spike Learnings Capture

Capturing spike learnings ensures validated knowledge flows into implementation.

### Step 1: Close Spike with Result

```bash
# Approach validated - spike succeeded
bd close <spike-id> --reason "YES: <approach that works>"

# Approach blocked - found alternative
bd close <spike-id> --reason "NO: <blocker encountered>"

# Partial success - document both
bd close <spike-id> --reason "PARTIAL: <what worked, what didn't>"
```

| Result | Format | Next Action |
|--------|--------|-------------|
| YES | `YES: <validated approach>` | Proceed with approach |
| NO | `NO: <blocker> → <alternative>` | Update design with alternative |
| PARTIAL | `PARTIAL: <works> / <doesn't>` | Refine approach |

### Step 2: Update design.md Section 5

Add structured results to design.md:

```markdown
## 5. Spike Results

### Spike: <question>
- **Result**: YES/NO/PARTIAL
- **Bead**: <spike-bead-id>
- **Learnings**: 
  - <learning 1>
  - <learning 2>
- **Code reference**: conductor/spikes/<track>/spike-xxx/
- **Impact on approach**: <how this affects Section 3>
```

### Step 3: Embed in Implementation Beads

When filing beads (`fb`), include spike learnings:

```markdown
# Implement Stripe webhook handler

## Context
Spike bd-12 validated: Stripe SDK works with our Node version.
See `conductor/spikes/billing/spike-001/` for working example.

## Learnings from Spike
- Must use `stripe.webhooks.constructEvent()` for signature verification
- Webhook secret stored in `STRIPE_WEBHOOK_SECRET` env var
- Raw body required (not parsed JSON)

## Acceptance Criteria
- [ ] Webhook endpoint at `/api/webhooks/stripe`
- [ ] Signature verification implemented
- [ ] Events: `checkout.session.completed`, `invoice.paid`
```

### Step 4: Integration Workflow

```
Spike completes → bd close with result
      ↓
Update design.md Section 5
      ↓
fb (file-beads) embeds learnings in beads
      ↓
Worker prompts receive learnings
      ↓
Implementation uses validated approach
```

### Learnings Reference Format

For worker prompts, reference spike learnings:

```markdown
## Spike Context
- **Spike**: <question answered>
- **Result**: YES/NO/PARTIAL
- **Key learnings**:
  - <learning 1>
  - <learning 2>
- **Code path**: conductor/spikes/<track>/spike-xxx/
```

## Spike Failure Handling

| Result | Action |
|--------|--------|
| YES | Proceed with validated approach |
| NO (alternative found) | Update approach, proceed |
| NO (blocker) | HALT, require user decision |
| TIMEOUT | Escalate to user |

## Integration Points

- **design.md Section 5**: Spike Results summary
- **Bead descriptions**: Embedded learnings
- **worker-prompt.md**: Spike code references
- **metadata.json**: Spike state tracking

### metadata.json Tracking

```json
{
  "planning": {
    "spikes": [
      {
        "id": "spike-001",
        "question": "Can Stripe SDK work with Node 18?",
        "result": "YES",
        "path": "conductor/spikes/<track>/spike-001/"
      }
    ]
  }
}
```

## Related

- [pipeline.md](pipeline.md) - Full planning pipeline (Phase 3: Verification)
- [design-template.md](design-template.md) - design.md Section 5 format
