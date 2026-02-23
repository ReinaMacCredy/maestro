# DECIDE Primitive

The DECIDE primitive abstracts over user interaction across CLI environments. Different runtimes expose different interaction mechanisms — some offer structured prompts with clickable options, some offer chat-based text exchange, some run non-interactively. DECIDE resolves through a fallback chain so the orchestrator never calls environment-specific tools directly.

## Primitive Definition

```
DECIDE(
  question: string,          # The question to ask
  options: [{label, description}],  # Available choices
  blocking: boolean,         # Must user respond before continuing?
  default: string           # Auto-selected if non-interactive
)
```

## Resolution Chain

DECIDE resolves in priority order, stopping at the first available capability:

1. **Structured prompt** — if the runtime supports structured prompts with selectable options: present the question with labeled options and wait for user selection.

2. **Chat prompt** — if the runtime supports chat-based interaction but not structured prompts: present the question as formatted text with options listed. Parse the user's text response to match an option label.

3. **Auto-default** — if the runtime is non-interactive or no prompt capability is available: select the `default` value immediately. Emit a log line in the format:

   ```
   [auto] {question} -> {default}
   ```

## Safety Policy

| Decision Type | Blocking? | Auto-Proceed? | Examples |
|--------------|-----------|---------------|----------|
| Destructive / irreversible | Always | Never | Delete files, force-push, drop tables |
| Execution confirmation | Default yes | Yes if low-risk | "Execute this plan?", "Proceed?" |
| Configuration choice | Default yes | Yes with default | "Which runtime?", "Worktree?" |
| Informational | No | Always | Tips, suggestions, warnings |

Rules:

- **Risky = stop**: Destructive or irreversible actions MUST block and wait for explicit user confirmation. Auto-default must not be used regardless of runtime capability.
- **Safe = auto**: Reversible, low-impact decisions may auto-proceed with the `default` when no prompt capability is available.
- **Log all auto-decisions**: Every auto-proceed emits `[auto] {question} -> {default}` so the session record is auditable.

## Usage Examples

### Plan confirmation (destructive gate)

```
DECIDE(
  question: "Execute this plan?",
  options: [
    {label: "Yes, execute", description: "Proceed with team creation"},
    {label: "Cancel", description: "Stop without executing"}
  ],
  blocking: true,
  default: "Cancel"
)
```

`default: "Cancel"` ensures that if auto-default fires, the safe action is chosen. But because execution is potentially irreversible, the orchestrator should treat this as a risky decision and refuse to auto-proceed — blocking is authoritative.

### Worktree choice (configuration)

```
DECIDE(
  question: "Where should this plan execute?",
  options: [
    {label: "Worktree (isolated)", description: "New branch, safe for parallel"},
    {label: "Main tree", description: "Current directory"}
  ],
  blocking: true,
  default: "Main tree"
)
```

This is a configuration decision. If the runtime is non-interactive, auto-proceed with "Main tree" and log `[auto] Where should this plan execute? -> Main tree`.

### Informational notice (non-blocking)

```
DECIDE(
  question: "No wisdom files found for this domain.",
  options: [
    {label: "OK", description: "Acknowledge and continue"}
  ],
  blocking: false,
  default: "OK"
)
```

Non-blocking decisions always auto-proceed. No wait, no log required unless debug mode is active.
