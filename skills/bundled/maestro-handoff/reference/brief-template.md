# Handoff Brief Template (Worked Example)

Use this as a scaffold when writing a brief to `/tmp/maestro-handoff-<timestamp>.md`. Replace all bracketed placeholders with concrete content from the conversation; delete sections that have nothing to say rather than leaving filler text.

```
## Task

Finish migrating the auth middleware from express-session to JWT. Complete
the rotation logic in `src/auth/jwt-rotator.ts` and wire it into the login
handler at `src/routes/auth.ts:42`.

## Context

Legal flagged the session-token storage for non-compliance. Rotation must
happen server-side so refresh tokens never reach the client. The team agreed
on 15-minute access tokens and 7-day refresh tokens during the design review.

## Relevant Files

- `src/auth/jwt-rotator.ts`: the rotator being built; `rotateRefreshToken` is
  90% done, the last step (idempotency key check) is missing.
- `src/routes/auth.ts`: login/logout handlers. Login at line 42 still calls
  `createSession`; should call `jwtRotator.issue()`.
- `tests/integration/auth.test.ts`: existing session tests; must keep passing
  under the JWT path by the end of this task.
- `docs/auth-migration.md`: the approved design doc.

## Current State

- `jwt-rotator.ts`: token generation works, refresh-token rotation is 90%
  complete, idempotency check is the only missing piece.
- `auth.ts`: still on the old `createSession` path; unchanged so far.
- Tests: session-based tests still green; no JWT integration tests yet.

## What Was Tried

- Reusing the express-session refresh mechanism inside the rotator: rejected
  because it leaks the refresh token back to the client.
- Storing rotation state in Redis: deferred; the first pass uses Postgres so
  we don't add a new runtime dependency mid-migration.

## Decisions

- 15-minute access tokens, 7-day refresh tokens (agreed in design review).
- Rotation state in Postgres, not Redis, for v1.
- Refresh tokens are HTTP-only cookies, never returned in JSON responses.

## Acceptance Criteria

- [ ] `rotateRefreshToken` idempotency key check is implemented and covered
      by a unit test.
- [ ] `auth.ts:42` login handler calls `jwtRotator.issue()` instead of
      `createSession`.
- [ ] `tests/integration/auth.test.ts` still passes, now exercising the JWT
      path.
- [ ] `docs/auth-migration.md` is updated to reflect the implemented rotation
      semantics.

## Constraints

- Do not change the public login response shape; the frontend does not
  change in this task.
- Do not remove `createSession` yet; a separate task covers rollback
  compatibility.
- Keep unrelated edits out of this task.
```

Notes:

- Sections with nothing to say should be deleted, not padded with "N/A" or
  "No prior decisions were attached". Filler text is noise to the receiver.
- `Acceptance Criteria` should be falsifiable. "Works correctly" is not.
- `Constraints` should name things the receiver might otherwise assume are in
  scope. Be specific.
