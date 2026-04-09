import { homedir } from "node:os";
import { join } from "node:path";
import type { MaestroConfig } from "./types.js";
import type { MemoryConfig } from "@/features/memory";

export const MAESTRO_DIR = ".maestro";

export const MEMORY_DIR = "memory";

export const GRAPH_DIR = join(homedir(), ".maestro", "graph");

export const DEFAULT_CONFIG: MaestroConfig = {
  sessionDetection: {
    enabled: true,
    agents: ["claude-code"],
  },
  defaultWorkflow: "plan-implement",
  execution: {
    defaultWorker: "codex",
    stopOnFailure: true,
    retryBudget: 1,
    rotateWorkerOnRetry: false,
  },
  workers: {
    "claude-code": {
      enabled: true,
      transport: "cli",
      command: "claude",
      args: ["--print"],
      outputMode: "stream-json",
    },
    codex: {
      enabled: true,
      transport: "cli",
      command: "codex",
      args: [],
      outputMode: "raw",
    },
    // [WIP] Gemini worker -- registered but not integration-tested; disabled by default
    gemini: {
      enabled: false,
      transport: "cli",
      command: "gemini",
      args: [],
      outputMode: "stream-json",
    },
  },
  supervision: {
    level: "mid",
    staleAfterMs: 300_000,
    killGraceMs: 5_000,
    progressIntervalMs: 30_000,
  },
  // [WIP] Parallel execution -- config/UI scaffolding only; runtime always runs sequentially
  parallel: {
    enabled: false,
    maxConcurrent: 1,
  },
  ui: {
    missionControl: {
      backgroundMode: "solid",
    },
  },
  memory: {
    enabled: true,
    corrections: { enabled: true, matching: "keyword", auto_capture: "prompt", severity_default: "soft" },
    learnings: { enabled: true, compile_threshold: 5, max_age_days: 7 },
    ratchet: { enabled: false, enforcement: "warn" },
    graph: { enabled: true },
  } satisfies MemoryConfig,
};

/**
 * Phase 1 strip: the old AGENT_INSTRUCTION_BLOCK described deleted
 * handoff-* commands. This replacement block advertises only the
 * mission/feature/memory surfaces that survive the v1.0.0 strip.
 * Phase 2 will extend it with the UKI handoff workflow.
 * The block is now static (no `{{agent}}` placeholder) because the
 * legacy `handoff-pickup --agent <slug>` flow is gone.
 */
export const AGENT_INSTRUCTION_BLOCK = `## Maestro Conductor (shared score)

Projects with \`.maestro/\` hold mission and memory state that all agents share.

**See what is in flight:**
\`\`\`bash
maestro status --json
maestro mission list --json
maestro feature list --mission <id> --json
\`\`\`

**Read a worker prompt (with injected memory):**
\`\`\`bash
maestro feature prompt <featureId> --mission <id>
\`\`\`

**Capture a correction rule for future sessions:**
\`\`\`bash
maestro memory-correct "use bun not npm" --trigger "package,install,npm"
\`\`\`

**Report feature progress:**
\`\`\`bash
maestro feature update <featureId> --mission <id> --status <status> --report @report.json
\`\`\`

**When to use**: Start every session with \`maestro status\` to see shared state. Use \`maestro feature prompt\` to read the current feature's briefing with memory context auto-injected.`;
