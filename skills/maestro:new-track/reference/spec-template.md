# Specification Template

## Structure

```markdown
# Specification: {title}

## Overview
{One paragraph summarizing the track purpose, scope, and expected outcome.}

## Type
{feature | bug | chore}

## Requirements

### Functional Requirements
1. {FR-1}: {description}
2. {FR-2}: {description}
3. {FR-3}: {description}

### User Interaction
- Interaction type: {UI | API | CLI | Background}
- Entry point: {where the user triggers this}
- Output: {what the user sees/receives}

### Non-Functional Requirements
- Performance: {latency/throughput expectations, or "standard"}
- Security: {auth requirements, data sensitivity, or "standard"}
- Compatibility: {browser/OS/version requirements, or "N/A"}

## Edge Cases & Error Handling
1. {Edge case 1}: {expected behavior}
2. {Edge case 2}: {expected behavior}
3. {Error scenario}: {recovery strategy}

## Out of Scope
- {Thing 1 this track explicitly does NOT cover}
- {Thing 2}

## Acceptance Criteria
- [ ] {Criterion 1 -- testable, specific}
- [ ] {Criterion 2}
- [ ] {Criterion 3}
```

## Question Bank by Type

### Feature Questions
| # | Question | Purpose |
|---|----------|---------|
| 1 | What should this feature do? Core behavior and outcomes. | Functional requirements |
| 2 | How will users interact with it? | Interaction design |
| 3 | Any constraints? (performance, security, compatibility) | Non-functional requirements |
| 4 | Known edge cases or error scenarios? | Robustness |

### Bug Questions
| # | Question | Purpose |
|---|----------|---------|
| 1 | What is happening? Steps to reproduce. | Observed behavior |
| 2 | What should happen instead? | Expected behavior |
| 3 | How critical? Affected users/flows? | Impact assessment |

### Chore Questions
| # | Question | Purpose |
|---|----------|---------|
| 1 | What needs to change and why? | Scope definition |
| 2 | Backward compatibility requirements? | Constraints |
