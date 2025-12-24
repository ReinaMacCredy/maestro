# Session Lifecycle Pattern

Guidance for AGENTS.md to enable context handoff between sessions.

## Overview

Session lifecycle is **best-effort**. Agent compliance varies, but even partial adoption improves context continuity.

## Session Start

On first response in a new session:

1. **Register agent**
   ```
   register_agent(
     project_key: "<workspace>",
     program: "amp",
     model: "<model>"
   )
   ```

2. **Check inbox for handoff**
   ```
   fetch_inbox(
     project_key: "<workspace>",
     agent_name: "<your_name>"
   )
   ```

3. **Summarize relevant context** before proceeding with user's request

## Session End

Before ending (user says bye, task complete, context compaction imminent):

1. **Send handoff message**
   ```
   send_message(
     project_key: "<workspace>",
     sender_name: "<your_name>",
     to: ["<next_session>"],  # or broadcast
     subject: "Session handoff - <date>",
     body_md: <handoff template>
   )
   ```

2. **Release any file reservations**
   ```
   release_file_reservations(
     project_key: "<workspace>",
     agent_name: "<your_name>"
   )
   ```

## Handoff Message Template

Adapt as needed based on session content:

```markdown
## Session Handoff - {date}

### Completed
- {list of completed items}

### Decisions Made
- {key decisions and rationale}

### Next Steps
- {what remains to be done}

### Open Questions
- {unresolved questions for next session}

### Context
- {any important context the next session needs}
```

## AGENTS.md Addition

Add this guidance to your project's AGENTS.md:

```markdown
## Agent Coordination

### Session Start
1. Register: `register_agent(project_key, program, model)`
2. Check inbox: `fetch_inbox(project_key, agent_name)`
3. Summarize handoff context before proceeding

### Session End
1. Send handoff: `send_message` with completed/decisions/next steps
2. Release reservations: `release_file_reservations`
```

## Notes

- Session lifecycle is advisory, not enforced in code
- Parallel dispatch coordination (file reservations) is more reliable
- Even partial compliance (e.g., just handoff messages) adds value
