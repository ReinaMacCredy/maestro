# New Track Workflow

## Purpose
Create a new track (feature, bug fix, or chore) with comprehensive specification and implementation plan.

## Prerequisites
- Conductor environment initialized (setup complete)
- Required files exist:
  - `conductor/tech-stack.md`
  - `conductor/workflow.md`
  - `conductor/product.md`

## State Management
This workflow does not use a persistent state file. Each invocation is atomic.

## Workflow Steps

### Phase 1: Setup Verification

1. **Check Required Files**
   - Verify all prerequisite files exist
   - If missing: Halt with message to run `/conductor:setup`

### Phase 2: Track Initialization

1. **Get Track Description**
   - If arguments provided: use as description
   - If no arguments: prompt user for description

2. **Infer Track Type**
   - Analyze description automatically
   - Types: `feature`, `bug`, `chore`, `refactor`
   - Do NOT ask user to classify

3. **Check for Duplicate**
   - List existing tracks in `conductor/tracks/`
   - Extract short names from existing track IDs
   - If proposed name matches existing: halt, suggest alternative

### Phase 3: Specification Generation

1. **Announce Goal**
   > "I'll guide you through questions to build a comprehensive specification."

2. **Question Phase**
   - **For Features**: 3-5 questions
     - Clarify functionality, implementation, interactions, inputs/outputs
   - **For Bugs/Chores**: 2-3 questions
     - Reproduction steps, scope, success criteria

3. **Question Guidelines**
   - Ask sequentially (one at a time)
   - Provide 2-3 suggested options
   - Always include "Type your own answer" option
   - Classify as Additive or Exclusive Choice
   - Reference `product.md`, `tech-stack.md` for context

4. **Draft `spec.md`**
   - Sections:
     - Overview
     - Functional Requirements
     - Non-Functional Requirements (if applicable)
     - Acceptance Criteria
     - Out of Scope

5. **User Confirmation**
   - Present draft for review
   - Loop until approved or changes incorporated

### Phase 4: Plan Generation

1. **Announce Goal**
   > "Now I will create an implementation plan based on the specification."

2. **Generate Plan**
   - Read confirmed `spec.md`
   - Read `conductor/workflow.md` for methodology
   - Structure: Phases → Tasks → Sub-tasks
   - Include status markers `[ ]`

3. **TDD Task Structure** (if workflow requires)
   ```markdown
   - [ ] Task: [Feature Name]
     - [ ] Write failing tests
     - [ ] Implement to pass tests
     - [ ] Refactor
   ```

4. **Phase Completion Tasks**
   - Check workflow for Phase Completion Protocol
   - If exists, append to each phase:
     ```markdown
     - [ ] Task: Conductor - User Manual Verification '<Phase Name>' (Protocol in workflow.md)
     ```

5. **User Confirmation**
   - Present draft for review
   - Loop until approved

### Phase 5: Create Artifacts

1. **Generate Track ID**
   - Format: `shortname_YYYYMMDD`
   - Short name derived from description

2. **Create Directory**
   ```
   conductor/tracks/<track_id>/
   ```

3. **Create `metadata.json`**
   ```json
   {
     "track_id": "<track_id>",
     "type": "feature|bug|chore|refactor",
     "status": "new",
     "created_at": "YYYY-MM-DDTHH:MM:SSZ",
     "updated_at": "YYYY-MM-DDTHH:MM:SSZ",
     "description": "<user description>"
   }
   ```

4. **Write Files**
   - `spec.md`: confirmed specification
   - `plan.md`: confirmed plan

5. **Update Tracks File**
   - Append to `conductor/tracks.md`:
     ```markdown
     
     ---
     
     ## [ ] Track: <Track Description>
     *Link: [./conductor/tracks/<track_id>/](./conductor/tracks/<track_id>/)*
     ```

6. **Announce Completion**
   > "New track '<track_id>' created. Start implementation with `/conductor:implement`."

## Error Handling

| Error | Action |
|-------|--------|
| Setup not complete | Halt, direct to `/conductor:setup` |
| Duplicate track name | Halt, suggest different name |
| Tool call fails | Halt, announce failure, await instructions |

## Output Artifacts

```
conductor/
├── tracks.md (updated)
└── tracks/
    └── <track_id>/
        ├── metadata.json
        ├── spec.md
        └── plan.md
```
