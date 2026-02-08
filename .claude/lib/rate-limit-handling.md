---
name: rate-limit-handling
description: Strategies for detecting and recovering from API rate limits during long-running /work sessions.
type: internal
---

# Rate Limit Handling

Guidance for the orchestrator and workers on handling API rate limits during long-running `/work` sessions.

## Detection

Watch for these signals in Bash tool output or agent errors:

- HTTP `429` status codes
- `rate_limit_exceeded` error messages
- `Too Many Requests` responses
- `overloaded_error` or `capacity` errors
- Repeated timeouts on API calls

## Backoff Strategy

When a rate limit is hit:

| Attempt | Wait Time | Action |
|---------|-----------|--------|
| 1st hit | 60 seconds | Pause, log the event, retry |
| 2nd hit | 120 seconds | Double the wait, retry |
| 3rd hit | 240 seconds | Double again, retry |
| 4th+ hit | 300 seconds (max) | Cap at 5 minutes per retry |

After 5 consecutive failures, stop retrying and report to the orchestrator.

## Worker Guidance

When rate-limited during a task:

1. **Pause** -- do not retry immediately
2. **Log** -- update your task description with `Rate limited at {timestamp}. Waiting {N}s.`
3. **Wait** -- respect the backoff schedule above
4. **Retry** -- attempt the operation again after waiting
5. **Report** -- if retries are exhausted, message the orchestrator with the error details

Do NOT abandon the task on a rate limit. Rate limits are transient.

## Orchestrator Guidance

When managing workers during rate limits:

1. **Single worker hit** -- let the worker self-recover via backoff. No action needed.
2. **Multiple workers hit simultaneously** -- pause all new worker spawning for 2 minutes. Let active workers finish their current backoff cycles.
3. **Persistent limits (>10 minutes)** -- reduce active worker count. Finish current wave before spawning replacements.
4. **Monitoring** -- check task descriptions for `Rate limited` entries during the verification loop.

## Prevention

- Spawn workers in waves of 3-5, not all at once
- Stagger worker start times when possible
- Prefer sequential file operations over parallel when the plan allows
- Use eco mode (`--eco`) to reduce model tier and API pressure
