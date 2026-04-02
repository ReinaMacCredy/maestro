import type { MaestroConfig, WorkflowTemplate } from "./types.js";

export const MAESTRO_DIR = ".maestro";

/** Built-in workflow templates */
export const BUILT_IN_WORKFLOWS: Readonly<Record<string, WorkflowTemplate>> = {
  "plan-implement": {
    description: "Standard planning then implementation",
    phases: [
      { kind: "work", label: "Planning", profile: "planning" },
      { kind: "work", label: "Implementation", profile: "implementation" },
    ],
  },
  "plan-review-implement": {
    description: "Review gate before implementation",
    phases: [
      { kind: "work", label: "Planning", profile: "planning" },
      { kind: "gate", label: "Plan Review", profile: "plan-review" },
      { kind: "work", label: "Implementation", profile: "implementation" },
    ],
  },
  "plan-implement-review": {
    description: "Post-implementation review",
    phases: [
      { kind: "work", label: "Planning", profile: "planning" },
      { kind: "work", label: "Implementation", profile: "implementation" },
      { kind: "gate", label: "Code Review", profile: "code-review" },
    ],
  },
  "plan-review-implement-review": {
    description: "Review gates before and after implementation",
    phases: [
      { kind: "work", label: "Planning", profile: "planning" },
      { kind: "gate", label: "Plan Review", profile: "plan-review" },
      { kind: "work", label: "Implementation", profile: "implementation" },
      { kind: "gate", label: "Code Review", profile: "code-review" },
    ],
  },
};

export const DEFAULT_CONFIG: MaestroConfig = {
  sessionDetection: {
    enabled: true,
    agents: ["claude-code"],
  },
  defaultWorkflow: "plan-implement",
};

export const NO_SESSION_ID = "none";

export const UNKNOWN_AGENT = "unknown";

export const DEFAULT_RUNTIME_LEASE_MS = 2 * 60_000;
export const DEFAULT_RUNTIME_STALE_MS = 90_000;
export const DEFAULT_RUNTIME_FAILURE_MS = 5 * 60_000;
export const DEFAULT_RUNTIME_RETRY_BUDGET = 2;

export const CASS_INSTALL_HINT = "Install: brew install dicklesworthstone/tap/cass";

export const DEFAULT_PROMPT_TEMPLATE = `You are picking up a handoff from another agent. Run these commands to get oriented:

1. export MAESTRO_SESSION=$(maestro session -q)
2. maestro handoff-pickup --claim --agent {{agent}} --markdown
3. maestro doctor --json

Follow the quickstart in the handoff output.
Read .maestro/AGENTS.md for project-local code style and bootstrap guidance.
{{#instructions}}Your instructions: {{instructions}}
{{/instructions}}{{#task}}Your task: {{task}}
{{/task}}{{#sessionId}}Session: {{sessionId}}
If you need more context about a decision or implementation detail, search the previous agent's conversation history:
  maestro handoff-dig "<your question>" --session {{sessionId}} --json

{{/sessionId}}Use maestro handoff-report --content "summary of work done" --json when done.`;

export const AGENT_INSTRUCTION_BLOCK = `## Cross-Agent Handoff (maestro)

Projects with \`.maestro/\` support handoffs between agents.

**Pick up a handoff assigned to you:**
\`\`\`bash
export MAESTRO_SESSION=$(maestro session -q)
maestro handoff-pickup --claim --agent {{agent}} --markdown
\`\`\`

**Create a handoff for another agent:**
\`\`\`bash
maestro handoff --session $(maestro session -q) --prompt <agent> --task "description"
\`\`\`

**Search previous agent's conversation for context:**
\`\`\`bash
maestro handoff-dig "<your question>" --json
\`\`\`

**Report completion:**
\`\`\`bash
maestro handoff-report --content "summary" --json
\`\`\`

**When picking up a handoff**: If the briefing includes an \`## Instructions\` section, treat those directives as your primary task objectives. Execute them before exploring broader context. If instructions reference plan phases or tasks, resolve them via \`maestro\` commands.

**When to use**: User mentions another agent, wants to delegate, or explicitly asks for a handoff.`;
