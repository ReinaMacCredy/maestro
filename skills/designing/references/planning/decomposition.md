# Decomposition - From Plan to Beads

Decomposition transforms a plan.md into executable beads that workers can implement in parallel.

## Entry Point

| Trigger | Reference | Action |
|---------|-----------|--------|
| `fb`, `file-beads` | [tracking/FILE_BEADS.md](../../../tracking/references/FILE_BEADS.md) | Create beads from plan |
| `rb`, `review-beads` | [tracking/REVIEW_BEADS.md](../../../tracking/references/REVIEW_BEADS.md) | Review filed beads |

## How Decomposition Feeds Bead Creation

```
plan.md → Decompose tasks → fb (file-beads) → beads → Track Assignment → Workers
```

The `fb` command from the [tracking skill](../../../tracking/SKILL.md) reads `plan.md` and creates beads automatically. Each bead receives:
- Task description from plan
- Spike learnings (if applicable)
- File scope for conflict prevention
- Dependencies for ordering

## Bead Template with Embedded Spike Learnings

For HIGH risk items validated by spikes, embed learnings directly in bead description:

```markdown
# <task title>

## Context
Spike bd-XX validated: <approach>
See `conductor/spikes/<track-id>/spike-xxx/` for working example.

## Learnings from Spike
- <learning 1>
- <learning 2>
- <learning 3>

## File Scope
Files this bead modifies (for reservation):
- `src/api/<endpoint>.ts`
- `src/handlers/<handler>.ts`

## Acceptance Criteria
- [ ] Criterion 1
- [ ] Criterion 2
- [ ] Criterion 3

## Test Requirements
- [ ] Unit test for <component>
- [ ] Integration test for <flow>
```

## Reference to .spikes/ Code

For HIGH risk items with validated spikes, always include spike reference:

| Risk Level | Spike Reference Required |
|------------|-------------------------|
| LOW | No - proceed with existing patterns |
| MEDIUM | Optional - interface sketch if available |
| HIGH | **Required** - reference validated spike code |

### Spike Reference Format

```markdown
## Spike Context
- **Spike**: <question answered>
- **Result**: YES/NO/PARTIAL
- **Key learnings**:
  - <learning 1>
  - <learning 2>
- **Code path**: conductor/spikes/<track-id>/spike-xxx/
```

Workers use this reference to:
1. Copy validated patterns from spike code
2. Avoid re-discovering edge cases
3. Apply proven approach to production code

## File Scope Requirements

Each bead **MUST** have explicit file scope for:
1. **File reservation** - prevents conflicts between parallel workers
2. **Track assignment** - enables orchestrator to group beads by file scope
3. **Blast radius** - identifies impact of changes

### Rules

| Rule | Enforcement |
|------|-------------|
| Explicit file scope | Required for all beads |
| No overlapping scopes | Between parallel beads |
| Wildcards allowed | For related files (e.g., `src/api/*.ts`) |
| Scope matches reservations | Workers reserve exact scope |

### File Scope Format

```markdown
## File Scope
- `src/api/users.ts` (primary)
- `src/types/user.ts` (types)
- `tests/api/users.test.ts` (tests)
```

### Anti-Patterns

| ❌ Don't | ✅ Do |
|----------|-------|
| `src/**/*` (too broad) | `src/api/users.ts` (specific) |
| No file scope | Always specify files |
| Overlapping scopes in parallel beads | Sequence or merge beads |
| Forget test files | Include test file scope |

## bd create Examples

### Basic Bead Creation

```bash
bd create "Implement user authentication endpoint" \
  --parent <epic-id> \
  --priority 1 \
  --description "Context: OAuth2 flow validated in spike-001..."
```

### With Spike Learnings

```bash
bd create "Implement Stripe webhook handler" \
  --parent billing-epic \
  --priority 0 \
  --description "Context: Spike bd-12 validated Stripe SDK approach.

## Learnings from Spike
- Must use stripe.webhooks.constructEvent() for signature verification
- Webhook secret stored in STRIPE_WEBHOOK_SECRET env var
- Raw body required (not parsed JSON)

## File Scope
- src/api/webhooks/stripe.ts
- src/handlers/stripe-events.ts
- tests/api/webhooks/stripe.test.ts

## Acceptance Criteria
- [ ] Webhook endpoint at /api/webhooks/stripe
- [ ] Signature verification implemented
- [ ] Events: checkout.session.completed, invoice.paid"
```

### With Dependencies

```bash
# Create parent task first
bd create "Setup webhook infrastructure" --parent <epic> -t task

# Create dependent task
bd create "Implement Stripe webhooks" \
  --parent <epic> \
  --blocks <infrastructure-task-id> \
  --description "Depends on webhook infrastructure setup..."
```

## fb (file-beads) Integration

The `fb` command automates bead creation from plan.md:

```bash
fb  # File beads from plan.md
```

### What fb Does

1. Parses `plan.md` task list
2. Creates beads with:
   - Title from task
   - Description with spike learnings (if Section 5 exists)
   - File scope from task context
   - Dependencies from task ordering
3. Links beads to epic
4. Enables track assignment for orchestration

### fb Output

```
📋 Filing beads from plan.md...

Created:
  ✓ my-workflow:3-xyz.1 - Setup webhook infrastructure
  ✓ my-workflow:3-xyz.2 - Implement Stripe webhooks [blocks .1]
  ✓ my-workflow:3-xyz.3 - Add webhook event handlers [blocks .2]

Dependencies:
  .2 → .1 (blocks)
  .3 → .2 (blocks)

Run `rb` to review filed beads.
```

## Track Assignment

After `fb`, beads are ready for track assignment:

| File Scope Pattern | Track Assignment |
|-------------------|------------------|
| `src/api/*` | API Track |
| `src/lib/*` | Library Track |
| `tests/*` | Test Track (or merged with implementation) |
| Non-overlapping scopes | Parallel tracks |
| Overlapping scopes | Sequential within track |

### Track Assignment Rules

1. **No overlapping file scopes** between parallel beads
2. **Sequence beads** that touch same files
3. **Group by directory** for track assignment
4. **Include tests** in same track as implementation

## Decomposition Checklist

Before running `fb`:

- [ ] plan.md has numbered task list
- [ ] Each task has clear scope
- [ ] Spike learnings captured in design.md Section 5
- [ ] File scopes identified for each task
- [ ] Dependencies clear from task ordering
- [ ] No overlapping scopes between parallel tasks

After running `fb`:

- [ ] Review beads with `rb`
- [ ] Verify dependencies correct
- [ ] Confirm file scopes accurate
- [ ] Ready for track assignment

## Related

- [spikes.md](spikes.md) - Spike workflow and learnings capture
- [pipeline.md](pipeline.md) - Full planning pipeline
- [tracking/FILE_BEADS.md](../../../tracking/references/FILE_BEADS.md) - fb command reference
- [tracking/REVIEW_BEADS.md](../../../tracking/references/REVIEW_BEADS.md) - rb command reference
- [orchestrator/auto-routing.md](../../../orchestrator/references/auto-routing.md) - Track assignment
