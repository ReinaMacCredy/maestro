# Example: Dispatch Three Parallel Agents

Annotated example of coordinated parallel dispatch.

## Scenario

User wants to update three skills simultaneously:
- Update beads skill documentation
- Add new section to conductor skill
- Fix typo in design skill

## Step 1: Parse Tasks for Files

Extract file patterns from each task:

| Task | Detected Files |
|------|----------------|
| "Update beads skill documentation" | `skills/beads/**` |
| "Add new section to conductor skill" | `skills/conductor/**` |
| "Fix typo in design skill" | `skills/design/**` |

## Step 2: Reserve Files

```
file_reservation_paths(
  project_key: "/path/to/workspace",
  agent_name: "BlueLake",
  paths: [
    "skills/beads/**",
    "skills/conductor/**",
    "skills/design/**"
  ],
  ttl_seconds: 3600,
  exclusive: true
)
```

**Output to user:**
```
🔒 Reserved: skills/beads/**, skills/conductor/**, skills/design/** (1h)
Dispatching 3 agents...
```

## Step 3: Inject Coordination Block

Each Task prompt gets a coordination block:

### Agent 1: Beads Skill

```markdown
Update the beads skill documentation to include the new compact commands.

---
**Coordination:**
- Working inside reservation: skills/beads/**
- If you need files outside this, call `register_agent` then `file_reservation_paths`
- On conflict with unreserved file: warn + skip
- Do NOT release reservations; coordinator handles cleanup
---
```

### Agent 2: Conductor Skill

```markdown
Add a new "CODEMAPS" section to the conductor skill.

---
**Coordination:**
- Working inside reservation: skills/conductor/**
- If you need files outside this, call `register_agent` then `file_reservation_paths`
- On conflict with unreserved file: warn + skip
- Do NOT release reservations; coordinator handles cleanup
---
```

### Agent 3: Design Skill

```markdown
Fix the typo in design skill ("desing" → "design").

---
**Coordination:**
- Working inside reservation: skills/design/**
- If you need files outside this, call `register_agent` then `file_reservation_paths`
- On conflict with unreserved file: warn + skip
- Do NOT release reservations; coordinator handles cleanup
---
```

## Step 4: Dispatch via Task Tool

```typescript
Task("Update beads skill documentation...");
Task("Add CODEMAPS section to conductor skill...");
Task("Fix typo in design skill...");
// All three run concurrently
```

## Step 5: Collect Results

Wait for all agents to complete. Each returns summary of changes.

## Step 6: Release Reservations

```
release_file_reservations(
  project_key: "/path/to/workspace",
  agent_name: "BlueLake"
)
```

**Output to user:**
```
🔓 Released reservations
```

## Handling Conflicts

If Step 2 returned conflicts:

```json
{
  "granted": ["skills/beads/**", "skills/design/**"],
  "conflicts": [
    {"path": "skills/conductor/**", "holders": ["GreenCastle"]}
  ]
}
```

**Response:**
```
🔒 Reserved: skills/beads/**, skills/design/** (1h)
⚠️ skills/conductor/** reserved by GreenCastle - skipping
Dispatching 2 agents (conductor task deferred)...
```

## Handling MCP Failure

If `file_reservation_paths` times out:

```
⚠️ Agent coordination unavailable - proceeding without file locks
Dispatching 3 agents...
```

Agents proceed without coordination. Risk of conflicts, but work continues.
