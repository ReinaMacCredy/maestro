# Step 13: Detect Relevant Skills

**Progress: Step 13 of 16** -- Next: Plan-to-BR Sync

## Goal
Scan the runtime's installed skill list for skills whose description matches this track's domain or technology. Store matches in metadata.json `skills` array.

## Execution Rules
- This step is automated -- no user interaction needed
- Scan skill descriptions against track keywords (tech stack, domain, capability areas)
- Skip silently if no skills match
- Do NOT install or suggest installing new skills

## Context Boundaries
- Approved spec and plan are written to disk
- The runtime's skill list is available (system-provided skill descriptions)
- Classification data from step 4 (project type, domain)

## Detection Sequence

1. **Gather Keywords**
   Extract matching keywords from:
   - Project type and domain (from step 4 classification)
   - Tech stack mentions in spec.md
   - FR capability areas from spec.md
   - Plan task descriptions from plan.md

2. **Scan Skill List**
   For each installed skill visible in the runtime:
   - Compare skill description against gathered keywords
   - A skill matches if its description contains 2+ keywords from the track
   - Record matched skills with the keywords that triggered the match

3. **Store Results**
   Format matches for metadata.json `skills` array:
   ```json
   {
     "name": "skill-name",
     "relevance": "matched",
     "matched_on": ["keyword1", "keyword2"]
   }
   ```

   If no skills match, set `skills: []`.

4. **Report**
   Briefly inform the user:
   - "Detected {N} relevant skill(s): {names}" -- or "No matching skills detected."
   - No approval needed -- informational only

## Quality Checks
- [ok] Keywords extracted from classification + spec + plan
- [ok] All installed skills scanned
- [ok] Matches based on 2+ keyword overlap (not single-word false positives)
- [ok] Results stored for metadata.json consumption in step 16

## Anti-patterns
- [x] Suggesting users install new skills -- only detect what is already installed
- [x] Matching on single generic keywords ("the", "add", "fix")
- [x] Skipping this step entirely -- even if no matches, confirm "No matching skills detected"

## Next Step
Read and follow `reference/steps/step-14-br-sync.md`.
