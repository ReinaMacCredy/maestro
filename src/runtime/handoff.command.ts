import { Command } from "commander";
import { parseNonNegativeInt, parsePositiveInt } from "@/shared/lib/cli-options.js";
import { stringifyForOutput } from "@/shared/lib/output.js";
import { summarizeHandoff } from "@/shared/lib/projection.js";
import { compareEnvelopesByCreatedAt } from "@/features/mcp/server/tools/handoff-tools.js";
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
  toAgent?: string;
  includePickedUp?: boolean;
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
      "List handoff envelopes under .maestro/handoffs/ (open-only by default; paginated, summary projection by default)",
    )
    .option("--task <id>", "filter by task id")
    .option("--trigger <verb>", "filter by trigger verb (task:claim, task:block, ...)")
    .option("--to-agent <name>", "filter by recipient tool name (strict exact-match)")
    .option(
      "--include-picked-up",
      "include envelopes already marked picked up (default: false, matches MCP)",
    )
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
      if (flags.toAgent) envelopes = envelopes.filter((e) => e.to_agent === flags.toAgent);

      // Defensive sort matches the MCP comparator so CLI and MCP agree on
      // ordering for legacy envelopes that lack `created_at`.
      const sorted = [...envelopes].sort(compareEnvelopesByCreatedAt);

      // Default to "open work only" so CLI matches MCP (`include_picked_up:
      // false`). The pickup filter runs BEFORE pagination/total so `total`
      // reflects the visible result set on both surfaces.
      const includePickedUp = flags.includePickedUp === true;
      const annotated: { envelope: HandoffEnvelope; pickedUp: boolean }[] = [];
      for (const envelope of sorted) {
        const pickup = await services.handoffEmitter.getPickup(envelope.id);
        const pickedUp = pickup !== undefined;
        if (!includePickedUp && pickedUp) continue;
        annotated.push({ envelope, pickedUp });
      }

      const offset = flags.offset ?? 0;
      const total = annotated.length;
      const rawLimit = flags.all === true ? total - offset : (flags.limit ?? 20);
      const limit = Math.max(rawLimit, 0);
      const page = annotated.slice(offset, offset + limit);
      const hasMore = offset + limit < total;

      if (flags.json === true || program.opts().json === true) {
        const items = flags.full === true
          ? page.map((row) => ({ envelope: row.envelope, picked_up: row.pickedUp }))
          : page.map((row) => summarizeHandoff(row.envelope, row.pickedUp));
        // Emit the legacy flat shape AND a nested `pagination` block so
        // downstream consumers can use either path. The nested shape matches
        // MCP `maestro_handoff_list` for cross-surface parity.
        console.log(
          stringifyForOutput({
            items,
            total,
            limit,
            offset,
            pagination: { total, limit, offset, hasMore },
          }),
        );
        return;
      }

      if (page.length === 0) {
        console.log("(no handoff envelopes)");
        return;
      }
      for (const row of page) {
        const env = row.envelope;
        const tail = env.agent_id ? ` agent=${env.agent_id}` : "";
        const toAgentTail = env.to_agent ? ` to_agent=${env.to_agent}` : "";
        const reason = env.reason ? ` reason="${truncate(env.reason, 60)}"` : "";
        // Defensive: legacy envelopes on disk may lack trigger_verb or task_id.
        const trigger = typeof env.trigger_verb === "string" ? env.trigger_verb : "(unknown)";
        const taskId = env.task_id ?? "?";
        const pickupTail = row.pickedUp ? " (picked up)" : "";
        console.log(
          `${env.id}  ${trigger.padEnd(13)} task=${taskId}${tail}${toAgentTail}${reason}${pickupTail}`,
        );
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
  if (env.to_agent) console.log(`to_agent:     ${env.to_agent}`);
  if (env.worktree_path) console.log(`worktree:     ${env.worktree_path}`);
  if (env.spec_path) console.log(`spec_path:    ${env.spec_path}`);
  if (env.reason) console.log(`reason:       ${env.reason}`);
}

function truncate(s: string, n: number): string {
  if (s.length <= n) return s;
  return `${s.slice(0, n - 1)}…`;
}
