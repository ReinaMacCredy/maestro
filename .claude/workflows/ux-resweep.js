export const meta = {
  name: 'ux-resweep',
  description: 'Re-sweep maestro UX after the 16-finding fix batch: exercise real user+agent scenarios on the rebuilt binary, adversarially verify, classify BUG/POLISH/COSMETIC',
  phases: [
    { title: 'Sweep', detail: 'group + new-angle finders exercise the rebuilt binary in throwaway repos' },
    { title: 'Verify', detail: 'one adversarial verifier per candidate, refute-by-default' },
  ],
}

const BIN = '/Users/reinamaccredy/Code/maestro/target/debug/maestro'
const REPO = '/Users/reinamaccredy/Code/maestro'

// The 16 findings already fixed this session. Finders MUST NOT re-report these;
// they may re-run them to confirm the fix held, but their job is NEW issues.
const ALREADY_FIXED = `
ALREADY FIXED (do NOT re-report; you may confirm they hold but hunt for NEW issues):
- B1: claims added to a task AFTER it was verified are now marked "(unverified)" in \`task show\`.
- B2: \`task block\` on a done task (verified/rejected/abandoned/superseded) is refused.
- B3: \`task supersede X --by X\` (self-supersede) is refused.
- B4: resolving an already-resolved blocker is refused.
- B5: \`feature set\` on a terminal (shipped/cancelled) feature no longer suggests the dead-end \`feature amend\`.
- B6: \`harness apply\` on a measured BEHAVIORAL item no longer points at a dead-end \`harness list\` re-derive.
- B7: install->install->uninstall of a maestro-created mirror no longer leaves an empty husk.
- P1: empty/whitespace \`--claim\` on \`task update\` is rejected (matches \`task complete\`).
- P2: \`task show\` with empty MAESTRO_CURRENT_TASK gives "task id is required", not "invalid task id" or a leaked VarError.
- P3: \`task archive\` remedy on a live task lists "reject, abandon, supersede, or verify".
- P4: \`feature archive\` with neither id nor --shipped says "provide a feature id or --shipped" (no "not both").
- P5: \`feature ship --dry-run\` says "qa-baseline skipped (no behavioral scenarios)" when the baseline declares zero scenarios.
- P6: \`decision show\` help reads "(decision-NN)" (a bare NN never resolved; the help no longer claims it does).
- P7: \`query proof\` renders verified_at as an RFC3339 instant, matching \`task show\`.
- P8: a task lookup against a missing .maestro/tasks dir reports "task not found", not a raw ENOENT path.
`

const FINDER_RULES = `
You are auditing maestro, a passive local-first Rust CLI for task/feature/decision/harness lifecycles. State lives under .maestro/.
The binary is already built at ${BIN}. Source is at ${REPO}/src.

METHOD (real execution, not speculation):
- Create throwaway repos under a fresh mktemp -d, run \`git init -q\`, then \`${BIN} init --yes\`.
- Drive REAL command sequences end-to-end. Capture stdout, stderr, AND exit code for every invocation (run each, then echo "exit=$?").
- A finding must be REPRODUCIBLE: give the exact command sequence from a fresh repo and the observed vs expected output.
- Confirm the cause in SOURCE: read the relevant src/ file and cite file:line. Do not report a finding you cannot localize in source.
- IMPORTANT: this shell's rg/grep DISPLAY mangles long identifiers to "n" — use sed/Read for accurate identifier reading, never trust a grep echo of a long symbol.

WHAT COUNTS AS A UX ISSUE (a real user or agent driving maestro hits it):
- Wrong/misleading output: a success message that overstates, an error whose remedy names a path that does not exist or omits a real one, a leaked internal error (io/serde/VarError) instead of a clean message.
- Inconsistent behavior across sibling verbs (one verb guards/validates, its peer does not, with no design reason).
- A dead-end remedy: an error that tells the user to run a command that cannot fix the situation.
- Help text that contradicts actual behavior.
- A confusing or silent outcome where the user cannot tell what happened.

WHAT IS NOT A FINDING:
- By-design behavior (e.g. the QA "C-skip" for a baseline with zero scenarios is correct; re-verify demotion of a task is intentional).
- Pure internal code smell with no user-facing effect.
- Anything in the ALREADY FIXED list.

${ALREADY_FIXED}

Return STRICT JSON only.
`

