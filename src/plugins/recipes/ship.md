# Ship

Use this recipe for close, commit, install, push, publish, release, or archive
gates. Local implementation authority does not imply authority for remote or
external state changes.

## Loop anatomy

### Perceive

Read `work show <id>`, child state, evidence, current branch, HEAD, and dirty
tree. Re-read the user's exact delivery authority and target.

### Choose

Select one legal next gate: final verification, independent QA or witness,
scoped commit, local install, external delivery, or stop. Do not bundle gates
whose authority differs.

### Act

Run the accepted verification and complete the work with any required `qa:` or
`witness:` evidence. Stage task-owned paths only and inspect the staged diff.
Perform external actions only when the user named them.

### Observe

Read back the actual result: test output, commit hash, installed version, or
remote state. A started or interrupted command is not delivery evidence.

### Learn

Record a recurrence guard only when closeout exposed a reusable failure mode.

### Continue

Report the delivered artifact and evidence, or the exact remaining authority
or verification gate. Never claim push, release, or publish from local state.
