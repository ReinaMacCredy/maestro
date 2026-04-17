---
name: maestro:brainstorming
description: "Use before any creative work - creating features, building components, adding functionality, or modifying behavior. Explores user intent, requirements and design before implementation."
---

# Brainstorming Ideas Into Designs

## Overview

Help turn ideas into fully formed designs and specs through natural collaborative dialogue.

Start by understanding the current project context, then run the structured interview in `reference/interview-guide.md` to refine the idea. Once you understand what you're building, present the design in small sections (200-300 words), checking after each section whether it looks right so far.

## The Process

**Understanding the idea:**
- Check out the current project state first (files, docs, recent commits)
- Read `reference/interview-guide.md` and follow its question sequence
- Skip questions the user already answered in their initial request
- Offer multiple-choice options where the guide provides them
- Only one question per message (two max if closely related)
- Summarize your understanding before moving to approaches

**Exploring approaches:**
- Propose 2-3 different approaches with trade-offs
- Present options conversationally with your recommendation and reasoning
- Lead with your recommended option and explain why

**Presenting the design:**
- Once you believe you understand what you're building, present the design
- Break it into sections of 200-300 words
- Ask after each section whether it looks right so far
- Cover: architecture, components, data flow, error handling, testing
- Be ready to go back and clarify if something doesn't make sense

## Chaining Into Maestro

Brainstorming produces understanding, not documents. The validated design feeds directly into the maestro workflow -- never into standalone files under `docs/`.

**When the design is validated, chain forward:**

1. **Create the feature**: `maestro feature-create <name>` -- this registers the feature in the maestro tracker
2. **Save discovery context**: `maestro context-write --feature <name> --name brainstorm --content "<validated design>"` -- persists the brainstorming output where the planner can reference it
3. **Choose the planning path** based on complexity:
   - **Simple/well-understood**: `maestro plan-write --feature <name>` -- write the plan directly from the brainstorming output
   - **Ambitious/multi-component**: Load `maestro:design` or `maestro:new-track` for deeper discovery and structured spec generation

The brainstorming output becomes the `## Discovery` section that `plan-write` requires. Do not write design docs to `docs/plans/` -- that bypasses the maestro workflow and leaves the design disconnected from execution.

## Key Principles

- **One question at a time** - Don't overwhelm with multiple questions
- **Multiple choice preferred** - Easier to answer than open-ended when possible
- **Skip what's known** - If the user gave details upfront, acknowledge them and move on
- **YAGNI ruthlessly** - Remove unnecessary features from all designs
- **Explore alternatives** - Always propose 2-3 approaches before settling
- **Incremental validation** - Present design in sections, validate each
- **Be flexible** - Go back and clarify when something doesn't make sense
- **Challenge assumptions** - Surface fragile assumptions, ask what changes if they fail, offer lean fallback options
