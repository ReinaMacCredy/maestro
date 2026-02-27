# Step 11: Codebase Pattern Scan

**Progress: Step 11 of 16** -- Next: Implementation Plan

## Goal
Scan the existing codebase for patterns relevant to this track. Feed findings into plan generation context so the implementation plan builds on existing conventions rather than inventing new ones.

## Execution Rules
- This step is informational -- it produces context for step 12, not user-facing deliverables
- Scan based on project type and FR capability areas from step 8
- Focus on EXISTING patterns, not proposing new ones
- Keep findings concise -- this feeds into plan context, not the spec
- Time-box: spend no more than 5 minutes scanning

## Context Boundaries
- Approved spec (spec.md) is written and available
- Classification (project type) guides what to scan for
- FR capability areas guide which patterns to look for
- This step does NOT modify the spec

## Scan Sequence

1. **Determine Scan Focus**
   Based on project type and FR capability areas, identify what to look for:
   - Similar components/modules to what this track will build
   - API patterns (routing, middleware, error handling)
   - Test conventions (framework, file structure, naming)
   - Database/data patterns (ORM, schema style, migrations)
   - Configuration patterns (env vars, config files)
   - Authentication/authorization patterns (if relevant FRs exist)

2. **Execute Scan**
   Use Glob and Grep to search the codebase:
   - Look for existing implementations of similar capabilities
   - Identify test file conventions and frameworks
   - Find configuration patterns
   - Note directory structure conventions

   Prioritize patterns that directly relate to FR capability areas. Skip areas the track will not touch.

3. **Compile Findings**
   Format as a concise context block:
   ```
   ## Codebase Patterns (for plan generation)

   - Test framework: {framework} in {directory pattern}
   - API pattern: {routing/controller style}
   - Data layer: {ORM/query pattern}
   - Config: {how config is managed}
   - Similar existing: {components/modules that are analogous}
   - Conventions: {naming, file organization, code style notes}
   ```

   Omit categories that are not relevant. Only include what was actually found.

4. **Present to User**
   Show findings briefly:
   - "I scanned the codebase and found these relevant patterns that will inform the implementation plan: {summary}"
   - No approval needed -- this is informational context
   - User can add notes or corrections before proceeding

## Quality Checks
- [ok] Scan focused on relevant patterns (not everything)
- [ok] Findings are concise and actionable
- [ok] Existing conventions identified (not inventing new patterns)
- [ok] Time-boxed (not an exhaustive audit)

## Anti-patterns
- [x] Exhaustive codebase audit -- this is a quick scan for plan context
- [x] Proposing new patterns instead of documenting existing ones
- [x] Skipping this step entirely -- even small codebases have conventions worth noting
- [x] Spending more than 5 minutes scanning
- [x] Including irrelevant patterns that do not relate to this track's FRs

## Next Step
Read and follow `reference/steps/step-12-plan.md`.
