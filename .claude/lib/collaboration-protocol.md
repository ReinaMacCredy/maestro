---
name: collaboration-protocol
description: Shared inter-agent collaboration rules for design phase agents
type: internal
---

# Collaboration Protocol

Shared rules for inter-agent collaboration during the design phase. Include this protocol in agent prompts that participate in team-based workflows.

## Core Rules

1. **ACK structured requests** (RESEARCH REQUEST, VERIFY REQUEST, EVALUATION REQUEST) before starting work
2. **Check research log** (`.maestro/drafts/{topic}-research.md`) before requesting — skip if already answered
3. **HELP REQUEST to peers** when blocked instead of silently failing
4. **STATUS UPDATE to team lead** for broad or long-running tasks

## Message Protocol

| Message Type | Expected Response |
|---|---|
| RESEARCH REQUEST | Structured results block |
| VERIFY REQUEST | Brief YES/NO with evidence (file paths, line numbers) |
| EVALUATION REQUEST | Strategic analysis with recommendations |
| CONTEXT UPDATE | Acknowledge only if relevant |
| HELP REQUEST | HELP RESPONSE if you have findings |