const FINDING_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['findings'],
  properties: {
    findings: {
      type: 'array',
      items: {
        type: 'object',
        additionalProperties: false,
        required: ['title', 'severity', 'repro', 'observed', 'expected', 'source', 'why_it_matters'],
        properties: {
          title: { type: 'string' },
          severity: { type: 'string', enum: ['BUG', 'POLISH', 'COSMETIC'] },
          repro: { type: 'string', description: 'exact command sequence from a fresh repo' },
          observed: { type: 'string' },
          expected: { type: 'string' },
          source: { type: 'string', description: 'file:line of the cause' },
          why_it_matters: { type: 'string', description: 'the real user/agent who hits it' },
        },
      },
    },
  },
}

const VERDICT_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['verdict', 'severity', 'reason'],
  properties: {
    verdict: { type: 'string', enum: ['CONFIRMED', 'PLAUSIBLE', 'REFUTED'] },
    severity: { type: 'string', enum: ['BUG', 'POLISH', 'COSMETIC'] },
    reason: { type: 'string', description: 'quote the line/output that proves the verdict' },
  },
}

const GROUPS = [
  { key: 'task-lifecycle', focus: 'task create/set/explore/accept/claim/complete/verify/update/show/list (+ all --filters). Drive draft->exploring->ready->in_progress->needs_verification->verified and every rejected/abandoned/superseded terminal. Probe odd args, repeated verbs, out-of-order transitions, and the exact wording of every error/remedy.' },
  { key: 'task-blockers-terminal-archive', focus: 'task block/unblock, blocker resolution, the blocker graph, doctor, and archive/unarchive. Probe blocking across states, self/cyclic blockers, double unblock, archive of live vs done vs blocked-referent tasks, and round-trip archive->unarchive.' },
  { key: 'feature-lifecycle', focus: 'feature create/accept/start/amend/set/ship/cancel/archive/unarchive and the QA baseline/slice gates. Drive proposed->ready->in_progress->shipped/cancelled. Probe ship gates (missing baseline, zero-scenario baseline, uncovered scenario, stale amend), terminal-edit guards, and child-task cascade on archive.' },
  { key: 'decision-and-query', focus: 'decision new/show/list and all query views (proof, etc). Probe id resolution (canonical, .md, prefix, bare NN, missing, ambiguous), and every query subcommand on empty/partial/rich state. Check help vs behavior for the whole decision + query surface.' },
  { key: 'harness-and-event', focus: 'harness list/apply/measure and event verbs. Drive the proposed->accepted->measured backlog lifecycle for BOTH state-detector and behavioral item types. Probe apply/measure on every status, force vs not, and event creation as a verify proof source.' },
  { key: 'install-lifecycle', focus: 'install/update/uninstall/status across claude+codex agents, greenfield+brownfield, single + repeated installs. Probe mirror ownership, husk cleanup, --check/--force/--verbose update flags, and what status reports in each state.' },
]

const NEW_ANGLES = [
  { key: 'message-consistency', focus: 'Audit error/remedy/success WORDING uniformly across EVERY command. Build a table: for each verb, does its guard message name a remedy that exists and is complete? Are sibling verbs consistent (one validates X, does its peer)? Are success messages truthful about what was actually checked/done? Hunt for the next P3/P4/P5-class wording wart.' },
  { key: 'cross-verb-interactions', focus: 'Audit SECOND-ORDER interactions across subsystems: feature<->child-task linkage through ship/cancel/archive; supersede chains and what task show / list display afterward; archive then operate-on-archived; harness item whose spawned task is then archived/rejected; decision referenced by a task. Look for states reachable by a real sequence that produce confusing or inconsistent output.' },
  { key: 'help-and-discoverability', focus: 'Run --help on the root and EVERY subcommand. Compare each flag/arg help string and each "about" against the actual implemented behavior in src/. Hunt for the next P6/P9-class gap: undocumented flags, help that claims an unsupported input form, stale examples, or a verb whose help misdescribes what it does.' },
  { key: 'fresh-and-empty-states', focus: 'Audit the cold-start and empty-state UX: every list/show/query/status/doctor/harness verb run in a JUST-init repo with zero tasks/features/decisions, and in a repo where .maestro subdirs are absent. Hunt for the next P8-class leak (raw io/serde error, abs-path leak, panic, or an empty output that leaves the user unsure whether it worked).' },
]

