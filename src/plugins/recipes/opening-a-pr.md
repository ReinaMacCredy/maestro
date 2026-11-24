# Opening a PR

Use this recipe as the deliver step that ends every code playbook. It finds
the delivery grant, then commits, pushes the work branch and opens a ready PR
only as far as that grant reaches. Ported from pstack's opening-a-pr playbook
(Lauren Tan, MIT; notice in THIRD_PARTY.md).

## The grant

A delivery grant exists in two cases only
(`~/maestro/WORKFLOW.md#authorization-boundaries`):

- The repository has `policy-auto-pr` enabled. The SessionStart brief then
  prints a `delivery grant:` line, and `maestro plugin list` shows the policy
  active. The owner enables it with `maestro plugin enable policy-auto-pr`.
  An agent never enables it for itself.
- The owner's words in this session grant commit and PR. Quote them in a work
  note on the held item before the first commit, with
  `maestro work note <id> "grant: <owner quote>"`.

A grant covers commits, the push of the work branch, and opening the PR.
Merge, tag and release stay owner gates under every grant, and so does every
irreversible external action. Reversible ones, such as PR comments and CI
re-runs, proceed without asking.

## Steps

1. Find the grant (The grant above). Record an owner-words grant as a
   `grant:` work note before any commit.
2. Work from a git worktree off the default branch
   (`maestro recipe show worktree`). Writing delegates inherit it. Two
   writers on one branch each get their own worktree. Leave unrelated dirty
   work alone; start a fresh worktree and apply only this change. When a
   worktree is snarled, start a fresh one from the default branch and redo
   the change minimally.
3. Run a simplification pass (WORKFLOW.md Per-tool adapters) over the diff.
   On Full it already ran after green and before maestro-verify, so the
   verified diff is final; mark this step and the next `skip: ran before
   verify`.
4. Run `maestro recipe show no-comments` on the diff. When either pass
   changed code, rerun the affected checks
   (`~/maestro/WORKFLOW.md#recovery-and-verification`).
5. With no grant, commit only if the owner asked for a commit, per Commits
   below. Then stop with the verified working tree. Record the pending gate
   in a work note ("push waits for the owner") and report it
   (`~/maestro/WORKFLOW.md#completion-and-delivery`). Mark steps 6 to 9
   `skip: no delivery grant`.
6. Under a grant, commit per verified unit, per Commits below. Stage
   task-owned paths only (`~/maestro/WORKFLOW.md#concurrency-and-git`).
7. Push the work branch with `git push -u origin <branch>`. Never push the
   default branch directly. Force-push only a branch this run alone pushes,
   with `git push --force-with-lease=<branch>:<sha you last pushed>`. A
   force-push to a shared branch pauses for the owner.
8. Open the PR ready with `gh pr create --base <base> --title "<title>" --body-file <file>`,
   per Title and body below. Never pass `--draft`. If it still opens as a
   draft, run `gh pr ready <number>`. When the harness ships its own PR
   tool, create, edit, retarget and mark ready through it instead, with
   draft off. A PR made beside that tool misses what the tool tracks.
9. Read the PR back with `gh pr view <number>` before you state its status.
   Record the URL with `maestro work note <id> "pr: <url>"`, post it, and
   keep building. Do not start a babysit (Babysit below).

## Commits

- Commit liberally once commits are allowed. Each commit is a future PR. It
  lands on its own and sits in an order that tells the story.
- Rebase into small, ordered commits before you open the PR.
- Amend when the fix belongs in the commit you just made. Make a new commit
  when the fix is separable.
- A commit body does not restate the subject.

## Title and body

Write every PR title, PR body and commit body per
`maestro recipe show technical-writing`. Every layer applies except
Diátaxis. Apply its STE vocabulary rules. Use one word per action, keep the
articles, and use a plain verb over an `-ing` form where one works. Then
apply `maestro recipe show unslop`.

### Title

Use Conventional Commits, `type(scope): subject`, with no trailing period.
The type is a word such as `feat`, `fix`, `docs`, `refactor`, `test`,
`chore` or `perf`. The scope is the area, such as `recipes` or `policy`.
The subject is short and imperative, and it names a real symbol when one
carries the change. For example,
`fix(recipes): retarget the opening-a-pr babysit trigger`.

### Body

The PR body is a one-minute briefing for a reviewer who has the diff, not a
lab notebook. It says why the change exists, what it leaves out, what it
could break, and how you proved it works. Write short, simple sentences with
few identifiers and no walls of text. The body becomes the squash commit
body, so keep it under about 40 lines. Use `##` headings, not bold lead-ins,
in this order, and drop a section with nothing to say:

- `## Why`. The problem and the approach in one to three short sentences.
  No SHAs, rebase genealogy, or "based on main" preamble.
- `## What changed`. One to three short bullets. Name a real symbol or path
  only when it carries the change. Name both sides of a rename or retarget.
- `## Scope`. Always present. One to three short items on what it covers and
  what it leaves out on purpose, such as a follow-up or a known gap. No
  symbols, paths, or file-by-file essay.
- `## Tradeoffs`. Only the rejected alternatives a reviewer would otherwise
  ask about. Skip it when there was no real choice.
- `## Blast Radius`. One or two sentences on who or what it touches and why
  that is safe or risky. If the default branch is red, state the cost of
  leaving it red.
- `## Verification`. One to three bullets, each a real run path and its
  outcome. For perf, give one primary number with its unit, before and
  after, and a link to the arena or fan-out directory for the rest. No
  sample-size methodology, lane recitals, or metric tables.

After the sections, attach videos or screenshots when they prove a claim.
Full SHAs, fan-out or arena lane recitals, lever-correction essays,
file-by-file checklists and "CLEAN" verdicts go in a linked artifact, not
the body.

## Size and stacks

Prefer five narrow PRs to one large PR. A stack is a base-branch chain. The
root PR targets trunk. Each child branch rebases onto its parent's exact tip,
and its PR targets the parent branch. Create a child with
`gh pr create --base <parent-branch>` and retarget one with
`gh pr edit <pr> --base <parent-branch>`. As a parent merges, its child moves
onto trunk per step 5 of `maestro recipe show shipping`. Branch from trunk
only for independent work. Rebase on trunk before substantial stack work.
`gh` is the forge CLI. Never require Graphite (`gt`).

## Babysit

Opening a PR does not start a babysit. Post the URL and keep building.
Finish the phase or the stack first. Run a separate babysit pass
(`maestro recipe show babysit`) only when the owner asks for one after the
whole stack exists. A babysit for each new PR stalls the build and spends
checks on commits that later waves restart. Push back when feedback drifts
from intent.

A delegate that opens a PR carries the grant in its brief. It runs the
review gate (it asks one reviewer agent to review the diff),
the simplification pass and `maestro recipe show no-comments`, posts the
URL, and returns without babysitting. The exception is a queue PR owner
(`maestro recipe show shipping`, Queues). Its brief assigns the babysit loop
and is the ask babysit waits for. It starts the loop after its code-ready
report, reports merge-ready or stack-ready, and is exempt from the rules here
and in babysit that hold babysitting until a whole stack is built.

## Reply

Name the grant used, the policy or the quoted `grant:` note. Give each PR
URL with its read-back state, or the pending gate the owner holds.
