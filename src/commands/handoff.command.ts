import type { Command } from "commander";
import { getServices } from "../services.js";
import { createHandoff } from "../usecases/create-handoff.usecase.js";
import { listHandoffs } from "../usecases/pickup-handoff.usecase.js";
import { output } from "../lib/output.js";
import type { HandoffEnvelope } from "../domain/types.js";

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
        output(isJson, all, formatListTable);
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

function formatListTable(list: readonly HandoffEnvelope[]): string[] {
  if (list.length === 0) return ["No handoffs found"];

  // Compute column widths
  const idWidth = 16;
  const statusWidth = Math.max(
    6,
    ...list.map((e) => formatStatus(e).length),
  );

  const header = `  ${"ID".padEnd(idWidth)}  ${"Status".padEnd(statusWidth)}  Message`;
  const sep = `  ${"----".padEnd(idWidth)}  ${"------".padEnd(statusWidth)}  -------`;

  const rows = list.map((e) => {
    const id = e.handoff.id.padEnd(idWidth);
    const status = formatStatus(e).padEnd(statusWidth);
    const msg = e.handoff.message.length > 50
      ? e.handoff.message.slice(0, 47) + "..."
      : e.handoff.message;
    return `  ${id}  ${status}  ${msg}`;
  });

  return [`${list.length} handoff(s)`, "", header, sep, ...rows];
}

function formatStatus(e: HandoffEnvelope): string {
  if (e.status === "picked-up" && e.pickedUpBy && e.pickedUpBy !== "unknown") {
    return `picked-up ${e.pickedUpBy}`;
  }
  return e.status;
}
