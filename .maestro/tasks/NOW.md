# NOW
Updated: 2026-05-07T11:23:39.003Z

## In progress (13)
### tsk-61f819 . Fix all reviewed regressions
Owner: codex-019d967b-7d5e-7e41-b4ea-7dc0a1c110f3 (claimed unknown, last activity 1w ago)
Priority: P0 | Type: bug
Labels: fix, review-followup
Fix all confirmed regressions from review of commits 44d5f670, f3a0d110, 09e1c9e8, and e55384bf on current branch, including installer side effects, bundle path safety/manifest validation, agent migration cleanup, and config inspector hidden deprecated keys.

### tsk-1bb2cf . Review task coordination hardening change
Owner: codex-019d965f-ca77-7822-b63a-7112406a9970 (claimed 2w ago, last activity 1w ago)
Priority: P1 | Type: chore
Labels: codex, review

### tsk-bf71b6 . Check why release still missing after green run
Owner: codex-019d967b-7d5e-7e41-b4ea-7dc0a1c110f3 (claimed 2w ago, last activity 1w ago)
Priority: P1 | Type: bug
Labels: github, release
Inspect the latest Release workflow run, especially the Publish Release job, and compare it with the current GitHub releases list to explain why no new release is visible.

### tsk-7282d9 . Update global AGENTS.md guidance
Owner: codex-019d965f-ca77-7822-b63a-7112406a9970 (claimed unknown, last activity 1w ago)
Priority: P2 | Type: chore
Labels: codex, config

### tsk-baf012 . Check maestro task dependency support against beads-rust
Owner: codex-019d965f-ca77-7822-b63a-7112406a9970 (claimed unknown, last activity 1w ago)
Priority: P2 | Type: chore
Labels: codex, maestro

### tsk-20256e . Map full parallel-agent safety picture in maestro
Owner: codex-019d965f-ca77-7822-b63a-7112406a9970 (claimed unknown, last activity 1w ago)
Priority: P2 | Type: chore
Labels: codex, maestro

### tsk-26c508 . Review recent task ownership recovery changes
Owner: codex-019d965f-ca77-7822-b63a-7112406a9970 (claimed 2w ago, last activity 1w ago)
Priority: P2 | Type: chore
Labels: codex, review

### tsk-4f33c1 . Read-only standards review 35f644fd^..1c8296da
Owner: codex-019d96b8-7c30-79b0-9b60-e7d6ec47b81e (claimed 2w ago, last activity 1w ago)
Priority: P2 | Type: task

### tsk-24ba59 . Inspect and plan CI fix for current PR
Owner: codex-019d9906-4b72-7b10-86b2-6b5d2e843f78 (claimed 2w ago, last activity 1w ago)
Priority: P2 | Type: task
Labels: codex, github, ci

### tsk-e3b6e9 . Investigate latest CI failure after boundary root fix
Owner: codex-019d9906-4b72-7b10-86b2-6b5d2e843f78 (claimed 2w ago, last activity 1w ago)
Priority: P2 | Type: task
Labels: ci, windows

### tsk-0ac52e . Create Maestro-native handoff to Claude Code
Owner: codex-019d9ae1-2f34-7273-aec1-3bfd0ca18601 (claimed 2w ago, last activity 1w ago)
Priority: P2 | Type: task

### tsk-018e80 . Scope diff and history for feat/task-contracts-stack
Owner: 019db09e-9dfa-7b82-94f5-05e9cf769e76 (claimed 2w ago, last activity 1w ago)
Priority: P2 | Type: task

### tsk-0f4914 . Load review instructions and diff scope
Owner: 019db2c1-c820-7350-bf2f-5b8f98ff9219 (claimed 2w ago, last activity 1w ago)
Priority: P2 | Type: task


## Ready to pick up (5)
### tsk-94987c . Desktop Path 5 native Ghostty underlay rollout
Priority: P1 | Type: epic
Labels: desktop, path5, ghostty, paseo
Implement the paseo fork architecture for Maestro Desktop using libghostty + GhosttyKit under a transparent Chromium compositor in apps/desktop. Preserve the existing sidebar, tabs, and git panel; keep maestro panels read-only through CLI JSON; never restart the daemon on port 6767 without explicit ...

