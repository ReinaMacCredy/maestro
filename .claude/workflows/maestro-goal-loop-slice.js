export const meta = {
  name: 'maestro-goal-loop-slice',
  description:
    'Resume one Maestro architecture-refactor plan slice: plan -> implement waves -> simplify+swarm review. Sub-agents edit but never stage/commit; the main thread verifies and commits.',
  phases: [
    { title: 'Plan', detail: 'Opus agent decomposes the slice into a dependency-tagged wave graph' },
    { title: 'Implement', detail: 'parallel Opus agents apply edits, one wave at a time' },
    { title: 'Review', detail: '4 /simplify + 4 review-swarm read-only reviewers over the slice diff' },
  ],
}

// args = {
//   taskId:   plan task id, e.g. "5.5"
//   title:    human title of the task
//   planPath: absolute path to PLAN-maestro-architecture-refactor.md
//   notesPath:absolute path to IMPLEMENTATION_NOTES.md (prior decisions/invariants)
//   baseRef:  git ref the slice started from (diff base for review), e.g. "HEAD"
//   implement:boolean (default true). false => Plan + Review only (for verify/audit-style slices).
// }
const slice = args || {}
const taskId = slice.taskId || '6.2'
const title = slice.title || 'Update Source Map And Test Matrix'
const planPath = slice.planPath || '/Users/reinamaccredy/Code/maestro/PLAN-maestro-architecture-refactor.md'
const notesPath = slice.notesPath || '/Users/reinamaccredy/Code/maestro/IMPLEMENTATION_NOTES.md'
const baseRef = slice.baseRef || '9b2e95ac'
const doImplement = slice.implement !== false

const PLAN_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  properties: {
    waves: {
      type: 'array',
      items: {
        type: 'object',
        additionalProperties: false,
        properties: {
          name: { type: 'string' },
          tasks: {
            type: 'array',
            items: {
              type: 'object',
              additionalProperties: false,
              properties: {
                id: { type: 'string' },
                instruction: { type: 'string' },
                writes: { type: 'array', items: { type: 'string' } },
              },
              required: ['id', 'instruction', 'writes'],
            },
          },
        },
        required: ['name', 'tasks'],
      },
    },
    validation: { type: 'array', items: { type: 'string' } },
    notes: { type: 'string' },
  },
  required: ['waves', 'validation'],
}

const FINDINGS_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  properties: {
    findings: {
      type: 'array',
      items: {
        type: 'object',
        additionalProperties: false,
        properties: {
          file: { type: 'string' },
          locator: { type: 'string', description: 'line number or symbol' },
          summary: { type: 'string' },
          severity: { type: 'string', enum: ['high', 'medium', 'low'] },
          confidence: { type: 'string', enum: ['high', 'medium', 'low'] },
          rationale: { type: 'string', description: 'concrete cost / why it matters' },
          suggested_fix: { type: 'string' },
          may_be_intended: {
            type: 'boolean',
            description: 'true if it may collide with a documented deliberate invariant',
          },
        },
        required: ['file', 'summary', 'severity', 'confidence', 'rationale', 'may_be_intended'],
      },
    },
    notes: { type: 'string' },
  },
  required: ['findings'],
}

const STYLE =
  'Follow the repo Rust style: private by default, explicit re-exports from facades, no panic paths for user input, ' +
  'no unwrap() outside tests unless justified by an invariant-specific expect, keep adapters thin. Match existing code style. ' +
  'Keep durable artifact writes routed through the owning domain facade or a documented migration-only exception. ' +
  'Operations must not depend on Interfaces. Keep foundation/core domain-neutral.'

// ---- Plan -------------------------------------------------------------------
phase('Plan')
const plan = await agent(
  `You are the goal-loop planner for the Maestro architecture refactor, Task ${taskId} "${title}".

Read the plan task in ${planPath} (locate the "### Task ${taskId}" section) plus that file's Global Constraints, and read the prior decisions/invariants in ${notesPath} (Phase ${taskId.split('.')[0]} section). Inspect the live tree with rg, file reads, and \`git log --oneline -6\` so the plan matches reality, not the doc's aspiration.

Decompose ONLY Task ${taskId} into an ordered list of waves:
- A wave is a set of subtasks that can run in parallel.
- HARD RULE: no two subtasks in the same wave may write the same file (disjoint \`writes\`).
- Order waves by dependency (later waves may depend on earlier waves).
- Each subtask: a concrete, self-contained instruction and the exact relative file paths it writes.
- Stay strictly inside Task ${taskId}'s declared Writes scope. Do not widen scope or invent abstractions.

Also return the task's Validation commands verbatim from the plan. ${STYLE}`,
  { label: `plan:${taskId}`, phase: 'Plan', schema: PLAN_SCHEMA },
)

