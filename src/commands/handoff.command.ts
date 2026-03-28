import type { Command } from "commander";
import { getServices } from "../services.js";
import { createHandoff } from "../usecases/create-handoff.usecase.js";
import { listHandoffs } from "../usecases/pickup-handoff.usecase.js";
import { output } from "../lib/output.js";

export function registerHandoffCommand(program: Command): void {
  program
    .command("handoff")
    .description("Create a handoff payload for another agent")
    .addHelpText("after", `
Examples:
  maestro handoff --list
  maestro handoff --sitrep "Auth done. Refresh blocked." --quickstart "Run: bun test"
  maestro handoff --plan --sitrep "Phase 1 complete" --quickstart "Start phase 2"
  maestro handoff --sitrep "Done" --quickstart "Continue" --message "Short summary"
  maestro handoff --dry-run --sitrep "test" --quickstart "test"
`)
    .option("--list", "List all handoffs with status")
    .option("--sitrep <text>", "Situation report (decisions, status, blockers)")
    .option("--quickstart <text>", "First steps for the receiving agent")
    .option("--plan", "Include plan state from .maestro/plan.json")
    .option("--message <text>", "Short summary message")
    .option("--dry-run", "Show what would be written without writing")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const services = getServices();
      const isJson = opts.json ?? program.opts().json;

      if (opts.list) {
        const all = await listHandoffs(services.handoffStore);
        output(isJson ?? true, all, (list) =>
          list.length === 0
            ? ["No handoffs found"]
            : [
                `${list.length} handoff(s):`,
                ...list.map(
                  (e) =>
                    `  ${e.handoff.id}  [${e.status}]  ${e.handoff.message}`,
                ),
              ],
        );
        return;
      }

      if (!opts.sitrep || !opts.quickstart) {
        console.error("[!] --sitrep and --quickstart are required when creating a handoff");
        console.error("    maestro handoff --sitrep '...' --quickstart '...'");
        console.error("    maestro handoff --list   (to list existing handoffs)");
        process.exit(1);
      }

      if (opts.dryRun) {
        output(true, {
          dryRun: true,
          sitrep: opts.sitrep,
          quickstart: opts.quickstart,
          plan: opts.plan ?? false,
          message: opts.message,
        }, () => []);
        return;
      }

      const handoff = await createHandoff(
        services.git,
        services.cass,
        services.sessionDetect,
        services.handoffStore,
        {
          plan: opts.plan ?? false,
          sitrep: opts.sitrep,
          quickstart: opts.quickstart,
          message: opts.message,
          dir: process.cwd(),
        },
      );

      output(isJson, handoff, (h) => [
        `[ok] Handoff created: ${h.id}`,
        `  Branch: ${h.git.branch}`,
        `  Session: ${h.session.sessionId}`,
        `  CASS indexed: ${h.session.cassIndexed}`,
        `  Pickup: maestro handoff-pickup --json`,
      ]);
    });
}
