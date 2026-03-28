import type { MaestroConfig } from "./types.js";

export const MAESTRO_DIR = ".maestro";

export const DEFAULT_CONFIG: MaestroConfig = {
  sessionDetection: {
    enabled: true,
    agents: ["claude-code"],
  },
};

export const CASS_INSTALL_HINT = "Install: brew install dicklesworthstone/tap/cass";

export const DEFAULT_PROMPT_TEMPLATE = `You are picking up a handoff from another agent. Run these commands to get oriented:

1. maestro handoff-pickup --claim --agent {{agent}} --markdown
2. maestro doctor --json

Follow the quickstart in the handoff output.
Read AGENTS.md for code style.
{{#task}}Your task: {{task}}
{{/task}}If you need more context about a decision or implementation detail, search the previous agent's conversation history:
  maestro handoff-dig "<your question>" --json

Use maestro handoff-report --content "summary of work done" --json when done.`;

export const AGENT_INSTRUCTION_BLOCK = `## Cross-Agent Handoff (maestro)

Projects with \`.maestro/\` support handoffs between agents.

**Pick up a handoff assigned to you:**
\`\`\`bash
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

**When to use**: User mentions another agent, wants to delegate, or explicitly asks for a handoff.`;
