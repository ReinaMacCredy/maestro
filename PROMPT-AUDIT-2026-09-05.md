# Prompt audit 2026-09-05: maestro's model-facing text

Applied as w673 on 2026-09-05: F1 in `a940f980` with decision d843 superseding
d839, F3 in `7cd9a870`, F2 and F4 to F9 in the commit that adds this file.
Flags F10 to F22 stay flags. The report below is the phase-1 audit as delivered,
with its scratch paths (`/tmp/prompt-audit/...`) left as written; the probe
prompts and raw outputs were not checked in.

Audit date 2026-09-05. Repository `~/Code/maestro` at `40494a6b` (release v0.119.1);
the working tree also carried another builder's uncommitted edits to SLP.md,
slp-runtime.ts, slp-v2.ts, work.ts and tests/slp-v2.test.ts, none of which this
audit touches. Phase 1 was read-only: nothing under the repo was edited, staged or
committed. Deliverables live under `/tmp/prompt-audit/`.

## Scope and target assumptions

Targets, from the owner's brief: every finding is judged against Claude Opus 5
(the shipped Lead and Peer seat model), Claude Sonnet 5 (the owner's live-test
seat and the cheapest node-profile target) and Claude Fable 5.1 (the owner's
builder and Lead panes). Where the targets pull apart the proposed text fits all
three, or Sonnet 5 when they conflict. Codex reads the same profile bodies and
SLP.md; each entry says whether the Codex reading changes.

Scope is the prompt surface the brief listed, verified and extended below. Out of
scope: CLI help strings, test files (except where a hunk changes a pinned
string), the docs site, Codex TOML rendering, `~/.claude/CLAUDE.md`, the owner's
private skills, `~/Code/cmux`, `room.ts`, `.claude/skills/maestro-lifecycle-test/`.

**Provenance frame.** Every emphatic or prohibitive line blamed traces to commits
between 2026-08-23 and 2026-09-05, most of them to 2026-09-05 itself. Nothing in
this corpus was written for a retired model, so idiom-dating cannot reach medium
confidence anywhere. Every medium or high finding below rests on one of two
things: two prompt surfaces that disagree with each other, or a Sonnet 5
literal-reading harm that the migration guide documents.

## Inventory (Step 1)

| Surface | Files | Lines | How it reaches the model |
|---|---|---|---|
| Seat and node profiles | 17 in `src/plugins/resources/profiles/` | 279 | body rendered as the agent system prompt by `maestro install` (`~/.claude/agents/maestro-*.md`, 32 files; Codex `developer_instructions`) |
| SLP pack and workflow | `resources/SLP.md`, `resources/WORKFLOW.md` | 285 | shared contract prepended to every seat; WORKFLOW read on first prompt |
| Recipes | 19 in `src/plugins/recipes/` (slp, work, design, audit, ship, learning, unattended, worktree, conflict-handoff, 10 style-*) | 662 | `maestro recipe show` |
| Skills | 11 SKILL.md plus 18 references in `src/plugins/skills/` | 2470 | loaded on trigger |
| Graph presets | `graphs/council.md`, `fix-loop.md`, `review-gate.md` | 752 | node sections become agent briefs |
| Brief composer | `graph.ts` `briefOf`, `graph-file.ts` `schemaKeySentence` | | prepends the d839 key sentence and the schema block to every agent node brief |
| Hook-emitted text | `slp-v2.ts` (READY challenge, `[from <role>][<id> STATE]` push), `slp-attention.ts` (`[from runtime]` stall line, `[attention]` lines), `coordination.ts` + `recipe.ts` + `memory.ts` + `attention.ts` (SessionStart / UserPromptSubmit brief block), `dispatch.ts` (dispatch and handback record text) | | stdout of the SessionStart / UserPromptSubmit hooks and the SLP push path |
| MCP tool descriptions | `mcp.ts` (`maestro_find`, `maestro_run`) | 2 tools | tool list; `instructions` = the SessionStart brief |
| Rendered install (read only) | `~/.claude/agents/maestro-lead.md`, `maestro-peer.md`, `maestro-peer-opus.md` | | the final shape; the owner's `~/maestro/profiles/{lead,peer}.md` shadow the shipped seats with `harness: claude`, `model: opus`, `effort: high`, `autocompact: 250000` and decision-cited descriptions |

