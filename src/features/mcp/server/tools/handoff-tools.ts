import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import {
  getHandoffDisplayState,
  listOpenHandoffsForTask,
  listProjectHandoffs,
  pickupHandoff,
  showProjectHandoff,
  type HandoffAgent,
  type HandoffRecord,
} from "@/features/handoff/index.js";
import { buildTaskOwnerId } from "@/features/task/index.js";
import { MaestroError } from "@/shared/errors.js";
import type { z } from "zod";
import { fail, fromMaestroError, ok, toCallToolResult } from "../errors.js";
import { paginate } from "../pagination.js";
import {
  HandoffListInput,
  HandoffOpenForTaskInput,
  HandoffPickupInput,
  HandoffShowInput,
} from "../schemas/inputs.js";
import {
  HandoffListOutput,
  HandoffOpenForTaskOutput,
  HandoffPickupOutput,
  HandoffShowOutput,
} from "../schemas/outputs.js";
import type { RegisterDeps } from "./types.js";

type HandoffListArgs = z.infer<typeof HandoffListInput>;

function applyHandoffFilters(
  records: readonly HandoffRecord[],
  args: HandoffListArgs,
): readonly HandoffRecord[] {
  let filtered: readonly HandoffRecord[] = records;
  if (args.displayState) {
    filtered = filtered.filter((r) => getHandoffDisplayState(r) === args.displayState);
  }
  if (args.taskId) {
    filtered = filtered.filter((r) => r.refs.taskId === args.taskId);
  }
  if (args.agent) {
    filtered = filtered.filter((r) => r.agent === args.agent);
  }
  return filtered;
}

function hintsFrom(err: unknown): readonly string[] {
  return err instanceof MaestroError ? err.hints : [];
}

function messageFrom(err: unknown, fallback: string): string {
  return err instanceof Error ? err.message : fallback;
}

function lowerMessage(err: unknown): string {
  return err instanceof Error ? err.message.toLowerCase() : "";
}

