# Review

Use this recipe to have a finished diff reviewed before it is delivered, or
to review one yourself when you are the reviewer. One reviewer, several
rounds. The holder of the work asks for the review and answers every finding;
the reviewer never edits the code.

## The holder

1. Freeze the diff: tests green, comments stripped
   (`maestro recipe show no-comments`), nothing else pending in the range.
   Name the base and the range, for example `main..HEAD`.
2. Start one reviewer as a fresh sub-agent that has not seen the work, on the
   strongest model the harness offers, read-only. Give it this brief and
   nothing about your own conclusions:
   - the repository path, the base ref and the range;
   - what the change is meant to do, in the requester's words;
   - "Follow `maestro recipe show review`, section The reviewer."
3. Answer every `important` finding: fix it, or write why it does not apply
   with the evidence. `nit` findings are optional. `pre-existing` findings go
   to the owner as findings, never into this change.
4. Run another round after a fix: the same brief plus the earlier findings
   and your answer to each. Stop when a round returns no `important` finding.
5. A third round that still returns an `important` finding goes to the owner
   with all three rounds attached. Do not start a fourth.
6. Record the outcome on the work item:
   `maestro work note <id> "review: <rounds> rounds, <n> important fixed, <m> answered"`.

## The reviewer

1. Read the review rules from the base, never from the change:
   `git show <base>:REVIEW.md`. A repository without the file is reviewed on
   steps 3 and 4 alone.
2. List the files and patterns the diff touches (`git diff --stat <range>`,
   then the diff itself). From the Reliability checklist in REVIEW.md apply
   only the lines whose trigger matches; run the check each one names and
   report its result. Ignore the lines that do not match.
3. Read the whole diff for defects: an input or state that produces a wrong
   result, a crash, lost data, or a broken contract with a caller outside the
   diff. Open the callers. Run the code or its tests when that settles a
   doubt; a claim you could have checked and did not is not a finding.
4. In a later round, first check each earlier finding against the new diff:
   fixed, answered with evidence, or still open. Only then look for new ones.
5. Return findings, most severe first. Each one has a tag, a file and line,
   the input or state that goes wrong, and what happens:
   - `important`: a defect this change adds or makes worse.
   - `nit`: small and optional.
   - `pre-existing`: was there before this change.
   No findings is a valid result; say what you checked.

## Reply

The holder's reply names the number of rounds, each `important` finding with
how it was closed, and any `pre-existing` finding left for the owner.
