# Step 16: Metadata, Registry, Commit & Summary

**Progress: Step 16 of 16** -- Complete

## Goal
Write metadata, update registry, commit all track files, and display summary.

## Execution Rules
- You MUST follow `reference/metadata-and-registry.md` for all schemas and formats
- Commit message format: `chore(maestro:design): add track {track_id}`
- Summary MUST include track ID, type, phase/task counts, and next step

## Execution Sequence

1. **Read Schema**
   Read `reference/metadata-and-registry.md` for metadata.json schema, index.md template, and tracks.md format.

2. **Write metadata.json**
   Set:
   - track_id, type, status ("new"), description
   - created_at and updated_at (ISO 8601)
   - phases and tasks counts (from plan.md)
   - skills: [] (or detected skills from step 13)

   Write to `.maestro/tracks/{track_id}/metadata.json`.

3. **Write index.md**
   Follow the template in `reference/metadata-and-registry.md`.
   Write to `.maestro/tracks/{track_id}/index.md`.

4. **Update Registry**
   Append to `.maestro/tracks.md`:
   ```markdown
   ---
   - [ ] **Track: {track description}**
     *Type: {type} | ID: [{track_id}](./tracks/{track_id}/)*
   ```

5. **Commit**
   ```bash
   git add .maestro/tracks/{track_id} .maestro/tracks.md
   # Include beads state if BR sync was performed
   [ -d ".beads" ] && git add .beads/
   git commit -m "chore(maestro:design): add track {track_id}"
   ```

6. **Display Summary**
   ```
   ## Track Created

   **{track description}**
   - ID: `{track_id}`
   - Type: {type}
   - Phases: {count}
   - Tasks: {count}

   **Files**:
   - `.maestro/tracks/{track_id}/spec.md`
   - `.maestro/tracks/{track_id}/plan.md`
   - `.maestro/tracks/{track_id}/metadata.json`
   - `.maestro/tracks/{track_id}/index.md`

   **Next**: `/maestro:implement {track_id}`
   ```

## Quality Checks
- [ok] metadata.json matches schema from reference
- [ok] index.md written per template
- [ok] tracks.md updated with new entry
- [ok] Git commit successful with correct message format
- [ok] Summary displayed with correct counts and next step

## Anti-patterns
- [x] Forgetting to update tracks.md registry
- [x] Wrong commit message format (must be `chore(maestro:design):` not `chore(maestro:new-track):`)
- [x] Missing phase/task counts in summary
- [x] Not showing the next step (`/maestro:implement`)
- [x] Committing before all files are written