### tsk-ddfbc5 . Phase 0: verify env and stock paseo desktop dev flow
Priority: P1 | Type: chore
Labels: desktop, path5, phase0, spike
Verify sw_vers, xcode-select, xcodebuild, zig, node, and npm; install workspace deps if needed; confirm stock dev:server, dev:app, and dev:desktop can be launched without restarting the main daemon on port 6767. Acceptance: stock paseo opens, port 6767 is reachable, and no console errors appear.

### tsk-bbbf3f . Review shipped Claude-style task coordination redesign
Priority: P1 | Type: chore
Labels: codex, review

### tsk-cada79 . Read-only bug hunt for shipped task-coordination redesign
Priority: P1 | Type: bug
Labels: codex, bug-hunt

### tsk-90c398 . Review last four commits from 44d5f670 to e55384bf
Priority: P2 | Type: task
Labels: review, review-swarm
Read-only review of commit range 44d5f670..e55384bf for regressions, security, reliability, and contract/test gaps.


## Stuck (13)
### tsk-61f819 . Fix all reviewed regressions
Owner: codex-019d967b-7d5e-7e41-b4ea-7dc0a1c110f3 (claimed unknown, last activity 1w ago)
Priority: P0 | Type: bug
Labels: fix, review-followup
Fix all confirmed regressions from review of commits 44d5f670, f3a0d110, 09e1c9e8, and e55384bf on current branch, including installer side effects, bundle path safety/manifest validation, agent migration cleanup, and config inspector hidden deprecated keys.

### tsk-1bb2cf . Review task coordination hardening change
Owner: codex-019d965f-ca77-7822-b63a-7112406a9970 (claimed 2w ago, last activity 1w ago)
Priority: P1 | Type: chore
Labels: codex, review

### tsk-bf71b6 . Check why release still missing after green run
Owner: codex-019d967b-7d5e-7e41-b4ea-7dc0a1c110f3 (claimed 2w ago, last activity 1w ago)
Priority: P1 | Type: bug
Labels: github, release
Inspect the latest Release workflow run, especially the Publish Release job, and compare it with the current GitHub releases list to explain why no new release is visible.

### tsk-7282d9 . Update global AGENTS.md guidance
Owner: codex-019d965f-ca77-7822-b63a-7112406a9970 (claimed unknown, last activity 1w ago)
Priority: P2 | Type: chore
Labels: codex, config

### tsk-baf012 . Check maestro task dependency support against beads-rust
Owner: codex-019d965f-ca77-7822-b63a-7112406a9970 (claimed unknown, last activity 1w ago)
Priority: P2 | Type: chore
Labels: codex, maestro

### tsk-20256e . Map full parallel-agent safety picture in maestro
Owner: codex-019d965f-ca77-7822-b63a-7112406a9970 (claimed unknown, last activity 1w ago)
Priority: P2 | Type: chore
Labels: codex, maestro

### tsk-26c508 . Review recent task ownership recovery changes
Owner: codex-019d965f-ca77-7822-b63a-7112406a9970 (claimed 2w ago, last activity 1w ago)
Priority: P2 | Type: chore
Labels: codex, review

### tsk-4f33c1 . Read-only standards review 35f644fd^..1c8296da
Owner: codex-019d96b8-7c30-79b0-9b60-e7d6ec47b81e (claimed 2w ago, last activity 1w ago)
Priority: P2 | Type: task

### tsk-24ba59 . Inspect and plan CI fix for current PR
Owner: codex-019d9906-4b72-7b10-86b2-6b5d2e843f78 (claimed 2w ago, last activity 1w ago)
Priority: P2 | Type: task
Labels: codex, github, ci

### tsk-e3b6e9 . Investigate latest CI failure after boundary root fix
Owner: codex-019d9906-4b72-7b10-86b2-6b5d2e843f78 (claimed 2w ago, last activity 1w ago)
Priority: P2 | Type: task
Labels: ci, windows

### tsk-0ac52e . Create Maestro-native handoff to Claude Code
Owner: codex-019d9ae1-2f34-7273-aec1-3bfd0ca18601 (claimed 2w ago, last activity 1w ago)
Priority: P2 | Type: task

### tsk-018e80 . Scope diff and history for feat/task-contracts-stack
Owner: 019db09e-9dfa-7b82-94f5-05e9cf769e76 (claimed 2w ago, last activity 1w ago)
Priority: P2 | Type: task

### tsk-0f4914 . Load review instructions and diff scope
Owner: 019db2c1-c820-7350-bf2f-5b8f98ff9219 (claimed 2w ago, last activity 1w ago)
Priority: P2 | Type: task

