# Graceful Fallback Pattern

Handle agent_mail MCP failures without blocking workflow.

## Core Principle

**Coordination is optional; work completion is mandatory.**

Never let MCP unavailability block the user's task.

## Timeout Strategy

All agent_mail calls should use ~3-second mental timeout:
- If no response in ~3s, assume failure
- Log warning, proceed without coordination
- Don't retry failed calls (wastes time)

## Failure Responses

| Operation | On Failure | User Sees |
|-----------|------------|-----------|
| `ensure_project` | Proceed | ⚠️ Coordination unavailable |
| `register_agent` | Proceed | (silent) |
| `file_reservation_paths` | Proceed uncoordinated | ⚠️ Could not reserve files |
| `release_file_reservations` | Proceed (TTL handles) | (silent) |
| `send_message` | Proceed | ⚠️ Handoff not sent |
| `fetch_inbox` | Proceed | (silent) |

## Warning Format

When coordination fails, show:

```
⚠️ Agent coordination unavailable - proceeding without file locks
```

Keep warnings brief; don't alarm user about optional features.

## Silent Failures

Some operations fail silently:
- `register_agent` - Identity not critical for task
- `release_file_reservations` - TTL expires anyway
- `fetch_inbox` - Missing context is suboptimal but not blocking

## Visible Failures

Some failures should warn user:
- `file_reservation_paths` - User should know parallel agents may conflict
- `send_message` (handoff) - User should know context may be lost

## Recovery

If MCP becomes available mid-session:
- Next coordination call will succeed
- No need to retry failed calls
- Previous session's reservations may have expired (OK)

## Implementation Notes

- Don't wrap every call in try/catch visibly
- Use internal timeout logic
- Log failures for debugging but don't surface all to user
- Prioritize completing user's actual task
