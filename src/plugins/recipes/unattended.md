# Unattended

Use this recipe when an external driver is authorized to keep advancing local
accepted work while the user is away. Maestro records state; it does not own a
hidden scheduler or a second lifecycle.

## Loop anatomy

### Perceive

Reconstruct state from `status`, `ready`, `work show`, decisions, and messages.
Do not rely on conversational memory.

### Choose

Select exactly one ready item whose authority and acceptance are already
settled. If none is safe, return dry or blocked instead of inventing work.

### Act

Use `recipe show work` to drive that one item. Stay within local reversible
actions unless the unattended grant explicitly covers a named external action.

### Observe

Verify the item through its real consumer path and record completion evidence.
Stop on approval prompts, secrets, destructive git, or exhausted failure
budget.

### Learn

Record only sourced corrections that will help a later session resume safely.

### Continue

The external driver may select the next ready item after the current unit is
verified. Return one of: next item, dry, blocked, or hard stop.
