# Specification Template

Enriched specification template for design tracks. Compose from discovery steps 4-9.

```markdown
# Specification: {title}

## Overview
{One paragraph: purpose, scope, expected outcome}

## Type
{feature | bug | chore}

## Vision & Differentiator
{What makes this special. Why build it this way versus alternatives.}

## Success Criteria
### User Success
{Measurable user outcomes -- e.g., "Users complete onboarding in under 3 minutes"}
### Business Success
{Measurable business metrics -- e.g., "Reduce support tickets by 40%"}
### Technical Success
{Quality attributes that matter -- e.g., "P95 response time under 200ms"}

## Product Scope
### MVP (Must-Have)
{Essential for initial delivery}
### Growth (Post-MVP)
{Enhancements for later iterations}
### Vision (Future)
{Dream features, explicitly deferred}

## User Journeys
### Journey 1: {Persona} -- {Scenario}
{Narrative: opening scene, rising action, climax, resolution}
### Journey 2: {Persona} -- {Scenario}
...
### Journey Requirements Summary
{Capabilities revealed by journeys, mapped to FR references}

## Domain Requirements
{Compliance, regulatory, industry-specific constraints. Omit section entirely if domain is general and complexity is low.}

## Requirements
### Functional Requirements
#### {Capability Area 1}
- FR-1: {Actor} can {capability}
- FR-2: {Actor} can {capability}
#### {Capability Area 2}
- FR-3: ...

### User Interaction
- Interaction type: {UI | API | CLI | Background}
- Entry point: {where the user triggers this}
- Output: {what the user sees/receives}

### Non-Functional Requirements
- Performance: {measurable target}
- Security: {specific requirements}
- Scalability: {growth expectations}
- Compatibility: {platform/version requirements}

## Edge Cases & Error Handling
1. {Edge case}: {expected behavior}

## Out of Scope
- {Thing 1 this track explicitly does NOT cover}

## Acceptance Criteria
- [ ] {Criterion 1 -- testable, specific, references FR-N}
- [ ] {Criterion 2}
```
