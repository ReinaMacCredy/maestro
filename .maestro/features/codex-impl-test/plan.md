## Discovery

The maestro CLI currently has 92 commands but the CLAUDE.md documentation lists only 70 in the "CLI Commands" section. 
We investigated and found that the count is stale from before the cross-agent handoff commands were added.
The handoff domain section lists only 3 commands but now has 9 (send, receive, ack, list, read, status, plan, pickup, report).
The total count in the "CLI Harness" description line also says 89 but should say 92.

### 1. Update CLAUDE.md command counts

Update the CLAUDE.md file at project root:
- Change "89 CLI commands" to "92 CLI commands" in the Architecture section
- Change "CLI Commands (70)" heading to the correct count
- Update the Handoff section from 3 to 9 commands: add `handoff-plan`, `handoff-pickup`, `handoff-report`, `handoff-list`, `handoff-read`, `handoff-status`
- Update the Other section count if needed (currently 13)

### 2. Update total in CLI Reference section

In the same CLAUDE.md file, the "CLI Reference (Agent Use)" section mentions commands.
Add entries for the 3 new cross-agent handoff commands under a new subsection or in the existing Handoff entries:
- `maestro handoff-plan --to <agent> --json` -- export plan for another agent
- `maestro handoff-pickup --json` -- discover pending handoff  
- `maestro handoff-report --content "..." --json` -- report completion
