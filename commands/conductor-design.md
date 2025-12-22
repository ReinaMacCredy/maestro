---
description: Design a feature or bug fix using collaborative dialogue, then optionally create spec and plan
argument-hint: "[description]"
---

# Conductor Design

Design a feature or bug fix through collaborative dialogue, with conductor context pre-loaded.

Design for: $ARGUMENTS

## 1. Verify Setup

Check these files exist:
- `conductor/product.md`
- `conductor/tech-stack.md`
- `conductor/workflow.md`

If missing, tell user to run `/conductor-setup` first.

## 2. Resolve Track ID

1. **Guard:** If `$ARGUMENTS` is empty or whitespace-only, ask: "Briefly describe the feature or bug this design is for."

2. Check for existing track directory: `ls -d "conductor/tracks/$ARGUMENTS" 2>/dev/null`
   - If directory exists: Use `$ARGUMENTS` as `track_id` (resuming existing design)

3. Otherwise:
   - Treat `$ARGUMENTS` as the initial description
   - From the description, derive a shortname (e.g., `auth`, `billing`, `search`)
   - **Check for existing tracks with same shortname:**
     ```bash
     ls -d conductor/tracks/${shortname}_* 2>/dev/null | head -1
     ```
   - If match found:
     - Use existing directory name as `track_id`
     - Inform user: "Found existing track: `<track_id>`"
   - If no match:
     - Generate new `track_id`: `shortname_YYYYMMDD` (use today's date)

4. Announce: "Working on design for track `<track_id>`."

## 3. Create Track Folder

- Ensure the track directory exists:
  ```bash
  mkdir -p conductor/tracks/<track_id>/
  ```
- Do NOT create `spec.md` or `plan.md` yet - this command focuses on `design.md` first

## 4. Load Conductor Context

Before starting the design process, load:
1. `conductor/product.md` - Product vision, users, goals
2. `conductor/tech-stack.md` - Technology choices and constraints
3. `conductor/workflow.md` - Development methodology (TDD, commits)

Also load (if they exist) any prior artifacts for this track:
- `conductor/tracks/<track_id>/design.md` (resume existing design)

## 5. Design Process

Follow this collaborative dialogue process:

**Understanding the idea:**
- Ask questions one at a time to refine the idea
- Prefer multiple choice questions when possible
- Only one question per message
- Focus on: purpose, constraints, success criteria

**Exploring approaches:**
- Propose 2-3 different approaches with trade-offs
- Lead with your recommended option and explain why

**Presenting the design:**
- Once you understand what you're building, present the design
- Break it into sections of 200-300 words
- Ask after each section: "Does this look right so far?"
- Cover: architecture, components, data flow, error handling, testing

## 6. Ground the Design (Required)

Before finalizing, verify all architectural decisions against current reality.

**Preferred:** Use `/ground <question>` command which automatically selects the right verification approach.

**Manual alternatives:**
- For external libraries/APIs: Use `web_search` to verify patterns against current docs
- For existing patterns: Use `Grep` and `finder` to confirm "how we do X here"
- For prior decisions: Use `find_thread` to check "did we solve this before?"

Do NOT proceed until grounding confirms the design is based on verified, current information.

## 7. Write design.md

When the design is validated and approved:

1. Write the design to: `conductor/tracks/<track_id>/design.md`

2. Structure the file as:
   ```markdown
   # <Track Title>

   ## Overview
   ...

   ## Goals and Non-Goals
   ...

   ## Architecture and Components
   ...

   ## Data and Interfaces
   ...

   ## Risks and Open Questions
   ...

   ## Acceptance and Success Criteria
   ...
   ```

3. Confirm: "Design for `<track_id>` saved to `conductor/tracks/<track_id>/design.md`."

## 8. Offer Track Creation

After `design.md` is written and reviewed:

1. Ask: "Create track now (spec + plan for `<track_id>`)?"

2. If **No**:
   - Reply: "Okay. Run `/conductor-newtrack <track_id>` later to generate spec and plan."
   - End here.

3. If **Yes**:
   - Load the `conductor-newtrack` skill workflow
   - Execute it for `<track_id>` (it will detect and use `design.md`)
   - Generate `spec.md` and `plan.md` in the same folder
   - Present for review and refinement

## 9. Final Message

After track creation (or when user chooses not to create immediately):

> "Plan approved. Say `fb` to file issues."

## Key Principles

- **One question at a time** - Don't overwhelm with multiple questions
- **Multiple choice preferred** - Easier to answer than open-ended
- **YAGNI ruthlessly** - Remove unnecessary features from designs
- **Explore alternatives** - Always propose 2-3 approaches before settling
- **Incremental validation** - Present design in sections, validate each
- **Ground before documenting** - Verify decisions against current reality
