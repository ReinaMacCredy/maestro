import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import {
  blockTasks,
  claimTask,
  createTask,
  listTasks,
  unblockTasks,
  type ListTasksFilters,
  type TaskStatus,
} from "@/features/task/index.js";
import type { Services } from "@/services.js";
import { fail, fromMaestroError, ok, toCallToolResult } from "../errors.js";
import { paginate } from "../pagination.js";
import { detectMcpSessionId } from "../session.js";
import {
  TaskBlockInput,
  TaskClaimInput,
  TaskCompleteInput,
  TaskCreateInput,
  TaskGetInput,
  TaskListInput,
  TaskUnblockInput,
} from "../schemas/inputs.js";
import { TaskListOutput, TaskOutput } from "../schemas/outputs.js";

interface RegisterDeps {
  readonly getServices: () => Services;
}

export function registerTaskTools(server: McpServer, deps: RegisterDeps): void {
  server.registerTool(
    "maestro_task_list",
    {
      title: "List maestro tasks",
      description:
        "List maestro tasks with optional filters. Returns paginated results sorted by createdAt ascending. Read-only.",
      inputSchema: TaskListInput,
      outputSchema: TaskListOutput,
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
        const filters: ListTasksFilters = {
          ...(args.status !== undefined ? { status: args.status as TaskStatus } : {}),
        };
        const tasks = await listTasks(services.taskStore, filters);
        const filtered = args.missionId
          ? tasks.filter((t) => t.missionId === args.missionId)
          : tasks;
        const page = paginate(filtered, args.limit, args.offset);
        return toCallToolResult(ok(page));
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
      outputSchema: TaskOutput,
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
        const task = await services.taskStore.get(args.id);
        if (task === undefined) {
          return toCallToolResult(
            fail("TASK_NOT_FOUND", `Task ${args.id} not found`, [
              "Confirm the id with maestro_task_list",
            ]),
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
      outputSchema: TaskOutput,
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
        "Claim a pending task for the current MCP session. Idempotent: claiming a task already owned by this session is a no-op.",
      inputSchema: TaskClaimInput,
      outputSchema: TaskOutput,
      annotations: {
        readOnlyHint: false,
        destructiveHint: false,
        idempotentHint: true,
        openWorldHint: false,
      },
    },
    async (args) => {
      try {
        const services = deps.getServices();
        const sessionId = detectMcpSessionId();
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
        "Mark a task as completed. Optional summary is stored on the task receipt. Errors if the task is already completed.",
      inputSchema: TaskCompleteInput,
      outputSchema: TaskOutput,
      annotations: {
        readOnlyHint: false,
        destructiveHint: false,
        idempotentHint: true,
        openWorldHint: false,
      },
    },
    async (args) => {
      try {
        const services = deps.getServices();
        const sessionId = detectMcpSessionId();
        const result = await services.taskStore.update(
          args.id,
          { status: "completed", summary: args.summary },
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
        "Mark this task as blocking the listed tasks. Maintains bidirectional blocks/blockedBy. Detects cycles.",
      inputSchema: TaskBlockInput,
      outputSchema: TaskOutput,
      annotations: {
        readOnlyHint: false,
        destructiveHint: false,
        idempotentHint: true,
        openWorldHint: false,
      },
    },
    async (args) => {
      try {
        const services = deps.getServices();
        const sessionId = detectMcpSessionId();
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
      outputSchema: TaskOutput,
      annotations: {
        readOnlyHint: false,
        destructiveHint: false,
        idempotentHint: true,
        openWorldHint: false,
      },
    },
    async (args) => {
      try {
        const services = deps.getServices();
        const sessionId = detectMcpSessionId();
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
