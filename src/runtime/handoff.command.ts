import { Command } from "commander";
import { buildV2Services } from "../providers/build-services.js";
import type { HandoffEnvelope } from "../repo/handoff-emitter.port.js";

export interface HandoffCommandV2Options {
  readonly resolveRepoRoot: () => string;
}

function findOrCreateHandoffCommand(program: Command): Command {
  const existing = program.commands.find((c) => c.name() === "handoff");
  if (existing) return existing;
  return program
    .command("handoff")
    .description("Inspect handoff envelopes emitted by task lifecycle verbs");
}

export function registerHandoffV2Commands(
  program: Command,
  opts: HandoffCommandV2Options,
): void {
  const handoff = findOrCreateHandoffCommand(program);

  handoff
    .command("list")
    .description("List handoff envelopes under .maestro/handoffs/")
    .option("--task <id>", "filter by task id")
    .option("--trigger <verb>", "filter by trigger verb (task:claim, task:block, ...)")
    .option("--json", "Output as JSON")
    .action(async (opts2: { task?: string; trigger?: string; json?: boolean }): Promise<void> => {
      const repoRoot = opts.resolveRepoRoot();
      const services = buildV2Services({ repoRoot });
      let envelopes = await services.handoffEmitter.list();
      if (opts2.task) envelopes = envelopes.filter((e) => e.task_id === opts2.task);
      if (opts2.trigger) envelopes = envelopes.filter((e) => e.trigger_verb === opts2.trigger);
      const sorted = [...envelopes].sort((a, b) =>
        a.created_at < b.created_at ? 1 : a.created_at > b.created_at ? -1 : 0,
      );

      if (opts2.json === true || program.opts().json === true) {
        console.log(JSON.stringify(sorted, null, 2));
        return;
      }

      if (sorted.length === 0) {
        console.log("(no handoff envelopes)");
        return;
      }
      for (const env of sorted) {
        const tail = env.agent_id ? ` agent=${env.agent_id}` : "";
        const reason = env.reason ? ` reason="${truncate(env.reason, 60)}"` : "";
        console.log(`${env.id}  ${env.trigger_verb.padEnd(13)} task=${env.task_id}${tail}${reason}`);
      }
    });

  handoff
    .command("show <id>")
    .description("Show a single handoff envelope by id, plus its pickup if recorded")
    .option("--json", "Output as JSON")
    .action(async (id: string, opts2: { json?: boolean }): Promise<void> => {
      const repoRoot = opts.resolveRepoRoot();
      const services = buildV2Services({ repoRoot });
      const envelope = await services.handoffEmitter.get(id);
      const pickup = envelope ? await services.handoffEmitter.getPickup(id) : undefined;

      if (opts2.json === true || program.opts().json === true) {
        console.log(JSON.stringify({ envelope, pickup }, null, 2));
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
