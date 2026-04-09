import type { Command } from "commander";
import { getServices } from "../services.js";
import { detectSession } from "../usecases/detect-session.usecase.js";
import { output } from "../lib/output.js";
import { MaestroError } from "../domain/errors.js";
import type { AgentSession } from "../domain/types.js";

export function registerSessionCommand(program: Command): void {
  program
    .command("session")
    .description("Detect the current agent session")
    .addHelpText("after", `
Examples:
  maestro session --json             # full session info as JSON
  maestro session -q                 # just the session ID (for scripting)
  maestro session                    # human-readable output
`)
    .option("--json", "Output as JSON")
    .option("-q, --quiet", "Output just the session ID")
    .action(async (opts) => {
      const services = getServices();
      const isJson = opts.json ?? program.opts().json;

      const result = await detectSession(services.sessionDetect, {
        cwd: process.cwd(),
      });

      if (!result) {
        if (opts.quiet) process.exit(1);
        throw new MaestroError("No session detected", [
          "Run inside Claude Code, Codex, or another supported agent",
          "The conductor reads CLAUDECODE / CODEX_THREAD_ID env vars only",
        ]);
      }

      if (opts.quiet) {
        console.log(result.session.sessionId);
        return;
      }

      output(isJson, result, (r) => formatText(r.session));
    });
}

function formatText(s: AgentSession): string[] {
  return [
    `Agent:     ${s.agent}`,
    `Session:   ${s.sessionId}`,
    `Source:    ${s.sourcePath}`,
    ...(s.startedAt
      ? [`Started:   ${new Date(s.startedAt).toLocaleString()}`]
      : []),
  ];
}
