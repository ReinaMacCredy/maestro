import { CliError, type CliOptions } from "../kernel/cli.ts";

const observerCommands = new Set([
  "brief",
  "bundle list",
  "bundle show",
  "decision list",
  "decision show",
  "dispatch list",
  "dispatch show",
  "doctor",
  "handback show",
  "legacy show",
  "mcp serve",
  "prompt list",
  "ready",
  "recipe list",
  "recipe show",
  "status",
  "supervisor status",
  "trace",
  "version",
  "watch",
  "work list",
  "work show",
]);

const observerPlugins = new Set([
  "brief",
  "bundle",
  "coordination",
  "decision",
  "dispatch",
  "import-rust",
  "lifecycle",
  "mcp",
  "observability",
  "policy-breakdown",
  "policy-dispatch",
  "policy-lifecycle",
  "policy-proof",
  "policy-qa",
  "policy-research",
  "policy-tdd",
  "policy-witness",
  "recipe",
  "supervisor",
  "version",
  "watch",
  "work",
]);

const helpFooter =
  "environment:\n  MAESTRO_READ_ONLY=1  Run only pure observer verbs without persisting session, lease, or liveness updates.";

export interface ObserverMode {
  allowBuiltIn: (name: string) => boolean;
  cli: CliOptions;
  enabled: boolean;
  loadExternalPlugins: boolean;
}

export function observerMode(): ObserverMode {
  const enabled = process.env.MAESTRO_READ_ONLY === "1";
  return {
    allowBuiltIn: (name) => !enabled || observerPlugins.has(name),
    enabled,
    loadExternalPlugins: !enabled,
    cli: {
      helpFooter,
      beforeInvoke(command) {
        if (!enabled || observerCommands.has(command)) return;
        throw new CliError(
          "READ_ONLY",
          `MAESTRO_READ_ONLY=1 blocks ${command}; remove MAESTRO_READ_ONLY and retry`,
          { command },
        );
      },
      beforeUnknown(args) {
        if (!enabled) return;
        const command = args.join(" ");
        throw new CliError(
          "READ_ONLY",
          `MAESTRO_READ_ONLY=1 blocks ${command}; remove MAESTRO_READ_ONLY and retry`,
          { command },
        );
      },
    },
  };
}
