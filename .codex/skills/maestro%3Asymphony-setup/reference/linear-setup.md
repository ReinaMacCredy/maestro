# Linear Setup for Symphony

Symphony uses Linear as its issue tracker and work queue. The agent reads issues from a Linear project, works on them, and updates their status as work progresses. This requires specific Linear configuration that standard Linear setups do not include.

## Required Custom Statuses

Symphony's ticket state machine uses three non-standard Linear issue statuses. These must be manually created in Linear's Team Settings because they cannot be added via the API.

| Status Name | State Type | Purpose | When Set |
|-------------|-----------|---------|----------|
| `Rework` | Started | Agent needs to address PR review feedback | After reviewer requests changes |
| `Human Review` | Started | PR is ready for human review | After agent pushes code and creates PR |
| `Merging` | Started | PR is approved, agent is squash-merging | After all checks pass and PR is approved |

All three must be **Started** state type (not Unstarted, Completed, or Canceled). This is a common mistake -- using the wrong state type will break Symphony's status transition logic.

## Creating Custom Statuses

### Step-by-step in Linear UI

1. Open **Linear** --> **Settings** (gear icon, bottom-left)
2. Navigate to **Teams** --> select the team that owns the project
3. Click **Workflow** in the left sidebar
4. Scroll to the **Started** section
5. Click **Add status** (or the `+` button)
6. For each missing status:
   - **Name**: Enter exactly as shown above (case-sensitive: `Rework`, not `rework`)
   - **Description** (optional but recommended):
     - Rework: "Agent addressing PR review feedback"
     - Human Review: "PR ready for human review"
     - Merging: "Agent merging approved PR"
   - **Color**: Choose any color (does not affect functionality)
7. Click **Save** / **Create**

### Verification via Linear API

After creating the statuses, verify they exist programmatically:

```graphql
query TeamWorkflowStates($teamId: String!) {
  team(id: $teamId) {
    states {
      nodes {
        name
        type
      }
    }
  }
}
```

Expected output should include:

```json
{
  "data": {
    "team": {
      "states": {
        "nodes": [
          { "name": "Rework", "type": "started" },
          { "name": "Human Review", "type": "started" },
          { "name": "Merging", "type": "started" }
        ]
      }
    }
  }
}
```

### Verification via Linear MCP

If you have the Linear MCP server configured:

```
Tool: list_issue_statuses
Input: { "teamId": "<team-id>" }
```

Check the response for all three status names with type "started".

### Finding the team ID

The team ID is needed for status verification. Find it via:

```graphql
query {
  teams {
    nodes {
      id
      name
      key
    }
  }
}
```

Or via MCP:

```
Tool: list_teams
Input: {}
```

Match the team name or key to identify the correct team.

## Ticket State Machine

Symphony drives issues through this state machine:

```
Todo
  |
  v
In Progress  <---------+
  |                     |
  | (agent pushes PR)   | (review feedback)
  v                     |
Human Review  ----------+
  |
  | (PR approved, CI green)
  v
Merging
  |
  | (squash-merge complete)
  v
Done

  Rework  <-- branched from Human Review when reviewer requests changes
    |
    | (agent addresses feedback, pushes)
    v
  Human Review (returns to review cycle)
```

### State transitions in detail

| From | To | Trigger | Agent action |
|------|----|---------|-------------|
| Todo | In Progress | Symphony picks up the issue | Agent starts working |
| In Progress | Human Review | Agent creates/updates PR | `push` skill creates PR |
| Human Review | Rework | Reviewer requests changes | Detected by `land_watch.py` (exit code 2) |
| Rework | Human Review | Agent addresses feedback | Agent pushes fixes, updates PR |
| Human Review | Merging | PR approved + CI green | `land_watch.py` returns 0 |
| Merging | Done | Squash-merge complete | `land` skill merges and closes |

### Status transitions the agent performs

The agent uses the `linear` skill to update issue status. Example mutation:

```graphql
mutation UpdateIssueState($issueId: String!, $stateId: String!) {
  issueUpdate(id: $issueId, input: { stateId: $stateId }) {
    success
    issue {
      id
      state {
        name
      }
    }
  }
}
```

The agent must first query the team's workflow states to get the `stateId` for the target status name.

## Common Issues

### Wrong state type

**Symptom**: Symphony skips issues or gets stuck in status transitions.
**Cause**: Custom statuses created with wrong state type (e.g., "Unstarted" instead of "Started").
**Fix**: Delete the incorrectly typed status in Team Settings --> Workflow, recreate with "Started" type.

### Case sensitivity

**Symptom**: Symphony cannot find the status to transition to.
**Cause**: Status name created with different casing (e.g., "human review" instead of "Human Review").
**Fix**: Rename the status in Team Settings to match exactly: `Rework`, `Human Review`, `Merging`.

### Multiple teams

**Symptom**: Statuses exist in one team but the project belongs to a different team.
**Cause**: The custom statuses were created in the wrong team.
**Fix**: Identify which team owns the Linear project and create statuses in that team's workflow settings.

### API key permissions

**Symptom**: GraphQL queries return authorization errors.
**Cause**: LINEAR_API_KEY lacks permissions for the target workspace or team.
**Fix**: Generate a new personal API key from Linear Settings --> API --> Personal API keys. Ensure the key has access to the workspace containing the project.

## Testing the Integration

After setup, verify the full integration works:

1. **Create a test issue** in the Linear project (assign it, set to "Todo")
2. **Start Symphony**: `./bin/symphony /path/to/WORKFLOW.md --port 4000`
3. **Watch Symphony pick up the issue** -- it should transition to "In Progress"
4. **Monitor the agent's work** -- it should make changes, commit, push, and create a PR
5. **Review the PR** -- approve it to trigger the "Merging" transition
6. **Verify the merge** -- the PR should be squash-merged and the issue moved to "Done"

If any step fails, use the `debug` Codex skill to inspect Symphony and Codex logs.
