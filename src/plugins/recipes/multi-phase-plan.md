# Multi-phase plan

Use this recipe for work that spans phases, a run of dependent commits, or
stacked PRs. You own the plan, not the code. The plan is a checklist that its
runner works unit by unit and the owner audits from the evidence. The plan is
the deliverable, so do not implement. It lives in a Full bundle, which keeps
the store records, the handoff, the close and the search the bundle contract
gives it (`~/maestro/WORKFLOW.md#bundle-contract-tier-full`). Ported from
pstack's multi-phase-plan playbook (Lauren Tan, MIT; notice in THIRD_PARTY.md).

## Steps

1. Skip the plan when the change touches one or two files with an obvious
   approach. Say so and stop. That change runs through its own recipe, such
   as `maestro recipe show feature` or `maestro recipe show bug-fix`.
2. Settle open questions by prototype before you write, one
   `maestro recipe show prototype` run per question. Keep each prototype's
   branch, SHA and screenshots for step 5. Ask the owner only about a product
   or preference call that no run can settle, and give options, per
   **Never Block on the Human** (PRINCIPLES.md).
3. Explore in subagents under the `explorer` profile, per
   **Guard the Context Window** (PRINCIPLES.md). Each returns file pointers,
   conventions, test commands and entry points, never an inlined dump.
4. Open the bundle with `maestro bundle open <id> --work <work-id>`. It
   scaffolds SPEC.md with a `## Units` section and VERIFY.md with the unit
   table.
5. Write SPEC.md. Fill the bundle contract's sections, then `## Units`.
   - Open `## Units` with one sentence on how the units run. They run in
     sequence from one session, as parallel sub-agents on disjoint paths, or
     as an SLP team for a standing program (`maestro recipe show slp`). The
     same sentence names the review-gated units. Merge stays the owner's gate.
   - Write one `### U<n>: <task as a verb phrase>` block per unit, in
     dependency order. One unit is one change with its own evidence. It lands
     as one commit, or as one PR under the delivery grant
     (`maestro recipe show opening-a-pr`), per
     **Sequence Work into Verifiable Units** (PRINCIPLES.md).
   - `Depends on:` names the units it waits for, or none.
   - `Files:` lists each path as edit, create or delete. Units that run in
     parallel own disjoint paths.
   - `Build:` is the one change, naming the symbol and the file.
   - `You will see:` is one observable result once the unit lands, with the
     exact output line or screen state.
   - A unit that changes an interaction is review-gated. The owner reviews it
     before merge, with screenshots and a short video of the change.
   - Prototype evidence from step 2 goes under `## Derived artifacts`. Name
     each question a prototype answered, with its branch, SHA and artifact
     path, and each question that stays unproven. A rejected alternative
     lives in its decision's rationale. Each risk goes under a `## Risks`
     heading, with the unit it lands in and what the owner watches.
6. Write VERIFY.md, one row per unit with its Scenario naming the unit id, plus
   one row per anti-goal. Tests alone are not sufficient verification. A unit
   is verified only when its unit, live and perf cells each hold evidence or
   `n/a: <reason>`, and its Commit cell holds the commit.
   - The unit cell names the test file, the case it gains, and the command
     that runs it.
   - The live cell holds lanes sized by risk, not a fixed count, each driven
     through the repository's verify-<repo> skill
     (`maestro recipe show verification-skill`). A lane names a concrete
     scenario, the capture it saves under `maestro topic path <slug>`, and its
     pass predicate. One lane is the regression lane against the base branch.
     It runs the same load-bearing scenario at the base and at the unit's
     head. When the base lacks the feature, the lane records that and gates
     the behavior the diff adds plus the end state the user waits for,
     instead of inventing a base result. A unit that touches two interfaces,
     such as a CLI and a web UI, gets lanes on both. An interface the
     verify-<repo> skill cannot drive is a risk, and the live cell still names
     how each lane drives it.
   - The perf cell is dual-sided. The base and the head both produce the named
     metric. When the base lacks the feature, isolate the work the diff adds
     and set an absolute budget for it and for the end state the user waits
     for. Never claim a ratio between unlike scenarios. The cell names the
     metric, the interleaved probe, the base value measured first, and the
     rule with the number that fails. Before any number is reported, run each
     side at least five times alternating, count errors, and name the limiter.
     A unit with no runtime cost writes `n/a: <reason>`.
7. Write the plan in short, plain sentences with real symbol and file names,
   no filler and no em dashes. The body is a how-to, and each heading states
   the task or the finding.
8. Run `maestro bundle check <id>` and fix every line it prints, per
   **Encode Lessons in Structure** (PRINCIPLES.md).
9. Hand back. Post the bundle id and the check's output, then stop. Execution
   starts only on the owner's explicit go
   (`~/maestro/WORKFLOW.md#authorization-boundaries`). Each unit then runs
   through its own recipe and ends in that recipe's deliver step. At verify,
   `maestro bundle check <id> --final` lints the filled table before
   maestro-verify closes the bundle.

## Reply

The bundle id. The units with their dependencies, and the review-gated set.
What the prototypes proved and what stays unproven. The output of
`maestro bundle check <id>`.