Extensions found while inventorying: `recipe.ts` and `attention.ts` register
brief lines the brief did not list; `brief.ts` itself prints nothing
model-facing beyond re-rendering the SessionStart brief; `lifecycle.ts` prints
only operator error strings (out of scope as CLI text); `dispatch.ts`'s only
model-facing prose is the record formatter and one error hint.

## Counts

| Group | High | Medium | Low / flag |
|---|---|---|---|
| 1 dated prompt text (1a to 1f) | 1 (F3) | 4 (F4, F5, F6, F8) | 7 (F10, F12, F13, F14, F16, F18, F19) |
| 2 brittle skill files | 0 | 1 (F7) | 4 (F11, F15, F17, F20) |
| 3 tool descriptions | 0 | 1 (F9) | 0 |
| 4 request config and architecture | 0 | 0 | 2 (F21, F22; F11 also) |
| cross-surface contradiction (two surfaces disagree) | 1 (F1) | 1 (F2) | 0 |

Nine findings carry a hunk (`proposed.diff`, one hunk per finding, all applying
cleanly with `git apply --check` from the repo root against `40494a6b`); two are
high (F1, measured on Sonnet; F3, a documented pattern with token cost as the harm),
seven are medium.
Thirteen are flags with no edit. Every test suite that pins an edited string was run on an
exported copy of HEAD with the full diff applied (see Diff index).

## Highest-impact findings

**The d839 key sentence contradicts the schema it introduces whenever the schema
has optional keys** (high: measured on `claude-sonnet-5`, probe P1b). `schemaKeySentence` says "Return one JSON object with
exactly these keys: position, claims, falsifier" in front of the council report
schema, whose six other properties (recommendation, alternative,
counterargument, failure_mode, unknowns, confidence) the brief's CASE OUTPUT
CONTRACT and the node prompt both ask for. Sonnet 5 follows instructions
literally and does not generalize; "exactly these keys" is an instruction to omit
the rest. The same sentence tells the council verifiers to drop `sources` and
`limitations`. The fix keeps d839's purpose (one plain sentence leading the
block, proven on lab g18) and changes one word's worth of semantics: required and
optional keys are named separately when they differ.

**The verifier and auditor profiles pin a text return format that the council
graph's JSON schema contradicts** (medium: the probe shows Sonnet already prefers the
task brief's JSON over the profile's text block). The system prompt says "Return exactly:
PROPOSITION CHECKED / MANDATE / ...", the node brief says "Answer with one JSON
object ... and nothing else". A schema-validated node with two retries is
exactly where a system-prompt-versus-task-prompt disagreement costs a run. The
profile now names the fields and defers the shape to the brief, keeping the text
block for the hand-run council path in the maestro-council skill.

**The finding stage of the review gate filters on certainty while a refuter
already sits downstream.** reviewer-correctness says "no speculation without a
reproduction path". The Sonnet 5 guide documents that conservative-reporting
instructions depress measured recall while the model's bug-finding is unchanged,
and recommends coverage-first reporting with downstream filtering, which is the
review-gate's own architecture (join, one refuter per finding, synthesizer).

Everything else is small. The shipped corpus has one ALL-CAPS emphasis marker in
total (the other caps hit is a heading in a format template), no "think step by step", no update suppressors, no anti-formatting rules
in seat text, no output cadences, and its prohibitions almost all carry a reason
or a decision id. The hook-emitted lines and the SLP READY script are clean.

## Findings with a hunk

### F1. `schemaKeySentence` announces "exactly these keys" for schemas with optional properties

