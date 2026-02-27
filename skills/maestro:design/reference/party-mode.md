# Party Mode -- Multi-Perspective Review

Party mode is single-agent perspective rotation. You adopt each expert viewpoint in sequence and critique the current content from that angle. Each perspective produces 2-3 SPECIFIC findings (not generic praise or broad observations). Findings get incorporated into the content before proceeding.

When a user selects [P] at any step's A/P/C menu, execute the following protocol.

## Protocol

For each perspective in order:

1. Announce: "Switching to {Role} perspective..."
2. Review current step content through that lens
3. Produce exactly 2-3 specific, actionable findings
4. Each finding MUST reference specific content and propose a concrete change

After all 5 perspectives, present consolidated findings for user approval.

## Perspectives

### 1. PM Perspective

**Focus:** Requirements completeness, scope realism, success criteria measurability.

Check:
- Are requirements complete enough to build from?
- Is scope realistic for the stated timeline/resources?
- Can every success criterion actually be measured?
- Are there unstated assumptions about user behavior?

### 2. Architect Perspective

**Focus:** Technical feasibility, hidden risks, scalability, pattern selection.

Check:
- Are there hidden technical risks not captured in requirements?
- Will this architecture scale to stated NFRs?
- Are there better patterns for this problem domain?
- What are the integration risks with existing systems?

### 3. Dev Perspective

**Focus:** Implementability, edge case coverage, task clarity.

Check:
- Can I build this from the spec as written?
- Are edge cases covered or at least acknowledged?
- Are functional requirements specific enough to implement without guessing?
- Are there ambiguous requirements that would cause rework?

### 4. QA Perspective

**Focus:** Testability, acceptance criteria completeness, failure mode coverage.

Check:
- Is every requirement testable?
- What is missing from acceptance criteria?
- What will break first under load/stress?
- Are error scenarios specified with expected behavior?

### 5. Security Perspective

**Focus:** Attack vectors, auth/authz coverage, data sensitivity.

Check:
- What attack vectors exist in this design?
- Is authentication and authorization fully specified?
- How is sensitive data handled (at rest, in transit, in logs)?
- Are there compliance requirements that affect implementation?

## After All Perspectives

Present consolidated findings as a numbered list. Ask user: "Accept these improvements? (y/n)"

- If yes: incorporate all findings into current content, redisplay A/P/C menu
- If no: discard findings, redisplay A/P/C menu

## Anti-patterns

- Generic praise ("looks good from PM perspective") -- every perspective MUST find something specific
- Repeating the same finding across perspectives -- each MUST surface unique insights
- Findings too broad to act on ("consider security more") -- MUST be specific enough to become a spec change
