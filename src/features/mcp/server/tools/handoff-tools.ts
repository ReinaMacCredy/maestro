import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import type {
  HandoffEnvelope,
  HandoffPickup,
} from "@/repo/handoff-emitter.port.js";
import { emitHandoff } from "@/service/emit-handoff.js";
import { fail, fromMaestroError, ok, toCallToolResult, type CallToolResult } from "../errors.js";
import { paginate } from "../pagination.js";
import {
  HandoffEmitInput,
  HandoffEmitShape,
  HandoffListInput,
  HandoffPickupInput,
  HandoffShowInput,
} from "../schemas/inputs.js";
import type { RegisterDeps } from "./types.js";

interface HandoffSummary {
  readonly id: string;
  readonly task_id: string;
  readonly trigger_verb: string;
  readonly created_at: string;
  readonly picked_up: boolean;
}

function summarizeHandoff(envelope: HandoffEnvelope, pickedUp: boolean): HandoffSummary {
  return {
    id: envelope.id,
    task_id: envelope.task_id,
    trigger_verb: envelope.trigger_verb,
    created_at: envelope.created_at,
    picked_up: pickedUp,
  };
}

function generatePickupId(): string {
  return `pkp-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
}

export function registerHandoffTools(server: McpServer, deps: RegisterDeps): void {
  server.registerTool(
    "maestro_handoff_list",
    {
      title: "List handoff envelopes",
      description:
        "List handoff envelopes at .maestro/handoffs/. Filters: task_id, trigger_verb, include_picked_up (default false = open work only). Paginated (default limit 20, max 100). view='summary' (default) returns id+task_id+trigger_verb+created_at+picked_up; view='full' returns the envelope and pickup metadata. Sorted by created_at ascending. Read-only.",
      inputSchema: HandoffListInput,
      annotations: {
        readOnlyHint: true,
        destructiveHint: false,
        idempotentHint: true,
        openWorldHint: false,
      },
    },
    async (args): Promise<CallToolResult> => {
      try {
        const services = deps.getServices();
        const emitter = services.v2.handoffEmitter;
        const all = await emitter.list();
        const filtered = all
          .filter((e: HandoffEnvelope) => args.task_id === undefined || e.task_id === args.task_id)
          .filter(
            (e: HandoffEnvelope) =>
              args.trigger_verb === undefined || e.trigger_verb === args.trigger_verb,
          )
          .slice()
          .sort((a: HandoffEnvelope, b: HandoffEnvelope) =>
            a.created_at.localeCompare(b.created_at),
          );

        const annotated: { envelope: HandoffEnvelope; pickup?: HandoffPickup }[] = [];
        for (const envelope of filtered) {
          const pickup = await emitter.getPickup(envelope.id);
          if (!args.include_picked_up && pickup !== undefined) continue;
          annotated.push(pickup !== undefined ? { envelope, pickup } : { envelope });
        }

        const page = paginate(annotated, args.limit, args.offset);
        const projected = args.view === "full"
          ? page
          : {
              ...page,
              items: page.items.map((row) =>
                summarizeHandoff(row.envelope, row.pickup !== undefined),
              ),
            };
        return toCallToolResult(ok(projected));
      } catch (err) {
        return toCallToolResult(fromMaestroError(err, "HANDOFF_LIST_FAILED"));
      }
    },
  );

  server.registerTool(
    "maestro_handoff_show",
    {
      title: "Show a handoff envelope",
      description:
        "Fetch a single handoff envelope by id (hnd-*). Returns {envelope, picked_up?}. Error codes: HANDOFF_NOT_FOUND, HANDOFF_MALFORMED. Read-only.",
      inputSchema: HandoffShowInput,
      annotations: {
        readOnlyHint: true,
        destructiveHint: false,
        idempotentHint: true,
        openWorldHint: false,
      },
    },
    async (args): Promise<CallToolResult> => {
      try {
        const services = deps.getServices();
        const emitter = services.v2.handoffEmitter;
        let envelope: HandoffEnvelope | undefined;
        try {
          envelope = await emitter.get(args.id);
        } catch (parseErr) {
          return toCallToolResult(
            fail(
              "HANDOFF_MALFORMED",
              `Envelope ${args.id} is not valid JSON`,
              {
                hints: ["Inspect .maestro/handoffs/<id>.json for hand-edits"],
              },
            ),
          );
        }
        if (envelope === undefined) {
          return toCallToolResult(
            fail("HANDOFF_NOT_FOUND", `Handoff ${args.id} not found`, {
              hints: ["Discover ids via maestro_handoff_list"],
            }),
          );
        }
        const pickup = await emitter.getPickup(args.id);
        return toCallToolResult(
          ok(pickup !== undefined ? { envelope, picked_up: pickup } : { envelope }),
        );
      } catch (err) {
        return toCallToolResult(fromMaestroError(err, "HANDOFF_SHOW_FAILED"));
      }
    },
  );

  server.registerTool(
    "maestro_handoff_emit",
    {
      title: "Emit a handoff envelope",
      description:
        "Write a handoff envelope to .maestro/handoffs/ so a follow-up agent can pick up the task. Used when an agent must hand off mid-stream without going through claim or block (e.g. ship/verify/abandon paths that do not yet emit on their own). The lifecycle verbs claim and block already emit automatically — do not re-emit them. Returns the materialized envelope including the generated id and created_at timestamp. Error codes: HANDOFF_EMIT_FAILED, INVALID_ARG.",
      inputSchema: HandoffEmitShape,
      annotations: {
        readOnlyHint: false,
        destructiveHint: false,
        idempotentHint: false,
        openWorldHint: false,
      },
    },
    async (rawArgs): Promise<CallToolResult> => {
      const parsed = HandoffEmitInput.safeParse(rawArgs);
      if (!parsed.success) {
        const issue = parsed.error.issues[0];
        return toCallToolResult(
          fail("INVALID_ARG", issue?.message ?? "Invalid handoff_emit input", {
            arg: issue?.path[0]?.toString(),
            hints: ["Pass reason when trigger_verb is task:block"],
          }),
        );
      }
      try {
        const args = parsed.data;
        const services = deps.getServices();
        const agentId = args.agent_id ?? deps.sessionId;
        const envelope = await emitHandoff(
          { emitter: services.v2.handoffEmitter },
          {
            task_id: args.task_id,
            trigger_verb: args.trigger_verb,
            agent_id: agentId,
            ...(args.worktree_path !== undefined ? { worktree_path: args.worktree_path } : {}),
            ...(args.spec_path !== undefined ? { spec_path: args.spec_path } : {}),
            ...(args.reason !== undefined ? { reason: args.reason } : {}),
          },
        );
        if (envelope === undefined) {
          return toCallToolResult(
            fail("HANDOFF_EMIT_FAILED", "Handoff emitter is not configured", {
              hints: ["Confirm services.v2.handoffEmitter is wired"],
            }),
          );
        }
        return toCallToolResult(ok({ envelope }));
      } catch (err) {
        return toCallToolResult(fromMaestroError(err, "HANDOFF_EMIT_FAILED"));
      }
    },
  );

  server.registerTool(
    "maestro_handoff_pickup",
    {
      title: "Mark a handoff envelope as picked up",
      description:
        "Record that the calling agent has read this envelope and is taking the work, so concurrent agents do not duplicate. This is a bookkeeping mark; it does not claim the task — call maestro_task_claim separately. Writes a sidecar at .maestro/handoffs/<id>.picked_up.json using exclusive create; a second pickup attempt returns HANDOFF_ALREADY_PICKED_UP. Error codes: HANDOFF_NOT_FOUND, HANDOFF_ALREADY_PICKED_UP, HANDOFF_PICKUP_FAILED, HANDOFF_MALFORMED.",
      inputSchema: HandoffPickupInput,
      annotations: {
        readOnlyHint: false,
        destructiveHint: false,
        idempotentHint: false,
        openWorldHint: false,
      },
    },
    async (args): Promise<CallToolResult> => {
      try {
        const services = deps.getServices();
        const emitter = services.v2.handoffEmitter;
        let envelope: HandoffEnvelope | undefined;
        try {
          envelope = await emitter.get(args.id);
        } catch {
          return toCallToolResult(
            fail("HANDOFF_MALFORMED", `Envelope ${args.id} is not valid JSON`, {
              hints: ["Inspect .maestro/handoffs/<id>.json for hand-edits"],
            }),
          );
        }
        if (envelope === undefined) {
          return toCallToolResult(
            fail("HANDOFF_NOT_FOUND", `Handoff ${args.id} not found`, {
              hints: ["Discover ids via maestro_handoff_list"],
            }),
          );
        }
        const pickup: HandoffPickup = {
          id: generatePickupId(),
          envelope_id: args.id,
          picked_up_by: args.picked_up_by ?? deps.sessionId,
          picked_up_at: new Date().toISOString(),
          ...(args.note !== undefined ? { note: args.note } : {}),
        };
        await emitter.markPickedUp(args.id, pickup);
        return toCallToolResult(ok({ envelope, pickup }));
      } catch (err) {
        return toCallToolResult(fromMaestroError(err, "HANDOFF_PICKUP_FAILED"));
      }
    },
  );
}
