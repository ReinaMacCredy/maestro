import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import type {
  HandoffEnvelope,
  HandoffPickup,
} from "@/repo/handoff-emitter.port.js";
import { emitHandoff } from "@/service/emit-handoff.js";
import { summarizeHandoff } from "@/shared/lib/projection.js";
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

/**
 * Sort comparator for handoff envelopes. Defensive against malformed
 * envelopes read off disk that lack `created_at` (legacy schema, partial
 * write, hand-edit). Such records sort to the top rather than throwing.
 */
export function compareEnvelopesByCreatedAt(
  a: HandoffEnvelope,
  b: HandoffEnvelope,
): number {
  const ac = typeof a.created_at === "string" ? a.created_at : "";
  const bc = typeof b.created_at === "string" ? b.created_at : "";
  return ac.localeCompare(bc);
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
        "List handoff envelopes at .maestro/handoffs/. Filters: task_id, trigger_verb, include_picked_up (default false = open work only). Paginated (default limit 20, max 100). view='summary' (default) returns id+task_id+trigger_verb+created_at+picked_up; view='full' returns the envelope and pickup metadata. Sorted by created_at ascending. Read-only. Optional to_agent filter (strict exact match; untargeted envelopes excluded when set).",
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
        const emitter = services.handoffEmitter;
        const all = await emitter.list();
        const filtered = all
          .filter((e: HandoffEnvelope) => args.task_id === undefined || e.task_id === args.task_id)
          .filter(
            (e: HandoffEnvelope) =>
              args.trigger_verb === undefined || e.trigger_verb === args.trigger_verb,
          )
          .filter((e: HandoffEnvelope) => args.to_agent === undefined || e.to_agent === args.to_agent)
          .slice()
          .sort(compareEnvelopesByCreatedAt);

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
        const emitter = services.handoffEmitter;
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
        "Write a handoff envelope to .maestro/handoffs/ so a follow-up agent can pick up the task. Used when an agent must hand off mid-stream without going through claim or block (e.g. ship/verify/abandon paths that do not yet emit on their own). The lifecycle verbs claim and block already emit automatically — do not re-emit them. Returns the materialized envelope including the generated id and created_at timestamp. Error codes: HANDOFF_EMIT_FAILED, INVALID_ARG. Optional to_agent addresses the envelope to a specific receiver tool (e.g. 'codex'), enabling inbox-style discovery.",
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
          { emitter: services.handoffEmitter },
          {
            task_id: args.task_id,
            trigger_verb: args.trigger_verb,
            agent_id: agentId,
            ...(args.worktree_path !== undefined ? { worktree_path: args.worktree_path } : {}),
            ...(args.spec_path !== undefined ? { spec_path: args.spec_path } : {}),
            ...(args.reason !== undefined ? { reason: args.reason } : {}),
            ...(args.to_agent !== undefined ? { to_agent: args.to_agent } : {}),
          },
        );
        if (envelope === undefined) {
          return toCallToolResult(
            fail("HANDOFF_EMIT_FAILED", "Handoff emitter is not configured", {
              hints: ["Confirm services.handoffEmitter is wired"],
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
        "Record that the calling agent has read this envelope and is taking the work, so concurrent agents do not duplicate. This is a bookkeeping mark; it does not claim the task — call maestro_task_claim separately. Writes a sidecar at .maestro/handoffs/<id>.picked_up.json using exclusive create; a second pickup attempt returns HANDOFF_ALREADY_PICKED_UP. Error codes: HANDOFF_NOT_FOUND, HANDOFF_ALREADY_PICKED_UP, HANDOFF_PICKUP_FAILED, HANDOFF_MALFORMED. Returns a top-level warnings: string[] field when the envelope's to_agent differs from picked_up_by; the pickup still succeeds.",
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
        const emitter = services.handoffEmitter;
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
        const warnings =
          envelope.to_agent !== undefined && envelope.to_agent !== pickup.picked_up_by
            ? [
                `Envelope was addressed to '${envelope.to_agent}'; picked up by '${pickup.picked_up_by}'. Pickup recorded; verify this is the envelope you intended.`,
              ]
            : undefined;
        return toCallToolResult(
          ok(warnings !== undefined ? { envelope, pickup, warnings } : { envelope, pickup }),
        );
      } catch (err) {
        return toCallToolResult(fromMaestroError(err, "HANDOFF_PICKUP_FAILED"));
      }
    },
  );
}
