import { Command } from "commander";
import { parseNonNegativeInt, parsePositiveInt } from "@/shared/lib/cli-options.js";
import { stringifyForOutput } from "@/shared/lib/output.js";
import { summarizeHandoff } from "@/shared/lib/projection.js";
import { buildCoreServices } from "../providers/build-services.js";
import type { HandoffEnvelope } from "../repo/handoff-emitter.port.js";

export interface HandoffCommandOptions {
  readonly resolveRepoRoot: () => string;
}

function findOrCreateHandoffCommand(program: Command): Command {
  const existing = program.commands.find((c) => c.name() === "handoff");
  if (existing) return existing;
  return program
    .command("handoff")
    .description("Inspect handoff envelopes emitted by task lifecycle verbs");
}

interface HandoffListFlags {
  task?: string;
  trigger?: string;
  json?: boolean;
  full?: boolean;
  all?: boolean;
  limit?: number;
  offset?: number;
}

export function registerHandoffCommands(
  program: Command,
  opts: HandoffCommandOptions,
): void {
  const handoff = findOrCreateHandoffCommand(program);

  handoff
    .command("list")
    .description(
      "List handoff envelopes under .maestro/handoffs/ (paginated, summary by default)",
    )
    .option("--task <id>", "filter by task id")
    .option("--trigger <verb>", "filter by trigger verb (task:claim, task:block, ...)")
    .option("--json", "Output as JSON")
    .option("--full", "Emit the full envelope shape instead of the summary projection")
    .option("--all", "Drop the default --limit 20 cap")
    .option("--limit <n>", "Limit the number of envelopes returned (default 20)", parsePositiveInt)
    .option("--offset <n>", "Skip the first N envelopes (default 0)", parseNonNegativeInt)
    .action(async (flags: HandoffListFlags): Promise<void> => {
      const repoRoot = opts.resolveRepoRoot();
      const services = buildCoreServices({ repoRoot });
      let envelopes = await services.handoffEmitter.list();
      if (flags.task) envelopes = envelopes.filter((e) => e.task_id === flags.task);
      if (flags.trigger) envelopes = envelopes.filter((e) => e.trigger_verb === flags.trigger);
      const sorted = [...envelopes].sort((a, b) =>
        a.created_at < b.created_at ? 1 : a.created_at > b.created_at ? -1 : 0,
      );

      const offset = flags.offset ?? 0;
      const total = sorted.length;
      const limit = flags.all === true ? Math.max(total - offset, 0) : (flags.limit ?? 20);
      const page = sorted.slice(offset, offset + Math.max(limit, 0));

      if (flags.json === true || program.opts().json === true) {
        const items = flags.full === true
          ? page
          : await Promise.all(
              page.map(async (env) => {
                const pickup = await services.handoffEmitter.getPickup(env.id);
                return summarizeHandoff(env, pickup !== undefined);
              }),
            );
        console.log(stringifyForOutput({ items, total, limit, offset }));
        return;
      }

      if (page.length === 0) {
        console.log("(no handoff envelopes)");
        return;
      }
      for (const env of page) {
        const tail = env.agent_id ? ` agent=${env.agent_id}` : "";
        const reason = env.reason ? ` reason="${truncate(env.reason, 60)}"` : "";
        // Defensive: legacy envelopes on disk may lack trigger_verb or task_id.
        const trigger = typeof env.trigger_verb === "string" ? env.trigger_verb : "(unknown)";
        const taskId = env.task_id ?? "?";
        console.log(`${env.id}  ${trigger.padEnd(13)} task=${taskId}${tail}${reason}`);
      }
    });

  handoff
    .command("show <id>")
    .description("Show a single handoff envelope by id, plus its pickup if recorded")
    .option("--json", "Output as JSON")
    .action(async (id: string, opts2: { json?: boolean }): Promise<void> => {
      const repoRoot = opts.resolveRepoRoot();
      const services = buildCoreServices({ repoRoot });
      const envelope = await services.handoffEmitter.get(id);
      const pickup = envelope ? await services.handoffEmitter.getPickup(id) : undefined;

      if (opts2.json === true || program.opts().json === true) {
        console.log(stringifyForOutput({ envelope, pickup }));
        if (!envelope) process.exitCode = 1;
        return;
      }

      if (!envelope) {
        console.error(`maestro handoff show: envelope ${id} not found`);
        console.error("  List envelopes: maestro handoff list");
        process.exitCode = 1;
        return;
      }
      printEnvelope(envelope);
      if (pickup) {
        console.log("");
        console.log("Pickup:");
        console.log(`  by:    ${pickup.picked_up_by}`);
        console.log(`  at:    ${pickup.picked_up_at}`);
        if (pickup.note) console.log(`  note:  ${pickup.note}`);
      }
    });
}

function printEnvelope(env: HandoffEnvelope): void {
  console.log(`id:           ${env.id}`);
  console.log(`task_id:      ${env.task_id}`);
  console.log(`trigger_verb: ${env.trigger_verb}`);
  console.log(`created_at:   ${env.created_at}`);
  if (env.agent_id) console.log(`agent_id:     ${env.agent_id}`);
  if (env.worktree_path) console.log(`worktree:     ${env.worktree_path}`);
  if (env.spec_path) console.log(`spec_path:    ${env.spec_path}`);
  if (env.reason) console.log(`reason:       ${env.reason}`);
}

function truncate(s: string, n: number): string {
  if (s.length <= n) return s;
  return `${s.slice(0, n - 1)}…`;
}