- **Location**: `src/plugins/graph-file.ts:475`; consumers `src/plugins/graphs/council.md:14-35` (`&report`), `:61-70` (`&verification`), `src/plugins/graphs/review-gate.md:66-75` (nested `lens`).
- **Evidence**: `` `Return one JSON object with exactly these keys: ${required.join(", ")}` `` rendered in front of a schema whose `required` is `[position, claims, falsifier]` but whose `properties` list nine keys; the council brief's CASE OUTPUT CONTRACT and the node prompt ("Role line: ... Type each material claim ...") ask for the optional ones.
- **Pattern**: cross-surface contradiction (keep-list 8: duplicates that disagree); Group 1c near-duplicate rules the model must reconcile.
- **Why obsolete on which target**: Sonnet 5 "interprets prompts literally and explicitly ... does not silently generalize an instruction" (Sonnet 5 § Behavioral shifts). "Exactly these keys" is read as "omit the others". Opus 5 and Fable 5.1 reconcile it more often but still receive three instructions that disagree. d839's rationale (lab g18: sonnet nodes invented field names, returned prose, omitted `evidence`) is about *leading with the shape*, not about excluding optional keys; the review-gate findings schemas have no optional keys, which is why the sentence never bit there.
- **Confidence**: High. The disagreement is verifiable by reading the two files, the Sonnet 5 literalism is documented, and probe P1b measured the loss: with a brief whose CASE OUTPUT CONTRACT does not enumerate the keys, the current wording made `claude-sonnet-5` drop optional report fields in three of three runs (5/6, 2/6 and 5/6 optional keys present; `recommendation` missing every time), while the proposed wording returned all nine keys in three of three. When the brief itself enumerates the keys (probe P1) both wordings returned all nine, so the harm depends on the Lead's brief, which the shipped text cannot rely on.
- **Action**: `rewrite`. When every property is required the sentence is unchanged ("exactly these keys: ..."), so the g18 fix and the two existing tests hold; when the required set is a strict subset it reads "Return one JSON object with the required keys position, claims, falsifier and any of the optional keys recommendation, alternative, ...". Adds one unit test. This amends the literal wording of d839; the Lead should record that as a note on d839 or a superseding decision when applying. Codex reading: same brief, same benefit. Hunk `01-schema-key-sentence.diff`. Probe P1 below.

### F2. verifier.md and auditor.md pin a text return shape the council graph contradicts

- **Location**: `src/plugins/resources/profiles/verifier.md:14-23`; `src/plugins/resources/profiles/auditor.md:16-20`. Contradicted by `src/plugins/graphs/council.md:213-240` (premise, verify nodes: "Answer with one JSON object {proposition, mandate, sources, observations, result, limitations} ... and nothing else") and `:352-392` (auditor node: `{"result": ..., "findings": [...]}`).
- **Evidence**: verifier: "Return exactly:" followed by a six-line text block; auditor: "Return your findings and end with exactly one line: AUDIT RESULT: CLEAR | REVISE | STOP".
- **Pattern**: cross-surface contradiction; system prompt (rendered profile) versus task prompt (node brief) on a schema-validated node.
- **Why obsolete on which target**: on every target the system prompt carries more authority than the task; Sonnet 5 will follow "Return exactly:" literally and the node fails schema intake, costing one of its two retries (d839) before a habit miss even occurs. The text block is still right for the hand-run council in `maestro-council/SKILL.md:150-157` and `:204-205`, so it must survive as the default when the brief names no shape.
- **Confidence**: Medium. The disagreement is verifiable, but probe P2 did not reproduce the predicted failure: with the current "Return exactly: <text block>" system prompt, Sonnet followed the task brief and returned one JSON object with all six keys in three of three runs. The hunk removes the contradiction; a measured benefit on Sonnet was not shown.
- **Action**: `rewrite`. The profile names the six (verifier) or two (auditor) fields and says "in the shape the brief names (a text contract or a JSON schema); when the brief names no shape, return exactly: <text block>". No test pins either string. Codex reading: identical text as `developer_instructions`; the Codex verifier seat gains the same fix. Hunk `02-verifier-auditor-shape.diff`. Probe P2 below.

### F3. Static instruction lines are re-inserted on every UserPromptSubmit

- **Location**: `src/plugins/coordination.ts:262-269` ("enabled policies: ..."), `:273` ("next: maestro ready"), `src/plugins/recipe.ts:126` ("recipes: maestro recipe list; maestro recipe show <name>").
- **Evidence**: `brief.register(() => "next: maestro ready")` with no `events` option, so `BriefService.render` emits it on SessionStart and on every UserPromptSubmit; the same for the recipes line and the policies line. The method map has been SessionStart-only since 2026-08-25 (`972962a9`, test 14b), which is the precedent this hunk extends.
- **Pattern**: Group 1d, instruction re-insertion every few turns.
- **Why obsolete on which target**: the migration guide lists per-turn re-insertion as a retention crutch that current models do not need; Fable 5.1's guide notes once-stated instructions persist across long sessions. `install.ts:134` writes the SessionStart hook with no matcher, so Claude Code re-fires it on `compact`, `clear` and `resume`; retention across compaction is already covered without the per-turn copy. Cost is one to three lines per turn on every session, Codex included.
- **Confidence**: High as a documented pattern; the harm is token cost, not behavior.
- **Action**: `rewrite` to `{ events: ["SessionStart"] }` for the three static lines. State-bearing lines (held work, live peers, the peer/lead role line, attention findings, the drift advisory) stay on both events. `tests/slp-adoption.test.ts:280-303` (test 137) pins the UserPromptSubmit output byte-for-byte including these three lines, so the hunk carries that test's update; `tests/ux-review.test.ts:116-118`, `tests/phase-four-deletion.test.ts:205` and `tests/session-coordination.test.ts` (14b) assert only SessionStart output or the held-work line and pass unchanged. **Codex call-out**: whether Codex re-fires SessionStart after its own compaction is unknown to this audit; if it does not, a Codex session loses these three lines after compaction. They are all reachable by `maestro help`, so the loss is minor. The room intake line (`coordination.ts:220-226`) also fires on both events, but test 499 (`tests/room-brief.test.ts`) pins that as a deliberate contract on the Codex harness, so it stays; see flag F12. Hunk `03-static-brief-lines-sessionstart.diff`.

