---
name: maestro-ship
description: Verify and close - run the VERIFY table, harvest durable lessons into decisions, close the bundle, and never claim remote state from local evidence.
---
<!-- maestro-skill-version: dev -->

# maestro-ship

Use for close, commit, install, push, publish, release, or archive gates.
Local implementation authority does not imply authority for remote or external
state changes.

## Verify

- Run every VERIFY.md scenario against its work item's acceptance/claims and
  fill the Result column; run each anti-goal check (grep, diff, readback).
- For risky seams, spot-check assertion strength before filling PASS:
  hand-flip 2-3 mutants in the new logic (invert a condition, off-by-one a
  boundary, append text to an asserted message) and the suite must go red
  each time; restore after each. A surviving mutant is a weak or missing
  test and a FAIL of that scenario, not a side note - beware substring
  matchers like `toThrow(string)` that pass on a changed message.
- Re-read the user's exact delivery authority and target before any gate.
- Select one legal next gate at a time: final verification, independent QA or
  witness, scoped commit, local install, external delivery, or stop. Do not
  bundle gates whose authority differs.
- Read back the actual result: test output, commit hash, installed version. A
  started or interrupted command is not delivery evidence.

Read-only review method: [references/audit.md](references/audit.md).

## Learn, then close

Before closing, harvest what outlives the bundle
([references/learning.md](references/learning.md)): a verified correction or
durable constraint becomes a locked decision or a work note - never only chat.

```
maestro bundle close <id>    # snapshots the trio into the store, archives it
```

The snapshot is the durable memory; after close the directory is disposable
and `maestro search` still recalls the text.

## Definition of done

Acceptance met, changed surface verified, available test/lint/type/build
checks pass, claims name their falsifier, risky changes carry rollback notes.
Never claim push, release, or publish from local state; those gates are the
user's.
