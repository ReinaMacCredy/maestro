import type { Command } from "commander";
import { getServices } from "../services.js";
import { detectSession } from "../usecases/detect-session.usecase.js";
import { output } from "../lib/output.js";
import type { HandoffSession } from "../domain/types.js";

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
      const config = await services.config.load(process.cwd());
      const isJson = opts.json ?? program.opts().json;

      const result = await detectSession(services.sessionDetect, config, {
        cwd: process.cwd(),
      });

      if (!result) {
        if (opts.quiet) {
          process.exit(1);
        }
        if (isJson) {
          console.log(JSON.stringify({ error: "No session detected" }, null, 2));
        } else {
          console.error("[!] No session detected");
          console.error("    Run inside Claude Code, Codex, or another supported agent");
        }
        process.exit(1);
      }

      if (opts.quiet) {
        console.log(result.session.sessionId);
        return;
      }

      output(isJson, result, (r) => formatText(r.session, r.method, r.stale));
    });
}

function formatText(
  s: HandoffSession,
  method: string,
  stale: boolean,
): string[] {
  return [
    `Agent:     ${s.agent}`,
    `Session:   ${s.sessionId}`,
    `Source:    ${s.sourcePath}`,
    `Method:    ${method}${stale ? " (stale)" : ""}`,
    ...(s.startedAt
      ? [`Started:   ${new Date(s.startedAt).toLocaleString()}`]
      : []),
  ];
}
