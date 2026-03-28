import type { Command } from "commander";
import { getServices } from "../services.js";
import { createHandoff } from "../usecases/create-handoff.usecase.js";
import { listHandoffs } from "../usecases/pickup-handoff.usecase.js";
import { generatePrompt } from "../usecases/generate-prompt.usecase.js";
import { output } from "../lib/output.js";
import type { HandoffEnvelope, MaestroConfig } from "../domain/types.js";

export function registerHandoffCommand(program: Command): void {
  program
    .command("handoff")
    .description("Create a handoff payload for another agent")
    .addHelpText("after", `
Examples:
  maestro handoff --prompt codex --task "implement note"    # fast mode (auto-sitrep)
  maestro handoff --session $(maestro session -q) --prompt codex --task "fix auth"
  maestro handoff --skip-session --prompt codex              # skip session detection
  maestro handoff --prompt codex                            # prompt for latest pending
  maestro handoff --list
`)
    .option("--list", "List all handoffs with status")
    .option("--sitrep <text>", "Situation report (decisions, status, blockers)")
    .option("--quickstart <text>", "First steps for the receiving agent")
    .option("--plan", "Include plan state from .maestro/plan.json")
    .option("--message <text>", "Short summary message")
    .option("--prompt [agent]", "Generate agent prompt (optionally specify agent name)")
    .option("--task <text>", "Task description to include in the prompt")
    .option("--session <id>", "Use a specific session ID (prefix match supported)")
    .option("--skip-session", "Skip session detection entirely")
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

      // Prompt-only mode: --prompt without any creation intent
      const hasCreateIntent = opts.sitrep || opts.quickstart || opts.task || opts.plan;
      const isPromptOnly = opts.prompt !== undefined && !hasCreateIntent;

      if (isPromptOnly) {
        const config = await services.config.load(process.cwd());
        const latest = await services.handoffStore.getLatestPending();
        const handoffId = latest?.handoff.id ?? "HANDOFF_ID";
        const agent = typeof opts.prompt === "string" ? opts.prompt : undefined;
        const prompt = generatePrompt(config, { agent, handoffId });
        if (isJson) {
          console.log(JSON.stringify({ prompt, handoffId }, null, 2));
        } else {
          printPrompt(prompt, agent ?? config.defaultAgent);
        }
        return;
      }

      if (opts.dryRun) {
        output(true, {
          dryRun: true,
          sitrep: opts.sitrep,
          quickstart: opts.quickstart,
          plan: opts.plan ?? false,
          message: opts.message,
          task: opts.task,
        }, () => []);
        return;
      }

      const handoff = await createHandoff(
        services.git,
        services.sessionDetect,
        services.config,
        services.handoffStore,
        {
          plan: opts.plan ?? false,
          sitrep: opts.sitrep,
          quickstart: opts.quickstart,
          task: opts.task,
          message: opts.message,
          session: opts.session,
          noSession: opts.skipSession,
          dir: process.cwd(),
        },
      );

      // Always generate a prompt after creation
      const config = await services.config.load(process.cwd());
      const agent = typeof opts.prompt === "string" ? opts.prompt : undefined;
      const prompt = generatePrompt(config, {
        agent,
        task: opts.task,
        handoffId: handoff.id,
      });

      output(isJson, { ...handoff, prompt }, (h) => [
        `[ok] Handoff created: ${h.id}`,
        `  Branch: ${h.git.branch}`,
        `  Session: ${h.session.sessionId}`,
      ]);

      if (!isJson) {
        console.log("");
        printPrompt(prompt, agent ?? config.defaultAgent);
      }
    });
}

function printPrompt(prompt: string, agent?: string): void {
  const label = agent ? `Prompt for ${agent}` : "Prompt (replace TARGET_AGENT)";
  console.log(`--- ${label} ---`);
  console.log(prompt);
  console.log("--- End prompt ---");
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
