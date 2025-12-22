---
name: design
version: "1.1.2"
description: Design Session - collaborative brainstorming to turn ideas into designs. Use when user types "ds" or wants to explore/design a feature before implementation.
license: Apache-2.0
compatibility: Works with Claude Code, Amp Code, Codex, and any Agent Skills compatible CLI
metadata:
  keywords:
    - brainstorming
    - design
    - planning
    - exploration
---

# Design Session (ds)

Turn ideas into fully-formed designs through collaborative dialogue.

## When to Use

Trigger on:
- User types `ds`
- User wants to brainstorm or explore an idea
- User says "design a feature" or "let's think through X"
- Before creating a conductor track

## The Process

### 1. Understand the Context

First, check out the current project state:
- Read existing docs, files, recent commits
- Understand what already exists
- Know the tech stack and constraints

### 2. Explore the Idea

Ask questions **one at a time** to refine the idea:
- Prefer multiple choice questions when possible
- Only one question per message
- Focus on: purpose, constraints, success criteria

### 3. Present Approaches

Propose 2-3 different approaches with trade-offs:
- Lead with your recommended option
- Explain the reasoning
- Let the user choose

### 4. Build the Design

Present the design in small sections (200-300 words each):
- Ask after each section: "Does this look right so far?"
- Cover: architecture, components, data flow, error handling
- Be ready to go back and clarify

### 5. Ground the Design (REQUIRED)

**This is a quality gate - do NOT proceed without grounding.**

Before finalizing, verify all architectural decisions against current reality:

- **External libraries/APIs**: Use `web_search` to verify patterns against current docs
- **Existing patterns**: Use `Grep` and `finder` to confirm "how we do X here"
- **Past decisions**: Search codebase history with `git log`

Do NOT proceed to documentation until grounding confirms design is based on verified, current information—not assumptions or outdated training data.

## After the Design

### Review and Handoff

1. Ask: "Review the design?"
2. Address any feedback
3. When approved, say: **"Design approved. Say `fb` to convert into beads issues."**

If a track doesn't exist yet, suggest running `/conductor-newtrack <description>` first.

## Key Principles

- **One question at a time** - Don't overwhelm
- **Multiple choice preferred** - Easier to answer
- **YAGNI ruthlessly** - Remove unnecessary features
- **Explore alternatives** - Always propose 2-3 approaches
- **Incremental validation** - Present in sections, validate each
- **Be flexible** - Go back when something doesn't make sense
