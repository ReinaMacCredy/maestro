/**
 * Plan scaffold template -- exported as a string to avoid Bun's markdown-to-HTML
 * conversion that occurs when importing .md files as ES modules.
 */
export const PLAN_SCAFFOLD_TEMPLATE = `# Implementation Plan: {{featureName}}

## Discovery

{Describe what you investigated and found. Minimum 100 characters required.}

## Non-Goals

- {What this feature intentionally does NOT do}

## Ghost Diffs

{Files that look like they should change but intentionally won't. "None" if not applicable.}

## Phase 1: {phase title}

### 1. {task title}

{Description -- what this task does and why}

#### Design
{Approach, architecture decisions, key implementation details}

#### Acceptance Criteria
- [ ] {verifiable condition}

### 2. {task title}
[depends: 1]

{Description}

#### Design
{Approach}

#### Acceptance Criteria
- [ ] {verifiable condition}

## Phase 2: {phase title}

### 3. {task title}
[depends: 1, 2]

{Description}

#### Design
{Approach}

#### Acceptance Criteria
- [ ] {verifiable condition}
`;