### F4. reviewer-correctness filters on certainty at the finding stage

- **Location**: `src/plugins/resources/profiles/reviewer-correctness.md:12-13`.
- **Evidence**: "No style, no cleanups, no speculation without a reproduction path."
- **Pattern**: Group 1a/1c severity or confidence filter in a review harness (Sonnet 5 § Code review harnesses).
- **Why obsolete on which target**: Sonnet 5 "follows that instruction more faithfully than earlier models did - it investigates just as thoroughly, identifies the bugs, and then doesn't report findings it judges below the stated bar", so measured recall falls. Opus 5's guide repeats the same point ("Severity filters still depress measured recall"). The review-gate and fix-loop already run one refuter per finding and "default to refuted=true when uncertain", which is the downstream filter the guide recommends; the finding stage should be coverage-first. The schema's `evidence` field is the real bar and stays.
- **Confidence**: Medium (documented, widely observed; no lab measurement in this repo).
- **Action**: `rewrite` in prose only, no schema change: "Report every bug you find, including ones you are not certain of, and say how sure you are inside the evidence: a separate refuter checks each finding against the code, so coverage matters more than certainty here. No style, no cleanups." The graph node prompts (`review-gate.md:142-146`, `fix-loop.md:61-65`) carry no certainty filter and are unchanged. Codex reading: same benefit. Hunk `04-reviewer-coverage-first.diff`. Not probed (noisy; see Probes).

### F5. Numeric summary caps on the gate verdict

- **Location**: `src/plugins/resources/profiles/synthesizer.md:13`; `src/plugins/graphs/review-gate.md:225`.
- **Evidence**: "Keep the summary to two sentences."; "summary: two sentences at most."
- **Pattern**: Group 1b/1f, numeric output ceiling.
- **Why obsolete on which target**: the guide removes hard caps in favour of qualitative length guidance; Sonnet 5 will honour "two sentences at most" literally even when the verdict's reason needs a third. A JSON `summary` string is not a format-sensitive output (keep-list 7 does not apply).
- **Confidence**: Medium.
- **Action**: `rewrite` to "the verdict and what decided it, briefly" in both places. No test pins either string. Hunk `05-summary-length-cap.diff`.

### F6. "Plan before you delegate." in the Lead mandate

- **Location**: `src/plugins/resources/profiles/lead.md:9`.
- **Evidence**: "You own technical coordination. Plan before you delegate. Brief every Peer with a bounded objective and its acceptance ..."
- **Pattern**: Group 1b, "plan before acting" scaffold (documented row: delete; causes over-planning).
- **Why obsolete on which target**: current models plan unprompted; Fable 5.1's guide names over-planning as its nudge-worthy failure ("When you have enough information to act, act"). The rest of the sentence already states what a brief must contain, which is the load-bearing part. Added with the profile on 2026-09-05 (`dabe0c35`), not as a mitigation for an observed failure. policy-breakdown enforces decomposition in code.
- **Confidence**: Medium (documented row; four words; provenance is authorship, not an incident).
- **Action**: `remove` the sentence. Codex reading: neutral. Hunk `06-lead-plan-before.diff`.

### F7. peer-opus.md carries migration-relative phrasing and an incident narrative

