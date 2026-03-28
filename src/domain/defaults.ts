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
{{/task}}Use maestro handoff-report --content "summary of work done" --json when done.`;
