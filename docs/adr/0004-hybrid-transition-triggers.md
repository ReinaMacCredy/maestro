# Hybrid transitions: manual entry, automatic exit on verdict

Agents manually call verbs to *enter* check or blocked states (`maestro task claim`, `maestro task verify`, `maestro task block`). The harness automatically *exits* those states based on the result of the verb invocation: `verifying -> doing` on FAIL verdict (the Ralph Wiggum Loop falls out), `verifying -> ready` on PASS, `ready -> shipped` when merge is detected at the next verb call.

Maestro stays passive (no daemon, no scheduler): "automatic" here always means "computed from the result of a verb the agent just called," never background polling. The no-runner-inversion lint continues to enforce this.

Rejected: all-manual (too many verbs for agent to remember); all-automatic (requires polling, breaks passive-harness rule).
