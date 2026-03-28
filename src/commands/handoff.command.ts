import type { Command } from "commander";
import { getServices } from "../services.js";
import { createHandoff } from "../usecases/create-handoff.usecase.js";
import { listHandoffs } from "../usecases/pickup-handoff.usecase.js";
import { generatePrompt } from "../usecases/generate-prompt.usecase.js";
import { output } from "../lib/output.js";
import { NO_SESSION_ID, UNKNOWN_AGENT } from "../domain/defaults.js";
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
    .option("--instructions <text>", "Custom directives for receiving agent")
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
      const hasCreateIntent = opts.sitrep || opts.quickstart || opts.task || opts.plan || opts.instructions;
      const isPromptOnly = opts.prompt !== undefined && !hasCreateIntent;

      if (isPromptOnly) {
        const config = await services.config.load(process.cwd());
        const latest = await services.handoffStore.getLatestPending();
        const handoffId = latest?.handoff.id ?? "HANDOFF_ID";
        const agent = typeof opts.prompt === "string" ? opts.prompt : undefined;
        const prompt = generatePrompt(config, {
          agent,
          handoffId,
          instructions: latest?.handoff.instructions,
          sessionId: latest?.handoff.session.sessionId,
        });
        if (isJson) {
          output(true, { prompt, handoffId }, () => []);
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
          instructions: opts.instructions,
          plan: opts.plan ?? false,
          message: opts.message,
          task: opts.task,
        }, () => []);
        return;
      }

      const config = await services.config.load(process.cwd());

      const handoff = await createHandoff(
        services.git,
        services.sessionDetect,
        config,
        services.handoffStore,
        {
          plan: opts.plan ?? false,
          sitrep: opts.sitrep,
          quickstart: opts.quickstart,
          task: opts.task,
          instructions: opts.instructions,
          message: opts.message,
          session: opts.session,
          noSession: opts.skipSession,
          dir: process.cwd(),
        },
      );

      const agent = typeof opts.prompt === "string" ? opts.prompt : undefined;
      const prompt = generatePrompt(config, {
        agent,
        task: opts.task,
        instructions: opts.instructions,
        sessionId: handoff.session.sessionId,
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

  // Group by session
  const groups = new Map<string, HandoffEnvelope[]>();
  for (const e of list) {
    const key = e.handoff.session.sessionId;
    const group = groups.get(key) ?? [];
    group.push(e);
    groups.set(key, group);
  }

  const lines: string[] = [`${list.length} handoff(s) across ${groups.size} session(s)`, ""];

  for (const [sessionId, envelopes] of groups) {
    const first = envelopes[0]!;
    const agent = first.handoff.session.agent;
    const started = first.handoff.session.startedAt
      ? new Date(first.handoff.session.startedAt).toLocaleString()
      : "";
    const label = sessionId === NO_SESSION_ID
      ? "No session"
      : `${sessionId.slice(0, 8)} (${agent}${started ? ", " + started : ""})`;

    lines.push(`Session: ${label}`);

    for (const e of envelopes) {
      const status = formatStatus(e).padEnd(15);
      const msg = e.handoff.message.length > 45
        ? e.handoff.message.slice(0, 42) + "..."
        : e.handoff.message;
      lines.push(`  ${e.handoff.id}  ${status}  ${msg}`);
    }

    lines.push("");
  }

  return lines;
}

function formatStatus(e: HandoffEnvelope): string {
  if (e.status === "picked-up" && e.pickedUpBy && e.pickedUpBy !== UNKNOWN_AGENT) {
    return `picked-up ${e.pickedUpBy}`;
  }
  return e.status;
}