- **Location**: `src/plugins/resources/profiles/peer-opus.md:6-11`.
- **Evidence**: description "SLP Peer variant on Claude Opus (owner ruling 2026-09-02 while Codex is exhausted)"; body "Everything in the Peer mandate above applies unchanged; only the harness and model differ."
- **Pattern**: Group 1d migration-relative phrasing ("applies unchanged", "differ" against a version the model never saw); Group 2 history narrative in a description that rots when the Codex quota resets.
- **Why obsolete on which target**: the rendered `maestro-peer-opus.md` composes shared contract + Peer mandate + this body, so "above" resolves, but "only the harness and model differ" is a diff against nothing the reader has. Sonnet reads "applies unchanged" as an instruction about change rather than a mandate. The description is a frontmatter contract key by the brief's definition; it is included in the hunk because the Agent tool shows it to the Lead as the subagent's summary, and the Lead may decline that half.
- **Confidence**: Medium.
- **Action**: `rewrite` body to "The Peer mandate above is your whole mandate." (hunk `07a-peer-opus-body.diff`) and, separately because it is a contract key, the description to "SLP Peer variant on Claude Opus" (hunk `07b-peer-opus-description.diff`); the two apply independently or together. Tests pin the profile name only.

### F8. The Peer mandate covers narrowing but not widening (re-baseline for Opus 5)

- **Location**: `src/plugins/resources/profiles/peer.md:12-13`.
- **Evidence**: "a brief you cannot meet is returned with its blocker, never quietly narrowed."
- **Pattern**: keep-list 11, re-baselining adds text; Opus 5 § Behavioral shifts, task scope expansion.
- **Why it matters on which target**: Opus 5 (the shipped Peer model) "can add steps the user didn't request, or apply its own judgment about what the task should be without making that clear"; Fable 5.1's guide has the same scope-discipline block; Sonnet 5 is neutral. The shared contract fixes the *record* ("changed scope requires new work") but nothing tells the Peer what to do with work it notices beyond the brief. maestro-work already says "A never silently becomes A+B+C" for the classic surface; the SLP Peer never reads that skill.
- **Confidence**: Medium.
- **Action**: `add`, folded into the existing sentence: "Deliver the brief at the scope it set: a brief you cannot meet is returned with its blocker, never quietly narrowed, and work you notice beyond it is a note on the item, never part of the change." Codex reading: harmless. Hunk `08-peer-scope-widening.diff`.

### F9. The two MCP tools are under-described

- **Location**: `src/plugins/mcp.ts:30-34`, `:42-46`.
- **Evidence**: `maestro_find`: "Find live Maestro verbs, flags, descriptions, and workflow recipes." / query "Text to match against verbs and recipes."; `maestro_run`: "Run one Maestro verb line through the normal strict CLI dispatcher." / line "One Maestro verb line without a shell."
- **Pattern**: Group 3, vague one-liners; no return contract, no when-not-to-use, no failure shape.
- **Why it matters on which target**: under-description is the common tool failure on every current model; the handler (`mcp.ts:174-245`) has a precise contract the description omits: ten verbs and five recipes ranked by keyword hits, an `{ok, data}` envelope on success, a tool error carrying the CLI's `{ok: false, error}` envelope (with the unblocking command for gates such as GATE_BLOCKED, test 44), strict positional handling (UNKNOWN_ARGUMENT, test 43), shell-style quoting without a shell, `mcp serve` refused, and the `line` excluding the word `maestro`.
- **Confidence**: Medium (Group 3 documented; no measured failure in this repo).
- **Action**: `add`; the descriptions grow to the contract, with no steering language and no examples. No test pins the description text. Hunk `09-mcp-tool-descriptions.diff`.

## Flags (no edit proposed)

### F10. Decision-cited negations of phantom alternatives (keep-list 5 wins)

