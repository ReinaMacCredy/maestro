# Conductor Workflow Definitions

This directory contains the **single source of truth** for all Conductor workflow logic.

## Purpose

The workflow definitions in this directory are designed to be:
- **Format-agnostic**: Written in markdown, can be referenced by TOML commands, Claude skills, or any other implementation
- **Centralized**: One place to update workflow logic that applies across all implementations
- **Consistent**: Ensures all AI agents follow the same protocols

## Directory Structure

```
workflows/
├── README.md              # This file
├── setup.md               # Project initialization workflow
├── newtrack.md            # Track creation workflow
├── implement.md           # Task implementation workflow
├── status.md              # Progress reporting workflow
├── revert.md              # Git-aware revert workflow
└── schemas/
    ├── metadata.schema.json        # Track metadata structure
    ├── implement_state.schema.json # Implementation state tracking
    └── setup_state.schema.json     # Setup progress state
```

## How to Use

### For TOML Commands (Gemini CLI)
Reference these workflows in your prompt sections:
```toml
prompt = """
Follow the workflow defined in ~/.gemini/extensions/conductor/workflows/setup.md
"""
```

### For Claude Skills/Commands
Import the workflow logic in your markdown prompts:
```markdown
# Reference: workflows/implement.md
Execute the task implementation workflow as defined.
```

### For Other Implementations
Read and adapt the workflow steps for your specific implementation while maintaining the core logic.

## Maintaining Consistency

When updating workflow logic:

1. **Update the workflow file first** in this directory
2. **Update any implementation-specific files** that reference the workflow
3. **Update schemas** if state file structures change
4. **Test across implementations** to ensure consistency

## Schema Validation

JSON schemas in the `schemas/` directory define the structure of state files:
- Use these for validation in your implementation
- Ensures all implementations produce compatible state files

## Core Principles

All workflows share these principles:
1. **Validate tool calls**: Every operation must be verified
2. **Resume capability**: State files enable resumable operations
3. **User confirmation**: Critical actions require explicit approval
4. **Error handling**: Failures are announced and handled gracefully
5. **Git integration**: All changes are properly committed and documented
