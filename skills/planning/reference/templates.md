# Templates — Feature Planning Pipeline

Document templates used across planning phases. Copy and fill in when creating artifacts.

## Discovery Report

Save to `.maestro/drafts/{topic}-discovery.md`.

```markdown
# Discovery Report: <Feature Name>

## Architecture Snapshot

- Relevant packages: ...
- Key modules: ...
- Entry points: ...

## Existing Patterns

- Similar implementation: <file> does X using Y pattern
- Reusable utilities: ...
- Naming conventions: ...

## Technical Constraints

- Node version: ...
- Key dependencies: ...
- Build requirements: ...

## External References

- Library docs: ...
- Similar projects: ...
```

## Approach Document

Save to `.maestro/drafts/{topic}-approach.md`.

```markdown
# Approach: <Feature Name>

## Gap Analysis

| Component | Have | Need | Gap |
| --------- | ---- | ---- | --- |
| ...       | ...  | ...  | ... |

## Recommended Approach

<Description>

### Alternative Approaches

1. <Option A> - Tradeoff: ...
2. <Option B> - Tradeoff: ...

## Risk Map

| Component   | Risk | Reason           | Verification |
| ----------- | ---- | ---------------- | ------------ |
| Stripe SDK  | HIGH | New external dep | Spike        |
| User entity | LOW  | Follows existing | Proceed      |
```

## Spike Bead

```markdown
# Spike: <specific question>

**Time-box**: 30 minutes
**Output location**: .spikes/<spike-id>/

## Question

Can we <specific technical question>?

## Success Criteria

- [ ] Working throwaway code exists
- [ ] Answer documented (yes/no + details)
- [ ] Learnings captured for main plan

## On Completion

Close with: `br close <id> --reason "YES: <approach>" or "NO: <blocker>"`
```

## Bead with Learnings

```markdown
# Implement Stripe webhook handler

## Context

Spike br-12 validated: Stripe SDK works with our Node version.
See `.spikes/billing-spike/webhook-test/` for working example.

## Learnings from Spike

- Must use `stripe.webhooks.constructEvent()` for signature verification
- Webhook secret stored in `STRIPE_WEBHOOK_SECRET` env var
- Raw body required (not parsed JSON)

## Acceptance Criteria

- [ ] Webhook endpoint at `/api/webhooks/stripe`
- [ ] Signature verification implemented
- [ ] Events: `checkout.session.completed`, `invoice.paid`
```

## Execution Plan

Save to `.maestro/drafts/{topic}-execution-plan.md`.

```markdown
# Execution Plan: <Feature Name>

Epic: <epic-id>
Generated: <date>

## Tracks

| Track | Agent       | Beads (in order)        | File Scope        |
| ----- | ----------- | ----------------------- | ----------------- |
| 1     | BlueLake    | br-10 → br-11 → br-12  | `packages/sdk/**` |
| 2     | GreenCastle | br-20 → br-21           | `packages/cli/**` |
| 3     | RedStone    | br-30 → br-31 → br-32  | `apps/server/**`  |

## Track Details

### Track 1: BlueLake - <track-description>

**File scope**: `packages/sdk/**`
**Beads**:

1. `br-10`: <title> - <brief description>
2. `br-11`: <title> - <brief description>
3. `br-12`: <title> - <brief description>

### Track 2: GreenCastle - <track-description>

**File scope**: `packages/cli/**`
**Beads**:

1. `br-20`: <title> - <brief description>
2. `br-21`: <title> - <brief description>

### Track 3: RedStone - <track-description>

**File scope**: `apps/server/**`
**Beads**:

1. `br-30`: <title> - <brief description>
2. `br-31`: <title> - <brief description>
3. `br-32`: <title> - <brief description>

## Cross-Track Dependencies

- Track 2 can start after br-11 (Track 1) completes
- Track 3 has no cross-track dependencies

## Key Learnings (from Spikes)

Embedded in beads, but summarized here for orchestrator reference:

- <learning 1>
- <learning 2>
```
