# Resume Protocol

## Setup State File

Location: `.maestro/setup_state.json`

Check for existing state:
```bash
cat .maestro/setup_state.json 2>/dev/null || echo "{}"
```

## Resume Decision Tree

```
Does .maestro/setup_state.json exist?
  |
  +-- NO  --> Fresh run. No resume needed. Continue to Step 3.
  |
  +-- YES --> Does it contain a valid "last_successful_step"?
        |
        +-- NO  --> Corrupted state. Delete the file. Warn user.
        |           Treat as fresh run.
        |
        +-- YES --> Display completed steps and remaining steps.
                    Ask the user what to do.
```

Ask the user: "A previous setup run was interrupted after step \"{last_successful_step}\".\n\nCompleted: {list of completed step names}\nRemaining: {list of remaining step names}\n\nWhat would you like to do?"
Options:
- **Resume from where I left off** -- Skip already-completed steps
- **Start over** -- Ignore previous progress and run all steps

If "Start over": delete `.maestro/setup_state.json` and treat `last_successful_step` as empty.

If "Resume": retain `last_successful_step` and skip steps whose names appear in the completed set below.

## Step Name Registry

Used for skip logic. A step is skipped if its name sorts at or before `last_successful_step` in this order:

| # | Step Name | What It Does | Files Created |
|---|-----------|-------------|---------------|
| 1 | `check_existing_context` | Check for prior context files | _none_ |
| 2 | `detect_maturity` | Brownfield/greenfield classification + scan | _none_ |
| 3 | `create_context_directory` | `mkdir -p .maestro/context` | `.maestro/context/` |
| 4 | `product_definition` | Product purpose, users, features | `.maestro/context/product.md` |
| 5 | `tech_stack` | Languages, frameworks, tools | `.maestro/context/tech-stack.md` |
| 6 | `coding_guidelines` | Principles, conventions, NFRs | `.maestro/context/guidelines.md` |
| 7 | `product_guidelines` | Voice, tone, UX, branding | `.maestro/context/product-guidelines.md` |
| 8 | `workflow_config` | Methodology, commits, coverage | `.maestro/context/workflow.md` |
| 9 | `tracks_registry` | Initialize tracks.md | `.maestro/tracks.md` |
| 10 | `style_guides` | Copy code style guides | `.maestro/context/code_styleguides/` |
| 11 | `index_md` | Generate context index | `.maestro/context/index.md` |
| 12 | `first_track` | Optional first track creation | `.maestro/tracks/{slug}/` |

## State Write Helper

After completing each major step, write:
```bash
echo '{"last_successful_step": "<step_name>"}' > .maestro/setup_state.json
```

## Resume Verification

When resuming, verify that files from completed steps actually exist on disk:

```
For each completed step:
  Does the expected file/directory exist?
    |
    +-- YES --> Step is truly complete. Skip it.
    |
    +-- NO  --> Step recorded as complete but file is missing.
                Warn: "Step {name} was marked complete but {file} is missing."
                Ask: re-run this step, or skip anyway?
```

This handles cases where a user manually deleted files between sessions.

## Cleanup on Completion

Remove the state file after setup completes successfully:
```bash
rm -f .maestro/setup_state.json
```

If setup completes but the final commit fails (e.g., no git), still remove the state file. The context files are written regardless of commit status.