phase('Sweep')

const groupFindings = await parallel(
  GROUPS.map((g) => () =>
    agent(
      `${FINDER_RULES}\n\nSCENARIO GROUP: ${g.key}\nFOCUS: ${g.focus}\n\nExercise this group exhaustively on the rebuilt binary in throwaway repos. Surface up to 8 NEW candidate UX issues (not in the ALREADY FIXED list). Quality over quantity: every finding must have a real repro + source citation. If the group is clean, return an empty findings array.`,
      { label: `find:${g.key}`, phase: 'Sweep', schema: FINDING_SCHEMA },
    ),
  ),
)

const angleFindings = await parallel(
  NEW_ANGLES.map((a) => () =>
    agent(
      `${FINDER_RULES}\n\nNEW-ANGLE AUDIT: ${a.key}\nFOCUS: ${a.focus}\n\nThis is a cross-cutting audit, not a single group. Surface up to 8 NEW candidate UX issues. Every finding needs a real repro + source citation. If clean, return an empty findings array.`,
      { label: `angle:${a.key}`, phase: 'Sweep', schema: FINDING_SCHEMA },
    ),
  ),
)

const candidates = [...groupFindings, ...angleFindings]
  .filter(Boolean)
  .flatMap((r) => r.findings || [])

log(`Sweep surfaced ${candidates.length} candidate findings; verifying each adversarially.`)

if (candidates.length === 0) {
  return { candidates: 0, confirmed: [], note: 'Sweep found zero candidates. UX is clean across the audited scenarios.' }
}

phase('Verify')

const verified = await parallel(
  candidates.map((c, i) => () =>
    agent(
      `You are an ADVERSARIAL verifier for a maestro UX finding. Default to REFUTED unless you can reproduce it yourself.\n\nThe binary is at ${BIN}; source at ${REPO}/src. Create your own throwaway repo and RUN the repro. Read the cited source. Do NOT trust the finder.\n\nCANDIDATE:\ntitle: ${c.title}\nseverity claimed: ${c.severity}\nrepro: ${c.repro}\nobserved: ${c.observed}\nexpected: ${c.expected}\nsource: ${c.source}\n\nReturn:\n- CONFIRMED only if you reproduced the wrong/misleading/leaked output yourself AND localized it in source. Quote the proving line.\n- PLAUSIBLE if the mechanism is real but you could not fully trigger it (state why).\n- REFUTED if it is by-design, already-correct, in the already-fixed set, or you could not reproduce it. Quote the line that proves it.\nAlso assign the TRUE severity (BUG = wrong behavior/data/guard; POLISH = misleading-but-harmless wording/inconsistency; COSMETIC = trivial formatting).\n\n${ALREADY_FIXED}`,
      { label: `verify:${i}:${c.severity}`, phase: 'Verify', schema: VERDICT_SCHEMA },
    ).then((v) => (v ? { ...c, verdict: v } : null)),
  ),
)

const survived = verified.filter(Boolean).filter((c) => c.verdict.verdict !== 'REFUTED')
const confirmed = survived.filter((c) => c.verdict.verdict === 'CONFIRMED')
const plausible = survived.filter((c) => c.verdict.verdict === 'PLAUSIBLE')

return {
  candidates: candidates.length,
  refuted: candidates.length - survived.length,
  confirmed: confirmed.map((c) => ({ title: c.title, severity: c.verdict.severity, repro: c.repro, source: c.source, reason: c.verdict.reason })),
  plausible: plausible.map((c) => ({ title: c.title, severity: c.verdict.severity, source: c.source, reason: c.verdict.reason })),
}
