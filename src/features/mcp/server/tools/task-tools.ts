import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import {
  blockTasks,
  claimTask,
  createTask,
  listTasks,
  planTasks,
  unblockTasks,
  updateTask,
  type BatchInput,
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
  TaskPlanInput,
  TaskUnblockInput,
} from "../schemas/inputs.js";
import type { RegisterDeps } from "./types.js";

export function registerTaskTools(server: McpServer, deps: RegisterDeps): void {
  server.registerTool(
    "maestro_task_list",
    {
      title: "List maestro tasks",
      description:
        "List tasks. Filters: plan_id, status, type, priority, label, parentId, assignee. Paginated (default limit 20, max 100). view='summary' (default) returns slug+id+title+status+type+priority+blockedByCount; view='full' returns the full Task. Sorted by createdAt asc. Read-only.",
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
        // Wire param is `plan_id` (v2 vocab); v1 Task domain still carries
        // `missionId`. Phase 5 aligns both when MCP rewires onto v2 use cases.
        const filtered = args.plan_id
          ? tasks.filter((t) => t.missionId === args.plan_id)
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

  server.registerTool(
    "maestro_task_plan",
    {
      title: "Create Task Batch",
      description: `Create multiple tasks atomically from a plan. Supports idempotent replay via batchId and optional auto-start.

Input:
- batchId (optional): Idempotency key. If provided and matches an existing receipt, returns the stored result.
- tasks: Array of 1-500 task definitions. Each task has:
  - name (optional): Batch-local symbolic name for parent/blockedBy references
  - title (required): Task title
  - description, type, priority, labels (optional)
  - parent (optional): tsk-* id, batch-local name, or slug (for step tasks)
  - slug (optional): '<verb>/<kebab>' for top-level tasks only (auto-derived from title if omitted)
  - blockedBy (optional): Array of tsk-* ids or batch-local names
- start (optional): Batch-local name of task to auto-claim and move to in_progress after batch creation

Returns:
- batchId (if provided)
- created: Array of {name?, id, status, assignee?}
- replayed: true if batchId matched existing receipt
- startedTaskId: ID of the started task (if start was provided)

Error codes: INVALID_ARG, BATCH_VALIDATION_FAILED, TASK_NOT_FOUND, OWNERSHIP_CONFLICT, STALE_RECEIPT`,
      inputSchema: TaskPlanInput,
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

        // Convert MCP input to BatchInput domain type
        const batchInput: BatchInput = {
          batchId: args.batchId,
          tasks: args.tasks.map(t => ({
            name: t.name,
            title: t.title,
            description: t.description,
            type: t.type,
            priority: t.priority,
            parent: t.parent,
            slug: t.slug,
            labels: t.labels,
            blockedBy: t.blockedBy,
          })),
        };

        // Create the batch
        const result = await planTasks(
          services.taskStore,
          batchInput,
          {
            sessionId,
            gitAnchor: services.gitAnchor,
            continuationStore: services.taskContinuationStore,
            nowMdWriter: services.taskNowMdWriter,
          },
        );

        // Handle --start equivalent
        let startedTaskId: string | undefined;
        if (args.start) {
          const targetTask = result.created.find(t => t.name === args.start);
          if (!targetTask) {
            return toCallToolResult(
              fail("TASK_NOT_FOUND", `No task with name '${args.start}' in batch`, {
                hints: [`Available names: ${result.created.filter(t => t.name).map(t => t.name).join(", ")}`],
                arg: "start",
              }),
            );
          }

          // Update to in_progress (auto-claims if not already claimed)
          const { task: updated } = await updateTask(
            services.taskStore,
            targetTask.id,
            { status: "in_progress" },
            { sessionId },
          );

          startedTaskId = updated.id;

          // Update the result to reflect the started task's new status
          const idx = result.created.findIndex(t => t.id === targetTask.id);
          if (idx !== -1) {
            result.created[idx] = {
              ...result.created[idx],
              status: updated.status,
              assignee: updated.assignee,
            };
          }
        }

        return toCallToolResult(
          ok({
            batchId: result.batchId,
            created: result.created,
            replayed: result.replayed,
            ...(startedTaskId ? { startedTaskId } : {}),
          }),
        );
      } catch (err) {
        return toCallToolResult(fromMaestroError(err, "BATCH_CREATION_FAILED"));
      }
    },
  );
}
