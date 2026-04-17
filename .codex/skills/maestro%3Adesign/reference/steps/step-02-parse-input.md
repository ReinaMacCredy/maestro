# Step 2: Parse Input & Generate Track ID

**Progress: Step 2 of 16** -- Next: Create Directory

## Goal
Extract track description and generate a unique track identifier.

## Execution

1. Extract the track description from `$ARGUMENTS`. If empty, ask the user: "What are you building? Describe the feature, fix, or change."

2. Auto-infer track type from description keywords:
   - **feature**: add, build, create, implement, support, introduce
   - **bug**: fix, broken, error, crash, incorrect, regression, timeout, fail
   - **chore**: refactor, cleanup, migrate, upgrade, rename, reorganize, extract

   If ambiguous, confirm with the user.

3. Generate track ID: `{shortname}_{YYYYMMDD}` format.
   - Extract 2-4 key words from description
   - Convert to snake_case
   - Append today's date as YYYYMMDD
   - Example: "Add user authentication with OAuth" --> `user_auth_oauth_20260227`

4. Scan `.maestro/tracks/` for directories starting with the same short name prefix. Warn if duplicate found.

## Next Step
Read and follow `reference/steps/step-03-create-dir.md`.
