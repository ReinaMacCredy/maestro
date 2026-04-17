# Note Patterns

Concrete examples of good and bad notes for common scenarios. Each good note follows the format:
`- [{date}] [{track_id}:{task_name}] {insight with reasoning}`

## Decisions

<Good>
```
- [2026-03-15] [auth-refactor:task-3] Chose JWT over session cookies -- stateless scales better for multi-region deploy, avoids shared session store
```
Specific choice, names alternatives, includes the "why."
</Good>

<Bad>
```
- decided on JWT
```
No date, no track, no reasoning. A future session cannot evaluate whether this still applies.
</Bad>

## Constraints

<Good>
```
- [2026-03-15] [payments:task-1] Stripe API rate limit is 100/sec in test mode; batch operations must throttle with 10ms delay between calls
```
Quantified constraint with actionable mitigation.
</Good>

<Bad>
```
- Stripe has rate limits
```
Every API has rate limits. This tells a future session nothing about what to do differently.
</Bad>

## Discoveries

<Good>
```
- [2026-03-15] [migration:task-2] Postgres NOTIFY payload max is 8000 bytes; large events must use polling fallback via pg_notify_overflow table
```
Exact limit, exact workaround, exact mechanism.
</Good>

<Bad>
```
- found a postgres limitation
```
Which limitation? What was the workaround? Useless to a future session.
</Bad>

## Blocked-by-User-Input

<Good>
```
- [2026-03-15] [deploy:task-4] User chose us-east-1 over eu-west-1 for primary region; latency tradeoff accepted for cost savings (~40% less)
```
Records the decision, the tradeoff, and the reasoning so no one re-asks.
</Good>

<Bad>
```
- user picked a region
```
Which region? Why? A future session will ask the user again.
</Bad>

## Cross-Track Dependencies

<Good>
```
- [2026-03-15] [api-v2:task-1] Depends on auth-refactor track completing JWT middleware (task-2) before API routes can use auth guards
```
Names both tracks, the specific dependency, and the blocking task.
</Good>

<Bad>
```
- need auth first
```
Which auth? Which track? Which task? Unresolvable without asking.
</Bad>

## Priority Context Examples

Priority notes are injected into every session and every worker. They must be hard constraints that affect all work.

<Good>
```
- All new endpoints MUST use the v2 response envelope ({ data, meta, errors })
- Do NOT import from @legacy/* -- migration in progress, use @core/* equivalents
- PostgreSQL 14 only -- no PG15 features (project constraint until Q3)
- Test database is shared; always use transaction rollback, never truncate
```
Each is a constraint that, if violated, creates wrong code. Workers cannot ask -- they need this upfront.
</Good>

<Bad>
```
- Remember to test things
- We're using TypeScript
- Task 3 is blocked
- The codebase is large
```
"Remember to test" is not a constraint. "We're using TypeScript" is obvious from file extensions. "Task 3 is blocked" is status (use task-update). "The codebase is large" is not actionable.
</Bad>
