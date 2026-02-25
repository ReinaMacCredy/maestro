# Resume Protocol

## Setup State File

Location: `.maestro/setup_state.json`

Check for existing state:
```bash
cat .maestro/setup_state.json 2>/dev/null || echo "{}"
```

If the file exists and contains `last_successful_step`, the previous run was interrupted. Ask:

Ask the user: "A previous setup run was interrupted after step \"{last_successful_step}\". What would you like to do?\n\nCompleted steps will be skipped automatically."
Options:
- **Resume from where I left off** -- Skip already-completed steps
- **Start over** -- Ignore previous progress and run all steps

If "Start over": delete `.maestro/setup_state.json` and treat `last_successful_step` as empty.

If "Resume": retain `last_successful_step` and skip steps whose names appear in the completed set below.

## Step Name Registry

Used for skip logic. A step is skipped if its name sorts at or before `last_successful_step` in this order:

1. `check_existing_context`
2. `detect_maturity`
3. `create_context_directory`
4. `product_definition`
5. `tech_stack`
6. `coding_guidelines`
7. `product_guidelines`
8. `workflow_config`
9. `tracks_registry`
10. `style_guides`
11. `index_md`
12. `first_track`

## State Write Helper

After completing each major step, write:
```bash
echo '{"last_successful_step": "<step_name>"}' > .maestro/setup_state.json
```

## Cleanup on Completion

Remove the state file after setup completes successfully:
```bash
rm -f .maestro/setup_state.json
```
