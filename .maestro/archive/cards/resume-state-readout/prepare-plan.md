## Task T1: Extend GitSnapshot with branch name and categorized dirty counts
check: GitSnapshot exposes the current branch name and two uncommitted file counts split by path prefix -- one count for paths under .maestro/ (maestro cards), one for all other tracked/untracked changes (code/other)
check: a unit test over a fixture repo with one .maestro/ change and one non-.maestro/ change asserts the branch is set and the counts are maestro-cards=1 and code/other=1

## Task T2: Render git line and contextual clean-worktree note on resume and status
after: T1
check: maestro resume (default, no --full) and maestro status each print a git line showing the current branch name and the two split uncommitted counts
check: the clean-worktree note is printed only when the next valid verb is ship/verify-shaped AND the code/other count is greater than zero; it is absent when the next verb is not ship/verify-shaped or the code/other count is zero

## Task T3: Concern-only proof line with named repair on resume and status
after: T2
check: in default resume/status (no --full), a proof line is shown only on concern -- Stale, Failed, or Missing when the task is in needs_verification -- and is absent for an accepted+fresh task and for a ready/unclaimed task
check: each proof concern names the exact repair -- Stale names 'maestro task verify <id>' framed as a refresh (HEAD moved; likely no code change) with the stale reason, Failed names fix-then-'maestro task verify <id>', Missing-when-expected names the lifecycle verb
check: the resume proof-recovery pointer emits 'maestro task verify <id>' and no longer emits 'maestro query proof <id>'

## Task T4: Non-blocking proof advisory in feature ship --dry-run
after: T1
check: maestro feature ship --dry-run <id> prints a non-blocking advisory listing each verified child task whose proof commit differs from current HEAD ('verified at older commits; re-verify if their code changed'); a feature whose verified children all match HEAD prints no advisory
check: the advisory does not block the dry-run and adds no entry to ship_gaps_for_record's blocking gaps (a dry-run that would otherwise pass still passes with the advisory present)