// Guard the disjoint-writes invariant; surface violations to the main thread.
const writeConflicts = []
for (const w of plan.waves || []) {
  const seen = new Map()
  for (const t of w.tasks || []) {
    for (const f of t.writes || []) {
      if (seen.has(f)) writeConflicts.push(`wave "${w.name}": ${t.id} and ${seen.get(f)} both write ${f}`)
      else seen.set(f, t.id)
    }
  }
}
if (writeConflicts.length) log(`PLAN WARNING - same-wave write conflicts: ${writeConflicts.join('; ')}`)

// ---- Implement --------------------------------------------------------------
const implemented = []
if (doImplement) {
  phase('Implement')
  for (const w of plan.waves || []) {
    const waveResults = await parallel(
      (w.tasks || []).map((t) => () =>
        agent(
          `Implement Maestro Task ${taskId} subtask ${t.id} (wave "${w.name}").

Instruction: ${t.instruction}

You may edit ONLY these files: ${(t.writes || []).join(', ') || '(none declared - do not edit source; report instead)'}.
${STYLE}

After editing, run \`cargo check --all-targets\` and fix until it compiles. Do NOT run the full test suite, fmt, clippy, and do NOT stage or commit - the main thread owns verification and git. Return a concise summary: files changed and what you did.`,
          { label: `impl:${t.id}`, phase: 'Implement', model: 'opus' },
        ),
      ),
    )
    implemented.push(...waveResults.filter(Boolean))
  }
}

// ---- Review -----------------------------------------------------------------
const HOW_TO_INSPECT = `The slice change is the diff of the working tree vs \`${baseRef}\`. Inspect it with \`git diff ${baseRef} --stat\` then \`git diff ${baseRef} -- <path>\` per file, and read full files for context.

CONTEXT - deliberate invariants: the proof/attempt-selection and harness-backlog surfaces encode legacy-compatibility rules resolved across prior review rounds, documented in ${notesPath}. Read the relevant section before judging proof/attempt-selection, verification-binding, or backlog-evidence behavior. If a finding touches those rules, set may_be_intended=true and explain how it diverges from the documented invariant.

STRICT READ-ONLY: do not edit, write, patch, stage, commit, or run cargo build/test. Use only git diff, rg, and file reads.`

const SIMPLIFY = [
  ['Reuse', 'new code that re-implements an existing helper; name the helper to call instead'],
  ['Simplification', 'redundant/derivable state, copy-paste variation, deep nesting, dead code; name the simpler form'],
  ['Efficiency', 'redundant computation or repeated IO, needless sequential work, hot-path work; name the cheaper alternative'],
  ['Altitude', 'special cases layered on shared infra; prefer generalizing the mechanism over a bandaid'],
]
const SWARM = [
  ['Intent & Regression', 'behavior drift outside stated scope, broken edges/fallbacks, contract drift, missing adjacent updates'],
  ['Security & Privacy', 'authn/authz gaps, unsafe input handling or injection, secret/path exposure in evidence, risky defaults'],
  ['Performance & Reliability', 'duplicate work/IO, hot-path cost, leaks or missing cleanup, ordering/race/failure handling'],
  ['Contracts & Coverage', 'API/schema/type/config/flag mismatches, migration or back-compat fallout, missing/weak tests, missing logs or metrics'],
]

phase('Review')
const reviewResults = await parallel([
  ...SIMPLIFY.map(([role, brief]) => () =>
    agent(
      `READ-ONLY /simplify "${role}" review of Maestro Task ${taskId} "${title}".\n\n${HOW_TO_INSPECT}\n\nFlag only: ${brief}. This is a cleanup lens, not a correctness-bug hunt. Return structured findings.`,
      { label: `simplify:${role}`, phase: 'Review', schema: FINDINGS_SCHEMA },
    ),
  ),
  ...SWARM.map(([role, brief]) => () =>
    agent(
      `READ-ONLY review-swarm "${role}" review of Maestro Task ${taskId} "${title}".\n\n${HOW_TO_INSPECT}\n\nCheck for: ${brief}. Treat high/medium as blocking-grade signal. Return structured findings.`,
      { label: `swarm:${role}`, phase: 'Review', schema: FINDINGS_SCHEMA },
    ),
  ),
])

const reviewerLabels = [...SIMPLIFY.map(([r]) => `simplify:${r}`), ...SWARM.map(([r]) => `swarm:${r}`)]
const findings = reviewResults
  .map((r, i) => ({ reviewer: reviewerLabels[i], result: r }))
  .filter((x) => x.result)

return { taskId, title, baseRef, plan, writeConflicts, implemented, findings }
