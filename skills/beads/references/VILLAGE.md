# Village Reference

Comprehensive reference for multi-agent coordination via mcp-beads-village.

## Contents
- Tool Specifications
- State Directories
- Conflict Resolution Patterns
- Team Coordination Protocols
- Troubleshooting

## Tool Specifications

### init
**Purpose**: Join a workspace and establish identity
**Parameters:**
| Parameter | Required | Default | Description |
|-----------|----------|---------|-------------|
| `team` | No | "default" | Team name for filtering |
| `role` | No | "any" | Role: `fe`, `be`, `devops`, `docs`, or custom |
| `leader` | No | false | Enable leader privileges (assign) |

**Example:**
```
init team="platform" role="be" leader=true
```

### claim
**Purpose**: Atomically claim next available task
**Parameters:**
| Parameter | Required | Default | Description |
|-----------|----------|---------|-------------|
| `role` | No | (from init) | Override role filter |
| `priority` | No | any | Filter by priority level |

**Returns**: Task ID and details, or empty if no tasks available

**Example:**
```
claim
claim role="any"
```

### done
**Purpose**: Complete task, release locks, notify team
**Parameters:**
| Parameter | Required | Default | Description |
|-----------|----------|---------|-------------|
| `id` | Yes | - | Task/issue ID to close |
| `msg` | No | "" | Completion message |

**Side effects:**
- Closes the issue
- Releases ALL file reservations held by this agent
- Broadcasts completion to team
- Unblocks dependent tasks

**Example:**
```
done id="bd-42" msg="Implemented with rate limiting"
```

### reserve
**Purpose**: Lock a file for exclusive editing
**Parameters:**
| Parameter | Required | Default | Description |
|-----------|----------|---------|-------------|
| `path` | Yes | - | File path to reserve |
| `ttl` | No | 10 | Lock duration in minutes |

**Returns**: Success or conflict (with holder info)

**Example:**
```
reserve path="src/auth.ts"
reserve path="src/db.ts" ttl=20
```

### release
**Purpose**: Release a file lock
**Parameters:**
| Parameter | Required | Default | Description |
|-----------|----------|---------|-------------|
| `path` | Yes | - | File path to release |

**Example:**
```
release path="src/auth.ts"
```

### msg
**Purpose**: Send message to agents
**Parameters:**
| Parameter | Required | Default | Description |
|-----------|----------|---------|-------------|
| `content` | Yes | - | Message content |
| `to` | No | (broadcast) | Target agent/team name |

**Example:**
```
msg content="API ready for integration" to="frontend"
msg content="Taking break, releasing locks"
```

### inbox
**Purpose**: Read messages
**Parameters:**
| Parameter | Required | Default | Description |
|-----------|----------|---------|-------------|
| `global` | No | false | Include all team messages |

**Example:**
```
inbox
inbox global=true
```

### status
**Purpose**: View team state
**Returns:**
- Online agents with roles
- Active file locks (who holds what)
- In-progress tasks by agent

**Example:**
```
status
```

### assign (Leader only)
**Purpose**: Assign task to specific agent
**Parameters:**
| Parameter | Required | Default | Description |
|-----------|----------|---------|-------------|
| `id` | Yes | - | Task ID |
| `to` | Yes | - | Agent name |

**Example:**
```
assign id="bd-42" to="alice"
```

## State Directories

Village maintains state in several locations:

| Directory | Purpose | Scope |
|-----------|---------|-------|
| `.reservations/` | File lock state | Project |
| `.mail/` | Message queues | Project |
| `~/.beads-village/` | Agent identity, global state | User |

**Note**: `.reservations/` and `.mail/` should be gitignored as they contain ephemeral coordination state.

## Conflict Resolution Patterns

### File Already Locked

```
1. reserve path="src/auth.ts"
   → Error: File locked by "backend-agent" until 14:35

2. status
   → Shows: backend-agent has src/auth.ts

3. msg to="backend-agent" content="Need src/auth.ts for API changes"

4. inbox
   → Wait for response

5. Either:
   - Wait for TTL expiry
   - Wait for explicit release
   - Claim different task
```

### Task Already Claimed

```
1. claim
   → Returns task or empty

If empty but tasks exist:
2. bd ready --json
   → Check what's theoretically available

3. status
   → See who has what claimed

4. Either wait or pick different work
```

### Stale Locks

If an agent crashes, locks expire after TTL (default 10 min).

To force release (admin/recovery only):
```
release path="src/auth.ts" force=true
```

## Team Coordination Protocols

### Session Start Protocol
```
1. init team="<team>" role="<role>"
2. inbox              # Check for messages
3. status             # See team state
4. claim              # Get work
```

### Session End Protocol
```
1. done id="<task>" msg="Summary of work"
   # OR if not complete:
2. msg content="Going offline, releasing locks"
3. release path="<file>"  # For each held file
```

### Handoff Protocol
```
Agent A (finishing):
1. Update bd notes with context
2. msg to="<agent-b>" content="Handing off <task>, see notes"
3. release all files

Agent B (receiving):
1. inbox
2. bd show <task>
3. claim or bd update --status in_progress
4. reserve files as needed
```

### Parallel Work Protocol
```
Leader:
1. init leader=true
2. assign id="bd-frontend" to="alice"
3. assign id="bd-backend" to="bob"

Workers:
1. init role="<assigned>"
2. claim  # Gets assigned work
3. reserve → work → done
```

## Troubleshooting

| Problem | Solution |
|---------|----------|
| `init` fails | Check Node.js 16+ installed, beads CLI available |
| `claim` returns empty | Run `bd ready` to check if work exists, or `status` to see who has claims |
| Lock stuck after crash | Wait for TTL (10 min) or use `release force=true` |
| Messages not received | Check `inbox global=true` for broadcast messages |
| Can't use `assign` | Must have `leader=true` in init |
| Role filtering wrong | Use `claim role="any"` to override |

## Quick Reference

```
# Join workspace
init team="platform" role="be"

# Get work
claim

# Lock files before editing
reserve path="src/file.ts"

# Complete task
done id="bd-42" msg="Summary"

# Communication
msg to="team" content="Update"
inbox
status
```