export function registerHandoffTools(server: McpServer, deps: RegisterDeps): void {
  server.registerTool(
    "maestro_handoff_list",
    {
      title: "List handoff packets",
      description:
        "List handoff packets visible from the current maestro project, newest first. Supports optional filters for openOnly, displayState (open|consumed|completed|failed), taskId, and agent. Read-only.",
      inputSchema: HandoffListInput,
      outputSchema: HandoffListOutput,
      annotations: {
        readOnlyHint: true,
        destructiveHint: false,
        idempotentHint: true,
        openWorldHint: false,
      },
    },
    async (args) => {
      try {
        if (args.openOnly === true && args.displayState !== undefined) {
          return toCallToolResult(
            fail(
              "INVALID_FILTER_COMBINATION",
              "Provide at most one of: openOnly, displayState",
              ["Drop openOnly when using displayState; openOnly=true is equivalent to displayState='open'"],
            ),
          );
        }
        const services = deps.getServices();
        const records = await listProjectHandoffs(services.handoffStore, {
          openOnly: args.openOnly ?? false,
          taskStore: services.taskStore,
          currentProjectRoot: services.projectRoot,
        });
        const filtered = applyHandoffFilters(records, args);
        const page = paginate(filtered, args.limit, args.offset);
        return toCallToolResult(ok(page));
      } catch (err) {
        return toCallToolResult(fromMaestroError(err, "HANDOFF_LIST_FAILED"));
      }
    },
  );

  server.registerTool(
    "maestro_handoff_show",
    {
      title: "Show a handoff packet",
      description:
        "Fetch a single handoff packet by id, scoped to the current maestro project. Returns code HANDOFF_NOT_FOUND when the packet does not exist or belongs to another project. Read-only.",
      inputSchema: HandoffShowInput,
      outputSchema: HandoffShowOutput,
      annotations: {
        readOnlyHint: true,
        destructiveHint: false,
        idempotentHint: true,
        openWorldHint: false,
      },
    },
    async (args) => {
      try {
        const services = deps.getServices();
        const record = await showProjectHandoff(services.handoffStore, args.id, {
          taskStore: services.taskStore,
          currentProjectRoot: services.projectRoot,
        });
        return toCallToolResult(ok({ record }));
      } catch (err) {
        if (lowerMessage(err).includes("not found")) {
          const hints = hintsFrom(err);
          return toCallToolResult(
            fail(
              "HANDOFF_NOT_FOUND",
              messageFrom(err, `Handoff ${args.id} not found`),
              hints.length > 0
                ? hints
                : ["Run maestro_handoff_list to see available packets"],
            ),
          );
        }
        return toCallToolResult(fromMaestroError(err, "HANDOFF_SHOW_FAILED"));
      }
    },
  );

  server.registerTool(
    "maestro_handoff_open_for_task",
    {
      title: "List open handoffs linked to a task",
      description:
        "Return ids of open handoff packets linked to the given task, newest first. Project-scoped. Useful for agents that resume work and need to know whether a packet is waiting on a specific task. Read-only.",
      inputSchema: HandoffOpenForTaskInput,
      outputSchema: HandoffOpenForTaskOutput,
      annotations: {
        readOnlyHint: true,
        destructiveHint: false,
        idempotentHint: true,
        openWorldHint: false,
      },
    },
    async (args) => {
      try {
        const services = deps.getServices();
        const ids = await listOpenHandoffsForTask(services.handoffStore, args.taskId, {
          taskStore: services.taskStore,
          currentProjectRoot: services.projectRoot,
        });
        return toCallToolResult(
          ok({ taskId: args.taskId, handoffIds: [...ids] }),
        );
      } catch (err) {
        return toCallToolResult(fromMaestroError(err, "HANDOFF_LIST_FAILED"));
      }
    },
  );

  server.registerTool(
    "maestro_handoff_pickup",
    {
      title: "Pick up a handoff packet",
      description:
        "Consume an open handoff packet. When the packet is task-linked and standalone is false, also claims and resumes the linked task. actorSessionId defaults to the MCP session id; ownerId defaults to buildTaskOwnerId(actorAgent, actorSessionId). Error codes: HANDOFF_NOT_FOUND, ALREADY_CONSUMED, CROSS_PROJECT_PICKUP, HANDOFF_TASK_COMPLETED, HANDOFF_TASK_BLOCKED, OWNERSHIP_CONFLICT.",
      inputSchema: HandoffPickupInput,
      outputSchema: HandoffPickupOutput,
      annotations: {
        readOnlyHint: false,
        destructiveHint: false,
        idempotentHint: false,
        openWorldHint: false,
      },
    },
    async (args) => {
      try {
        const services = deps.getServices();
        const actorAgent: HandoffAgent = args.actorAgent;
        const actorSessionId = args.actorSessionId ?? deps.sessionId;
        const ownerId = args.ownerId ?? buildTaskOwnerId(actorAgent, actorSessionId);
        const result = await pickupHandoff(
          {
            handoffStore: services.handoffStore,
            taskStore: services.taskStore,
            contracts: services.contracts,
            continuationStore: services.taskContinuationStore,
            continuationHistory: services.taskContinuationHistory,
          },
          {
            id: args.id,
            actorAgent,
            actorSessionId,
            ownerId,
            currentProjectRoot: services.projectRoot,
            standalone: args.standalone ?? false,
          },
        );
        return toCallToolResult(
          ok({
            record: result.record,
            ...(result.taskId !== undefined ? { taskId: result.taskId } : {}),
            ...(result.ownerId !== undefined ? { ownerId: result.ownerId } : {}),
            ...(result.contractTransferWarning !== undefined
              ? { contractTransferWarning: result.contractTransferWarning }
              : {}),
            ...(result.unlinkedTaskId !== undefined
              ? { unlinkedTaskId: result.unlinkedTaskId }
              : {}),
          }),
        );
      } catch (err) {
        const lower = lowerMessage(err);
        if (lower.includes("handoff not found") || lower.includes("handoff packet not found")) {
          const hints = hintsFrom(err);
          return toCallToolResult(
            fail(
              "HANDOFF_NOT_FOUND",
              messageFrom(err, `Handoff ${args.id} not found`),
              hints.length > 0
                ? hints
                : ["Run maestro_handoff_list to see available packets"],
            ),
          );
        }
        if (lower.includes("already consumed")) {
          return toCallToolResult(
            fail("ALREADY_CONSUMED", messageFrom(err, "Handoff already consumed"), hintsFrom(err)),
          );
        }
        if (lower.includes("belongs to project")) {
          return toCallToolResult(
            fail(
              "CROSS_PROJECT_PICKUP",
              messageFrom(err, "Handoff is linked to a task from another project"),
              hintsFrom(err),
            ),
          );
        }
        if (
          lower.includes("is already finished") ||
          lower.includes("is already completed and cannot be resumed")
        ) {
          return toCallToolResult(
            fail(
              "HANDOFF_TASK_COMPLETED",
              messageFrom(err, "Linked task is already completed"),
              hintsFrom(err),
            ),
          );
        }
        if (lower.includes("is blocked and cannot be resumed")) {
          return toCallToolResult(
            fail(
              "HANDOFF_TASK_BLOCKED",
              messageFrom(err, "Linked task is blocked"),
              hintsFrom(err),
            ),
          );
        }
        return toCallToolResult(fromMaestroError(err, "HANDOFF_PICKUP_FAILED"));
      }
    },
  );
}
