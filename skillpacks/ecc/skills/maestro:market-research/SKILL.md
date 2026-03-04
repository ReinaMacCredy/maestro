---
name: maestro:market-research
description: "Market research, competitive analysis, and industry intelligence with source attribution. Use during maestro:design for research tasks, TAM/SAM/SOM estimates, or competitor analysis."
lifecycle: [design]
domain: [content:research]
---

# Market Research

Produce research that supports decisions, not research theater.

## Maestro Integration

**Lifecycle**: design
**Activates when**: maestro:new-track detects relevant tech in tech-stack.md, or maestro:implement encounters matching task types.

### Phase Guidance
**In maestro:design**: Conduct market research during discovery phase. Produce decision-oriented summaries with source attribution for design specifications.

### Related Skills
- maestro:investor-materials
- maestro:investor-outreach
- maestro:article-writing

## Research Standards

1. Every important claim needs a source.
2. Prefer recent data and call out stale data.
3. Include contrarian evidence and downside cases.
4. Translate findings into a decision, not just a summary.
5. Separate fact, inference, and recommendation clearly.

## Common Research Modes

### Investor / Fund Diligence
Collect:
- fund size, stage, and typical check size
- relevant portfolio companies
- public thesis and recent activity
- reasons the fund is or is not a fit
- any obvious red flags or mismatches

### Competitive Analysis
Collect:
- product reality, not marketing copy
- funding and investor history if public
- traction metrics if public
- distribution and pricing clues
- strengths, weaknesses, and positioning gaps

### Market Sizing
Use:
- top-down estimates from reports or public datasets
- bottom-up sanity checks from realistic customer acquisition assumptions
- explicit assumptions for every leap in logic

### Technology / Vendor Research
Collect:
- how it works
- trade-offs and adoption signals
- integration complexity
- lock-in, security, compliance, and operational risk

## Output Format

Default structure:
1. executive summary
2. key findings
3. implications
4. risks and caveats
5. recommendation
6. sources

## Quality Gate

Before delivering:
- all numbers are sourced or labeled as estimates
- old data is flagged
- the recommendation follows from the evidence
- risks and counterarguments are included
- the output makes a decision easier

---
## Relationship to Maestro Workflow
- `/maestro:new-track` -- Detects this skill during Step 9.5 (skill matching)
- `/maestro:implement` -- Loads this skill's guidance during task execution
- `/maestro:review` -- Uses checklists as review criteria
