import {
  CliError,
  requiredPosition,
  stringOption,
  stringOptions,
  type CliInvocation,
  type CliResult,
} from "../../kernel/cli.ts";
import type { PluginContext } from "../../kernel/loader.ts";
import { registerSessionCommand } from "../session-required.ts";
import {
  blockerDetails,
  blockIfNeeded,
  requireWork,
  unresolvedBlockers,
  type GateResult,
  type WorkAddGateInput,
  type WorkGateInput,
  type WorkService,
} from "./records.ts";

export function registerWorkAdd(context: PluginContext, service: WorkService): void {
  context.effect(() =>
    registerSessionCommand(
      context,
      "work add",
      async (invocation): Promise<CliResult> => {
        // room d126: there is no per-item collaboration mode. The flag is
        // declared hidden only so the refusal names the doctrine instead
        // of an unknown-flag error, on every path, before any pane opens.
        if (invocation.options.mode !== undefined) {
          throw new CliError(
            "INVALID_OPTION",
            "--mode is not an option (room d126): items carry no collaboration tag, and a fork is always raised with work note --blocked",
          );
        }
        return context.events.waterfall<CliInvocation, CliResult>("work.add.invoke", invocation, addWork);
      },
      {
        description: "Add a tracked work item.",
        flags: {
          "--kind": {
            description:
              "Set the work kind: feature|task|bug|chore|implement|idea|research (policies key on kind; default task).",
            value: true,
          },
          "--parent": { description: "Attach the item beneath a parent work ID.", value: true },
          "--blocked-by": {
            description: "Add a blocking work ID.",
            value: true,
            multiple: true,
          },
          "--acceptance": { description: "Record the observable acceptance condition.", value: true },
          "--start": { description: "Also start the new item, claiming its lease for this session, as work start does." },
          "--mode": { hidden: true, value: true },
        },
        positionals: [{ name: "title", required: true }],
        rootDescription: "Manage tracked work, leases, dependencies, and evidence.",
      },
    ),
  );

  async function addWork(invocation: CliInvocation): Promise<CliResult> {
    const start = invocation.options.start === true;
    const title = requiredPosition(invocation, 0, "work title");
    const parentId = stringOption(invocation, "parent") ?? null;
    const blockers = stringOptions(invocation, "blocked-by");
    for (const blocker of blockers) requireWork(context, blocker);
    let id = "";
    const now = new Date().toISOString();
    const kind = stringOption(invocation, "kind") ?? "task";
    const acceptance = stringOption(invocation, "acceptance") ?? null;
    const sessionId = context.sessions.current().id;
    const gate = await context.events.waterfall<WorkAddGateInput, GateResult>(
      "work.add",
      { kind, parentId, sessionId, title },
      async () => ({ blocked: false }),
    );
    blockIfNeeded(gate);
    context.store.database.exec("BEGIN IMMEDIATE");
    try {
      if (parentId) {
        const parent = requireWork(context, parentId);
        if (parent.state === "done" || parent.state === "cancelled") {
          throw new CliError(
            "INVALID_STATE",
            `${parentId} is ${parent.state} and cannot accept children; add new top-level work instead: maestro work add "<title>"`,
          );
        }
      }
      id = context.store.nextCountedId("w", ["work", "slp_work"]);
      context.sessions.record("work.add");
      context.store.database
        .query(
          `INSERT INTO work
              (id, title, kind, state, parent_id, acceptance, created_at, updated_at)
             VALUES (?, ?, ?, 'open', ?, ?, ?, ?)`,
        )
        .run(id, title, kind, parentId, acceptance, now, now);
      const insertBlocker = context.store.database.query(
        "INSERT INTO work_blockers (work_id, blocker_id) VALUES (?, ?)",
      );
      for (const blocker of blockers) insertBlocker.run(id, blocker);
      if (start) {
        const unresolved = unresolvedBlockers(context, id);
        if (unresolved.length > 0) {
          const details = blockerDetails(context, id, unresolved, sessionId);
          throw new CliError("BLOCKED", details.reason, {
            blockers: details.blockers,
            command: details.command,
          });
        }
        blockIfNeeded(
          await context.events.waterfall<WorkGateInput, GateResult>(
            "work.start",
            { work: requireWork(context, id), children: [], sessionId },
            async () => ({ blocked: false }),
          ),
        );
        context.sessions.record("work.start");
        context.store.database
          .query("UPDATE work SET state = 'active', held_by = ?, updated_at = ? WHERE id = ?")
          .run(sessionId, now, id);
      }
      context.store.database.exec("COMMIT");
    } catch (error) {
      try {
        context.store.database.exec("ROLLBACK");
      } catch {}
      throw error;
    }
    context.log.append({
      type: "work.add",
      entityType: "work",
      entityId: id,
      sessionId,
      payload: { title, kind, parentId, acceptance, blockers },
    });
    if (!start) return { data: { work: service.get(id) }, text: `${id} added: ${title}` };
    context.log.append({
      type: "work.start",
      entityType: "work",
      entityId: id,
      sessionId,
      payload: { holder: sessionId },
    });
    return {
      data: { work: service.get(id) },
      text: `${id} added: ${title}\n${id} started by ${sessionId}`,
    };
  }
}
