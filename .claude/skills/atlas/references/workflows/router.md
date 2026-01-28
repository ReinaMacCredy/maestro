---
name: atlas-router
version: 1.0.0
description: Workflow router that detects user intent and routes to appropriate Atlas workflow mode
triggers:
  - "@atlas-plan"
  - "@plan"
  - "@oracle"
  - "@explore"
  - "@librarian"
---

# Atlas Router

Routes user requests to the appropriate workflow mode based on keywords and intent detection.

## Trigger Keywords

| Keyword | Mode | Description |
|---------|------|-------------|
| `@atlas-plan` | Atlas Planning | Interview-first planning mode for Atlas |
| `@plan`, `ultraplan` | Prometheus | Interview-first planning mode |
| `@oracle` | Oracle Consultation | Strategic advisor delegation |
| `@explore` | Explore | Codebase search delegation |
| `@librarian`, `@library` | Librarian | External documentation research |
| `@momus`, `@review-plan` | Momus | Plan review and validation |
| `@metis`, `@pre-plan` | Metis | Pre-planning analysis |
| `ultrawork`, `ulw` | High-Priority | Maximum thoroughness mode |

## Example Usage

```
User: "@atlas-plan add user authentication"
→ Atlas planning mode: Interview first, then plan to .claude/plans/

User: "/atlas-work"
→ Execute Atlas plan from .claude/plans/
```
