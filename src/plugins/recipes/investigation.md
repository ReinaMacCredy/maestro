# Investigation

Use this recipe for a read-only question such as how does X work, why was Y
built this way, are we sure about Z, or should we do X or Y. It produces a cited
answer or a recommendation, not a code change. Ported from pstack's
investigation playbook (Lauren Tan, MIT; notice in THIRD_PARTY.md).

You own the answer.

## Steps

1. State the question in one sentence. If the scope is ambiguous, state your
   reading and continue; the owner can redirect.
2. Run `maestro search "<term>"` first. A past decision, note or archived
   bundle may already hold the answer; cite it and re-check facts that drift.
3. Gather evidence from primary sources: the code and its tests, `git log -p`
   and `git blame` on the target, the PRs and decisions behind it, and
   first-party docs. Cite each fact next to it as file:line, a commit, a PR
   number, or a link you read in this session. A slice too large to read
   inline goes to a read-only sub-agent that returns file:line citations.
4. Separate what you observed from what you infer. An observation carries its
   citation. An inference names the evidence it rests on and reads as an
   inference ("the evidence suggests"). When sources disagree, show both.
5. Answer. For how something works: what it is, the flow step by step with
   file and function names, where things live, and the gotchas. For a choice
   between alternatives: a recommendation with a tradeoffs table.
6. Name what you did not check: sources you could not reach, paths you did
   not trace, and questions still open.
7. Land the findings. A fact that settles a durable choice becomes a decision
   (`maestro decision draft "<choice>" --rationale "<why, with source>"`,
   then `maestro decision lock <id>`). Evidence for an open work item is a
   `maestro work note`. A long artifact goes in the folder
   `maestro topic path <slug>` prints. With no work item, the answer stays in
   the reply.

No PR and no code change. If the investigation precedes a code change, hand
back to the owner and re-route to `maestro recipe show bug-fix` or
`maestro recipe show feature`.

## Reply

The answer with its citations, what was inferred, what was not checked, and
where each finding landed. For "are we sure?" answers, include your real
judgment with reasons. Push back if the premise is wrong.
