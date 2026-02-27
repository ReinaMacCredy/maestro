# Step 1: Validate Prerequisites

**Progress: Step 1 of 14** -- Next: Parse Input

## Goal
Verify the project is set up for maestro tracks before proceeding.

## Execution

1. Check `.maestro/context/product.md` exists. If missing: tell the user "Run `/maestro:setup` first to initialize project context." Stop.

2. Check `.maestro/tracks.md` exists. If missing, create it:
   ```markdown
   # Track Registry

   Active and completed development tracks.
   ```

3. Read `.maestro/context/product.md` to understand the project. Keep this context for later steps.

## Next Step
Read and follow `reference/steps/step-02-parse-input.md`.
