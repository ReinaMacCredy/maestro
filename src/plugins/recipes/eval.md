# Eval

Use this recipe to test how a skill, recipe, structure or prompt change
affects agent behavior before you promote it. You own the experiment design.
You plan, blind, run and synthesize. Ported from pstack's eval playbook (Lauren
Tan, MIT; notice in THIRD_PARTY.md).

## Blinding rules

These rules hold for every step below.

- No `eval`, `test`, `judge`, `experiment`, `rubric`, `score`, `compare`,
  `benchmark`, `candidate` or `arena` in any directory, file or prompt the
  candidate sees.
- The candidate prompt looks like an organic user request. State the goal,
  not the meta.
- No chain-eliciting cues. Do not ask the candidate to list which skills,
  recipes, principles or files it applied. Ask for design notes in general,
  and grade chain-following from the shape of the code, not from self-report.
- Sanitize directory and slug names. Use project-shaped names a user might
  pick.
- Do not tell the candidate other candidates exist.
- The judge may know it is judging, but it sees outputs by sanitized label
  only, never by model name.
- To compare two variants, one judge scores both sets in a single pass on one
  scale, blind to which set each came from.

## Steps

1. Frame. State the variant under test and the behavior that counts as
   success. Write the rubric, 3 to 6 concrete criteria, for the judge only.
   Hold it back from every candidate.
2. Set up sanitized environments. Give each candidate its own working
   directory with the variant in place. Plant the context an organic task
   would have: a project skeleton, and the skills or recipes the candidate
   would naturally read.
3. Author one organic prompt, worded the way a user would type it, with no
   leak of what is being measured.
4. Run N parallel candidates the way `maestro recipe show arena` fans out its
   runners, on different models where the harness offers them. Each works in
   its own sanitized directory on the same prompt. A candidate runs as a plain
   subagent or a fresh harness session with no maestro profile. A profile's
   name and mandate are text the candidate sees, and the `arena-runner`
   profile names the arena.
5. Run one blinded judge the way that recipe runs its cross-judge, on a
   model family different from the candidates': the `arena-judge` profile
   when no candidate ran on Claude, a Codex council profile such as
   `challenger` when one did.
   The judge sees the rubric and the outputs by sanitized label, never a model
   name.
6. Verify the chain from transcripts, not self-report. Read each candidate's
   session transcript, the one the harness keeps for that session, and only
   this run's candidates. Never search transcripts across workspaces, since
   that reads private chats from unrelated projects. Look at which files each
   candidate actually opened. Grade chain-following from the files it really
   read plus the shape of its code, never from its own claims.
7. Read every candidate output yourself, end to end. Compare it with the
   judge's verdict. Disagreement means a model is biased or the rubric is
   ambiguous. Synthesize.

## Reply

The variant under test. The rubric. Notes per candidate. The judge's verdict.
Your synthesis. A recommendation on whether to promote the variant.
