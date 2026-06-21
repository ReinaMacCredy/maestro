## Task T1: Lock Droid hook payload attribution
check: Focused hook integration proves a Droid-shaped hook JSON payload with session_id writes to the matching .maestro/runs/<session>/events.jsonl bucket.

## Task T2: Document Droid session identity in shipped skills
after: T1
check: Embedded Maestro skill guidance tells Droid users to read hook JSON stdin session_id and does not advertise a DROID_SESSION_ID env var.

## Task T3: Preserve resource versions and validators
after: T2
check: Resource version guard rows are updated for changed shipped skills and focused validators pass.
