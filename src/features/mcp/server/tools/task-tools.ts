import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import {
  blockTasks,
  claimTask,
  createTask,
  listTasks,
  unblockTasks,
  type ListTasksFilters,
} from "@/features/task/index.js";
import { summarizeTask } from "@/shared/lib/projection.js";
import { fail, fromMaestroError, ok, toCallToolResult, type CallToolResult } from "../errors.js";
import { paginate } from "../pagination.js";
import {
  TaskBlockInput,
  TaskClaimInput,
  TaskCompleteInput,
  TaskCreateInput,
  TaskGetInput,
  TaskListInput,
  TaskUnblockInput,
} from "../schemas/inputs.js";
import type { RegisterDeps } from "./types.js";

export function registerTaskTools(server: McpServer, deps: RegisterDeps): void {
  server.registerTool(
    "maestro_task_list",
    {
      title: "List maestro tasks",
      description:
        "List tasks. Filters: missionId, status, type, priority, label, parentId, assignee. Paginated (default limit 20, max 100). view='summary' (default) returns slug+id+title+status+type+priority+blockedByCount; view='full' returns the full Task. Sorted by createdAt asc. Read-only.",
      inputSchema: TaskListInput,
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
        const filters: ListTasksFilters = {
          ...(args.status !== undefined ? { status: args.status } : {}),
          ...(args.type !== undefined ? { type: args.type } : {}),
          ...(args.priority !== undefined ? { priority: args.priority } : {}),
          ...(args.label !== undefined ? { label: args.label } : {}),
          ...(args.parentId !== undefined ? { parentId: args.parentId } : {}),
          ...(args.assignee !== undefined ? { assignee: args.assignee } : {}),
        };
        const tasks = await listTasks(services.taskStore, filters);
        const filtered = args.missionId
          ? tasks.filter((t) => t.missionId === args.missionId)
          : tasks;
        const page = paginate(filtered, args.limit, args.offset);
        const projected = args.view === "full"
          ? page
          : { ...page, items: page.items.map(summarizeTask) };
        return toCallToolResult(ok(projected));
      } catch (err) {
        return toCallToolResult(fromMaestroError(err, "TASK_LIST_FAILED"));
      }
    },
  );

  server.registerTool(
    "maestro_task_get",
    {
      title: "Get a maestro task",
      description:
        "Fetch a single task by id. Returns code TASK_NOT_FOUND when the task does not exist. Read-only.",
      inputSchema: TaskGetInput,
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
        const task = await services.taskStore.get(args.id);
        if (task === undefined) {
          return toCallToolResult(
            fail("TASK_NOT_FOUND", `Task ${args.id} not found`, {
              hints: ["Confirm the id with maestro_task_list"],
            }),
          );
        }
        return toCallToolResult(ok({ task }));
      } catch (err) {
        return toCallToolResult(fromMaestroError(err, "TASK_GET_FAILED"));
      }
    },
  );

  server.registerTool(
    "maestro_task_create",
    {
      title: "Create a maestro task",
      description:
        "Create a new top-level task. Slug is derived from title automatically. Each call produces a new task.",
      inputSchema: TaskCreateInput,
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
        const task = await createTask(services.taskStore, {
          title: args.title,
          description: args.description,
        });
        return toCallToolResult(ok({ task }));
      } catch (err) {
        return toCallToolResult(fromMaestroError(err, "TASK_CREATE_FAILED"));
      }
    },
  );

  server.registerTool(
    "maestro_task_claim",
    {
      title: "Claim a maestro task",
      description:
        "Claim a pending task for the current MCP session. Idempotent when this session already owns the task. Error codes: TASK_NOT_FOUND, OWNERSHIP_CONFLICT.",
      inputSchema: TaskClaimInput,
      annotations: {
        readOnlyHint: false,
        destructiveHint: false,
        idempotentHint: true,
        openWorldHint: false,
      },
    },
    async (args): Promise<CallToolResult> => {
      try {
        const services = deps.getServices();
        const { sessionId } = deps;
        const task = await claimTask(services.taskStore, args.id, { sessionId });
        return toCallToolResult(ok({ task }));
      } catch (err) {
        return toCallToolResult(fromMaestroError(err, "TASK_CLAIM_FAILED"));
      }
    },
  );

  server.registerTool(
    "maestro_task_complete",
    {
      title: "Complete a maestro task",
      description:
        "Mark a task as completed. Requires `summary` or its alias `reason` to populate the task receipt. Error codes: INVALID_ARG (missing receipt), TASK_NOT_FOUND, ALREADY_COMPLETED, OWNERSHIP_CONFLICT.",
      inputSchema: TaskCompleteInput,
      annotations: {
        readOnlyHint: false,
        destructiveHint: false,
        idempotentHint: true,
        openWorldHint: false,
      },
    },
    async (args): Promise<CallToolResult> => {
      try {
        const services = deps.getServices();
        const { sessionId } = deps;
        const summary = args.summary ?? args.reason;
        if (summary === undefined || summary.trim().length === 0) {
          return toCallToolResult(fail("INVALID_ARG", "Provide summary or reason for the completion receipt", {
            arg: "summary",
            hints: [
              "Pass a one-line outcome via `summary` or its alias `reason`",
              "Mirrors the CLI rule that `task update --status completed` requires `--reason`",
            ],
          }));
        }
        const result = await services.taskStore.update(
          args.id,
          { status: "completed", summary },
          { sessionId },
        );
        return toCallToolResult(ok({ task: result.task, autoClaimed: result.autoClaimed }));
      } catch (err) {
        return toCallToolResult(fromMaestroError(err, "TASK_COMPLETE_FAILED"));
      }
    },
  );

  server.registerTool(
    "maestro_task_block",
    {
      title: "Add blocker edges from this task",
      description:
        "Mark this task as blocking the listed tasks. Maintains bidirectional blocks/blockedBy. Error codes: TASK_NOT_FOUND, SELF_BLOCK, CYCLE_DETECTED, OWNERSHIP_CONFLICT.",
      inputSchema: TaskBlockInput,
      annotations: {
        readOnlyHint: false,
        destructiveHint: false,
        idempotentHint: true,
        openWorldHint: false,
      },
    },
    async (args): Promise<CallToolResult> => {
      try {
        const services = deps.getServices();
        const { sessionId } = deps;
        const task = await blockTasks(services.taskStore, args.id, args.blockedTaskIds, {
          sessionId,
          force: args.force,
        });
        return toCallToolResult(ok({ task }));
      } catch (err) {
        return toCallToolResult(fromMaestroError(err, "TASK_BLOCK_FAILED"));
      }
    },
  );

  server.registerTool(
    "maestro_task_unblock",
    {
      title: "Remove blocker edges from this task",
      description:
        "Remove blocker edges that this task has on the listed tasks. Idempotent: unblocking missing edges is a no-op.",
      inputSchema: TaskUnblockInput,
      annotations: {
        readOnlyHint: false,
        destructiveHint: false,
        idempotentHint: true,
        openWorldHint: false,
      },
    },
    async (args): Promise<CallToolResult> => {
      try {
        const services = deps.getServices();
        const { sessionId } = deps;
        const task = await unblockTasks(services.taskStore, args.id, args.blockedTaskIds, {
          sessionId,
          force: args.force,
        });
        return toCallToolResult(ok({ task }));
      } catch (err) {
        return toCallToolResult(fromMaestroError(err, "TASK_UNBLOCK_FAILED"));
      }
    },
  );
}