- `src/plugins/resources/WORKFLOW.md:84-85` "`dispatch` and `handback` are not legacy and are not scheduled for removal" (Hub d87, pinned by `tests/docs-contract.test.ts:250`); `src/plugins/recipes/slp.md:42-43` "There is no Observer, Advisor, sensor, scheduler, review, or reconcile role in SLP."; `src/plugins/skills/maestro-council/SKILL.md:21` "No Observer seat exists (Hub d98)" (pinned by `tests/docs-contract.test.ts:340`).
- Group 1d migration-relative phrasing: each line is a diff against a prior state (the Observer seat removed with pack v3 in `b3f6641a`; agents calling dispatch "legacy" at the SLP v2 cutover) and names alternatives a fresh model would not otherwise imagine. Each also carries a Hub decision id, and the audit has no evidence the reason expired (the owner's memory corpus still describes an Observer; "Retired verbs" sits two paragraphs from the dispatch sentence). Low confidence; a decision-preserving rewrite ("A council has exactly the seats its tier names (Hub d98)"; "SLP has exactly four agent roles and one non-agent runtime pane") is available if the Lead wants it, with the two test lines updated alongside.

### F11. `maestro-graph` pins `model: "opus"` for every Claude agent node

- `src/plugins/skills/maestro-graph/SKILL.md:74-75`. Group 2 pinned model name in a skill; Group 4 roster contract. The Agent tool falls back to the agent definition's `model` only when the parameter is omitted, and every shipped node profile says `model: default` (rendered with no `model:` line), so deleting the pin makes nodes inherit the Lead's model (Fable 5.1 on the owner's builder panes) and raises cost, while keeping it makes the profile's `model` key dead and blocks the Sonnet 5 cheap-node target the brief names. The fix is a frontmatter change (`model: sonnet` or `opus` per node profile), which the brief classes as contract, then dropping the pin from the skill. No lone skill hunk.

### F12. The room intake line fires on every UserPromptSubmit

- `src/plugins/coordination.ts:220-226`. Same pattern as F3 (Group 1d) but `tests/room-brief.test.ts:13` ("carry one Supervisor intake line on both prompt events") pins the per-turn delivery as a contract tested on the Codex harness, and the audit cannot verify whether Codex re-fires SessionStart after compaction. Leave until that is known.

### F13. Council seat prohibitions stated three times

- Profile bodies (`independent.md:11-12`, `challenger.md:13-14`, `specialist.md:11-12`: "You are analysis only: no edits, no spawning, no contact with other seats, no council skill"), the seat opener (`council.md:130-133` and `brief.md:27-30`: "do not load the council skill, open work, spawn or contact agents, or read other seats' work items") and the closer (`council.md:142-146`, `brief.md:36-40`: "Do not edit, create, rename, or delete files. Do not write code. Do not spawn or contact agents."). Group 1c repetition as reinforcement, but the three agree, the writes are enforced by `disallowed_tools` and audited under Hub d93, and the opener's first line is load-bearing on the pane path (a brief that opens "You are ..." is swallowed as a slash command). Keep-list 8. The closer's last two sentences ("Do not optimize for agreement. Distinguish direct observations from inference ...") are unique and should stay wherever the rest goes.

### F14. Six-step Loop choreography and the recipes' "Loop anatomy"

- `src/plugins/skills/maestro-work/SKILL.md:104-133`; `recipes/{audit,design,learning,ship,unattended,work}.md` "Perceive / Choose / Act / Observe / Learn / Continue". Group 1c step choreography; Fable 5.1 § Long-running agent recommendations says to de-prescribe migrated skills. Most steps here are record mechanics whose order matters (`work start` before edits, `work done` with the falsifier after), and the judgment steps state the quality bar rather than a method. Flag; a rewrite is a doctrine decision, not an audit fix.

### F15. Recipe and skill-reference duplicates have drifted

- `recipes/audit.md` versus `maestro-verify/references/audit.md` (the reference adds a triage pointer); `recipes/learning.md` versus `maestro-verify/references/learning.md` (the reference adds a "Better loop" section); `recipes/unattended.md` == `maestro-design/references/unattended.md`; `recipes/worktree.md` == `maestro-work/references/worktree.md`; `recipes/conflict-handoff.md` == `maestro-work/references/conflict-handoff.md`. Group 2 "information lives in exactly one place". They do not contradict, so keep-list 8 applies; which copy is canonical is the owner's call.

### F16. Verification scaffolding: the targets disagree

- `maestro-verify/SKILL.md:79-85` (fresh-context verifier subagent), `maestro-work/SKILL.md:124-125` (Observe step). Opus 5's checklist says delete verification instructions and subagent verification; Fable 5.1's says keep test-before-report instructions and that fresh-context verifier subagents outperform self-critique; Sonnet 5 is neutral. Under the brief's tie rule the text stays. Recorded so nobody applies the Opus 5 checklist item to this repo uniformly.

### F17. Node profiles used standalone say "the schema" with no schema present

- `refuter.md`, `fixer.md`, `synthesizer.md`, the four reviewer profiles: "Answer with exactly one JSON object matching the schema." The classifier says "the schema in the prompt". Under the graph the brief always carries one (d838); as a standalone `subagent_type` there is none, and Sonnet will look for it. Low; add "in the prompt" if the standalone use is wanted.

### F18. "Re-anchor on this list when unsure which step is active."

- `maestro-council/SKILL.md:44`. Group 1d retention crutch; current models retain the protocol list. The adjacent "Announce each phase in one line" is the opposite of cruft on Fable 5.1 (a specific line saying when user-facing text is wanted) and stays. Low.

### F19. "No emoji" in grilling.md

- `maestro-design/references/grilling.md:22`. Group 1d anti-formatting rule; Fable 5.1 under-formats and the guide says remove or make conditional. Provenance is the owner's global style preference, shipped to every user of the skill. Low; owner's call.

### F20. "Working discipline, carried here because this profile replaces the harness's default instructions"

- `SLP.md:17-21`. The clause explains the prompt's own construction (Group 2 history narrative) rather than the task; the four rules after it are context and stay. Low; drop the clause if the pack is next edited for another reason.

### F21. Group 4: the review-gate classifier lists changed files a function node already knows

- `graphs/review-gate.md:116-117`: "files: every changed path" is derivable from the `diffstat` function node; only the three boolean flags and `subsystems` are judgment. The model-call site stays (it is the judgment step); the `files` field could come from code. Low, structural.

### F22. Group 4: no per-surface token accounting was found

- The audit found no accounting of tokens per prompt surface (seat, node, hook line). Without it F3's saving and F11's cost trade cannot be measured. Advisory only.

## Clean surfaces

Reported as clean after the Step 4 greps and a line-by-line read:

- `SLP.md` shared contract: the READY challenge script ("reply on one line and nothing else ... Do not run tools ... Then wait for work") is a fragile-operation script and stays exact (keep-list 3); the `--blocked` question route (owner ruling 2026-09-05, `2def194d`) is enforced by `disallowed_tools` and explained in prose; the push-line and nudge descriptions match the code.
- Hook-emitted text: `[from <role>][<id> STATE] <summary>; read: maestro status <id>` (d753/d760/d840), `[from runtime][<id>] <kind> <evidence>; stop and run: ...` (d763, a fixed template by decision), `[attention] <seat> idle|pane exited|closed`, the READY challenge body. No emphasis, no steering.
- Profiles `team-supervisor.md`, `classifier.md`, `fixer.md`, `refuter.md`, `reviewer-contracts.md`, `reviewer-regression.md`, `reviewer-security.md`, `reviewer-simplify.md`, `independent.md`, `challenger.md`, `specialist.md`: every prohibition carries a reason or a decision; the refuter's "default to refuted=true when uncertain" is the designed downstream filter, not a finding-stage filter.
- `WORKFLOW.md` (apart from F10), all ten `style-*` recipes, `maestro-explore`, `maestro-diagnose`, `maestro-coach`, `maestro-questionnaire`, `maestro-improve`, `maestro-bundle`, `maestro-design` and every reference file: 2026-era text with reasons beside constraints; one ALL-CAPS "ONE" in `maestro-design/SKILL.md:82` (owner ruling, walk one fork at a time) is the corpus's only emphasis marker; the other caps grep hit, `report-format.md:16`, is a heading in a format template.
- `graphs/fix-loop.md`: every node prompt states goal, inputs and the reason for each limit.
- `dispatch.ts`, `memory.ts`, `brief.ts`, `lifecycle.ts`: no model-facing steering text beyond record formatting and operator error strings.

## Probes (Step 7)

Run on the weakest target, `claude -p --model sonnet --tools "" --setting-sources ""`,
with the profile body as `--system-prompt-file` and the composed node brief (node
prompt with a filled council brief, the key sentence, the schema block) on stdin,
three runs per arm, from `/tmp/prompt-audit/probes/` with `MAESTRO_READ_ONLY=1`.
`--setting-sources ""` skips every settings file and therefore the maestro hooks;
`--bare` could not be used because it also skips the keychain read and the CLI
reports "Not logged in". Context not stripped: `--system-prompt` replaces the
default system prompt, but the CLI may still inject `~/.claude/CLAUDE.md` as user
memory; it is identical across arms, so it does not confound the comparison. Prompts and every output are saved beside this file under
`probes/`.

The `sonnet` alias resolved to `claude-sonnet-5` (`probes/model-id.txt`). Every
output is scored the way the graph intake scores it: the first JSON object
extracted from a fence or from prose, then its keys counted
(`probes/score.txt` holds the scoring output; `probes/out-<arm>-<n>.txt` the raw
replies; `probes/p1-*.md`, `p1b-*.md`, `p2-*.md` the exact prompts).

| Probe | Arm | Runs | Result |
|---|---|---|---|
| P1: council `independent` node, `&report` schema, brief whose CASE OUTPUT CONTRACT enumerates all nine keys | before (current key sentence) | 3 | 9/9 keys, all 6 optional present, JSON only, every run |
| | after (hunk 01 wording) | 3 | 9/9 keys, all 6 optional present, JSON only, every run |
| P1b: same, but CASE OUTPUT CONTRACT reads "the JSON schema in this prompt" | before | 3 | 8, 5, 8 keys; optional present 5/6, 2/6, 5/6; `recommendation` dropped in all three |
| | after | 3 | 9/9 keys, all 6 optional present, every run |
| P2: council `premise` node, verifier profile as system prompt, current key sentence in the brief | before (current "Return exactly: <text block>") | 3 | one JSON object with all six keys, no text block, every run |
| | after (hunk 02 wording) | 3 | identical |

Reading: F1's harm is real and reproduces exactly where the guide predicts (the
literal reader has only the key sentence to go on); the fix removes it with no
regression on the enumerated-contract case. F2's predicted harm did not
reproduce: Sonnet 5 already followed the task brief's JSON over the profile's
"Return exactly" text block, so hunk 02 is a contradiction removal whose benefit
was not measured, and it stays at medium. F4 (reviewer recall) was not probed:
a recall measurement needs a seeded diff and many runs, and the reviewer nodes
need tools, which the isolated probe disables. F3, F5 to F9 were not probed:
none would need a live team, but their harms are token cost or a documented
behavior, not a first-return failure a small probe can show.

## Diff index

`/tmp/prompt-audit/proposed.diff` concatenates the ten hunks (nine findings; F7 is split in two) with a
`# finding N: <slug>` header each; the same hunks are in
`/tmp/prompt-audit/hunks/NN-<slug>.diff`. Diffed against the working tree at
`40494a6b` (the affected files were unmodified there). `git apply --check` from
the repo root passed for each hunk and for the combined file at audit time, and
again after the other builder's commit landed (HEAD `afc431be`, tree clean);
that commit touched SLP.md, slp-runtime.ts, slp-v2.ts, work.ts and the site,
none of the files a hunk edits.

| Hunk | Finding | Files |
|---|---|---|
| 01-schema-key-sentence | F1 | `src/plugins/graph-file.ts`, `tests/graph-brief.test.ts` |
| 02-verifier-auditor-shape | F2 | `profiles/verifier.md`, `profiles/auditor.md` |
| 03-static-brief-lines-sessionstart | F3 | `src/plugins/coordination.ts`, `src/plugins/recipe.ts`, `tests/slp-adoption.test.ts` |
| 04-reviewer-coverage-first | F4 | `profiles/reviewer-correctness.md` |
| 05-summary-length-cap | F5 | `profiles/synthesizer.md`, `graphs/review-gate.md` |
| 06-lead-plan-before | F6 | `profiles/lead.md` |
| 07a-peer-opus-body | F7 | `profiles/peer-opus.md` (body) |
| 07b-peer-opus-description | F7 | `profiles/peer-opus.md` (description key) |
| 08-peer-scope-widening | F8 | `profiles/peer.md` |
| 09-mcp-tool-descriptions | F9 | `src/plugins/mcp.ts` |

Applying any profile hunk needs `maestro install` afterwards to re-render
`~/.claude/agents/maestro-*.md`. The owner's shadow copies of `lead.md` and
`peer.md` under `~/maestro/profiles/` were diffed against the shipped files:
both bodies are byte-identical to the shipped bodies (only the frontmatter
differs), so they would need the same two edits (F6, F8) by hand, which this
audit does not make.

Test evidence: with the full diff applied to an exported copy of `40494a6b`
(made a git repo so `maestro install` can record its source), `bun test` passed
for graph-brief, graph-executor, session-coordination, room-brief,
extensions-mcp, slp-profiles and docs-contract (31 tests); a second run over
slp-adoption, phase-four-deletion, ux-review, role-line and session-coordination
(67 tests) also passed (`scratch/test-run-2.log`).
