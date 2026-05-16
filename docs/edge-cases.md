# Agent Edge Cases

Five recurring failure modes that surfaced once Maestro started anchoring
long-running agent runs. Each section names the trigger, the detection rule
the harness applies, the agent-facing surface (where the warning actually
appears), and the recovery verb the agent should run.

Regression coverage lives in `tests/e2e/edge-cases/`.

---

## 1. Anchor staleness

**Trigger.** A `session-start` evidence row anchors "recent commits" to the
head SHA at session start. If the branch is force-pushed or reset, that SHA
becomes unreachable; without detection, `task introspect` silently reports
"no commits since session anchor" even though the agent committed.

**Detection rule.** `composeTaskIntrospection` calls
`git cat-file -e <sha>^{commit}` for the recorded `headSha`. If the commit
is no longer reachable, `view.anchor = { sha, stale: true }` and
`recentCommits` is left empty.

**Agent-facing surface.** `maestro task introspect <id>` markdown:

```
## Recent commits (last 5 since last session-start)
anchor: stale (commit abcdef1 not reachable)
Recovery hint: re-run `maestro session start` to anchor at HEAD.
```

The JSON form carries `anchor.stale=true`.

**Recovery verb.** `maestro session start <taskId>` — anchors a new
`session-start` row at the current HEAD.

---

## 2. Tool-call loop detection

**Trigger.** An agent retries the same command without changing approach
(classic case: same failing `bun test` three times in a row).

**Detection rule.** `composeTaskIntrospection` scans evidence in
chronological order. A "run" is reset by `verdict-requested`,
`session-start`, or `session-exit`. Inside a run, identical
`(kind, stableHash(payload))` rows in a row are counted; the highest run
≥3 becomes `loopWarning`.

**Agent-facing surface.** `maestro task introspect <id>` markdown emits a
`## Loop warning` section with the kind, count, payload hash, and a
recovery hint pointing at `ralph review --stuck-threshold 1`. The JSON form
carries `loopWarning`.

**Recovery verb.** `maestro ralph review --task <id> --stuck-threshold 1`
— the convergence oracle reports whether the findings are actually
converging or whether the agent should pick a different angle.

---

## 3. ProofMap holes appended as diagnostic

**Trigger.** A verdict is non-PASS for an unrelated reason (trust errors,
auto-merge disallowed). Agents fixate on the visible failure and ship with
silent acceptance-criteria coverage gaps.

**Detection rule.** `computeRisk` calls `appendProofMapDiagnostic` on the
FAIL (trust errors) and HUMAN (autoMergeNotAllowed) exit paths. The helper
uses the existing `uncoveredCriteria` predicate (spec preferred over
`contract.doneWhen`) and pushes a single `proof-map-incomplete` reason
listing the uncovered criterion IDs. Idempotent — never duplicates a reason
already added by the release-policy gate.

**Agent-facing surface.** `maestro verdict show --task <id>` lists a
`proof-map-incomplete` reason whose message names the uncovered criterion
IDs and the recovery verb.

**Recovery verb.** `maestro evidence record --task <id> --kind command
--command "..." --exit 0 --criterion <ac-id>` — the `criterion_id` payload
field is what `uncoveredCriteria` reads. Record one row per uncovered
criterion, then re-request the verdict.

---

## 4. Cost-budget BLOCK names the exhausted limit

**Trigger.** `checkCostBudget` flips to `exhausted=true`, the verdict
becomes BLOCK, and the agent has no way to tell *which* limit hit
(`maxRetries`, `maxWallClockSeconds`, or `maxTokens`).

**Detection rule.** `request-verdict.usecase.ts` passes
`checkCostBudget(...).reason` through to `computeRisk` as
`costBudgetReason`. `costBudgetExhausted(reason)` (the reason template)
embeds both the human-readable limit name and the machine code into the
verdict reason.

**Agent-facing surface.** `maestro verdict show --task <id>` reason
message:

> Cost budget exhausted; further execution blocked. Exhausted limit:
> `costBudget.maxRetries` (reason=max-retries). Run `maestro task budget
> --task <id>` to inspect the limits, amend the contract's costBudget via
> `maestro contract amend` to raise the cap, or escalate to a human via
> `maestro task block <id> --reason "<cost-budget context>"`.

The `findingChecks` field carries the machine code for programmatic
consumers.

**Recovery verbs.** `maestro task budget --task <id>` for inspection;
`maestro contract amend ...` to raise the cap; `maestro task block <id>
--reason "<context>"` to escalate (drops a handoff envelope for the
human or follow-up agent).

---

## 5. Skill-binary drift runtime hint

**Trigger.** The installed skill bundle references a verb the binary
doesn't have (skill is newer than the binary, or a feature was reverted
without bumping skill bundle). Agents typing the verb hit a bare Commander
"unknown command" line and spin.

**Detection rule.** `checkSkillBinaryParity` parses ``maestro <verb>``
references out of every bundled `SKILL.md` and checks the verb's first
segment against the binary's known top-level command names. The same check
runs in `maestro setup --check` (audit-time) and in the
`commander.unknownCommand` catch arm of `src/index.ts` (runtime).

**Agent-facing surface.** When `maestro <missing-verb>` fails, an extra
stderr line is emitted before the Commander exit:

```
Skill expects "maestro task observe"; binary v1.0.7 does not have it.
Run "maestro update" or downgrade the skill bundle.
```

**Recovery verbs.** `maestro update` to upgrade the binary;
`maestro setup --check` to audit drift; if the skill is the wrong version,
manually replace `~/.claude/skills/maestro-*/` with the binary's bundled
skill source.
