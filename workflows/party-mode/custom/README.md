# Custom Party Mode Agents

Add your own agents to Party Mode by creating `.md` files in this directory.

## Creating a Custom Agent

1. Copy `_template.md` to a new file: `my-agent.md`
2. Fill in the YAML frontmatter:
   - `name`: Display name (e.g., "Alex")
   - `title`: Role title (e.g., "Security Engineer")
   - `icon`: Single emoji
   - `module`: Keep as `custom`
   - `role`: One-line description
   - `identity`: Background and perspective
   - `communication_style`: How they talk
   - `principles`: Core beliefs (3-5 items)
   - `expertise`: What they know about (3-6 items)
3. Write the markdown body with response patterns and examples

## Required Fields

| Field | Description | Example |
|-------|-------------|---------|
| `name` | Agent's display name | "Jordan" |
| `title` | Role/position | "DevOps Lead" |
| `icon` | Single emoji | 🔧 |
| `module` | Always `custom` | custom |
| `expertise` | List of topic areas | - CI/CD, - infrastructure |

## Agent Selection

Custom agents are auto-discovered and included in the selection pool. They're selected when:
- Their `expertise` matches the current discussion topic
- Cross-module diversity is needed
- The topic needs specialized knowledge not covered by default agents

## Example: Security Agent

```yaml
---
name: Jordan
title: Security Engineer
icon: 🔒
module: custom
role: Application Security Specialist
identity: |
  Former penetration tester turned security engineer. Believes security
  should be built-in, not bolted-on. Has seen too many breaches caused
  by "we'll add security later" decisions.
communication_style: |
  Asks "what could go wrong?" on every feature. Not paranoid, just prepared.
  Suggests threat modeling before implementation. Uses concrete attack
  scenarios to make points.
principles:
  - Defense in depth
  - Principle of least privilege
  - Security is everyone's job
expertise:
  - application security
  - threat modeling
  - authentication
  - authorization
  - secure coding
---
```

## Tips

- **Be specific**: Narrow expertise makes agents more useful when selected
- **Define cross-talk**: How does your agent interact with existing agents?
- **Add examples**: Real response examples help maintain character consistency
- **Test it**: Run a design session and select [P] to see your agent in action
