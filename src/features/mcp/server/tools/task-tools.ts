import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import {
  taskClaim,
  taskBlock,
  taskShip,
  taskFromSpec,
} from "@/service/index.js";
import { refreshNowMdFromServices } from "@/service/refresh-now-md.js";
import {
  FsContractStoreAdapter,
  FsContractVersionStoreAdapter,
} from "@/shared/domain/legacy-task/index.js";
import type { Task } from "@/types/task.js";
import { fail, fromMaestroError, ok, toCallToolResult, type CallToolResult } from "../errors.js";
import { paginate } from "../pagination.js";
import {
  TaskBlockInput,
  TaskClaimInput,
  TaskFromSpecInput,
  TaskGetInput,
  TaskListInput,
  TaskShipInput,
} from "../schemas/inputs.js";
import type { RegisterDeps } from "./types.js";

/** Lean task summary for token-budget mode. */
interface V2TaskSummary {
  readonly id: string;
  readonly slug: string;
  readonly title: string;
  readonly state: string;
  readonly mission_id?: string;
  readonly assignee?: string;
}

function summarizeV2Task(task: Task): V2TaskSummary {
  return {
    id: task.id,
    slug: task.slug,
    title: task.title,
    state: task.state,
    ...(task.mission_id !== undefined ? { mission_id: task.mission_id } : {}),
    ...(task.assignee !== undefined ? { assignee: task.assignee } : {}),
  };
}

export function registerTaskTools(server: McpServer, deps: RegisterDeps): void {
  server.registerTool(
    "maestro_task_list",
    {
      title: "List maestro tasks",
      description:
        "List v2 tasks. Filters: mission_id, state (draft|claimed|doing|verifying|blocked|ready|shipped|abandoned). Paginated (default limit 20, max 100). view='summary' (default) returns id+slug+title+state+mission_id+assignee; view='full' returns the full v2 Task. Sorted by created_at asc. v1 filters (type, priority, label, parentId, assignee) are not supported in v2 — omit them. Read-only.",
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
        let tasks: readonly Task[];
        if (args.mission_id !== undefined) {
          tasks = await services.v2.taskStore.listByMissionId(args.mission_id);
        } else if (args.state !== undefined) {
          tasks = await services.v2.taskStore.listByState(args.state);
        } else {
          tasks = await services.v2.taskStore.list();
        }
        // Apply state filter on top of mission_id (and vice versa) so both filters
        // compose when supplied together, matching the tool description.
        if (args.mission_id !== undefined && args.state !== undefined) {
          tasks = tasks.filter((t) => t.state === args.state);
        }
        const page = paginate(tasks, args.limit, args.offset);
        const projected = args.view === "full"
          ? page
          : { ...page, items: page.items.map(summarizeV2Task) };
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
        "Fetch a single v2 task by id. Returns code TASK_NOT_FOUND when the task does not exist. Read-only.",
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
        const task = await services.v2.taskStore.get(args.id);
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
    "maestro_task_from_spec",
    {
      title: "Create a v2 task from a spec file",
      description:
        "Create a v2 task in draft state from a product-spec markdown file. The spec must be an on-disk markdown file with a valid slug in its YAML frontmatter. Each call creates a new task; use the returned id for subsequent claim/ship. Error codes: TASK_CREATE_FAILED.",
      inputSchema: TaskFromSpecInput,
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
        const task = await taskFromSpec(
          {
            repoRoot: services.projectRoot,
            specStore: services.v2.specStore,
            taskStore: services.v2.taskStore,
            evidenceStore: services.v2.evidenceStore,
            observabilityStore: services.v2.observabilityStore,
          },
          args.spec_path,
        );
        await refreshNowMdFromServices(services.v2);
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
        "Claim a draft task for this agent (draft -> claimed). Auto-creates a worktree for heavy-mode specs. Error codes: TASK_NOT_FOUND, TASK_CLAIM_FAILED.",
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
        const agentId = args.agent_id ?? deps.sessionId;
        const task = await taskClaim(
          {
            taskStore: services.v2.taskStore,
            evidenceStore: services.v2.evidenceStore,
            missionStore: services.v2.missionStore,
            observabilityStore: services.v2.observabilityStore,
            worktreeStore: services.v2.worktreeStore,
            handoffEmitter: services.v2.handoffEmitter,
            contractStore: new FsContractStoreAdapter(services.projectRoot),
            contractVersionStore: new FsContractVersionStoreAdapter(services.projectRoot),
            repoRoot: services.projectRoot,
          },
          { id: args.id, agentId },
        );
        await refreshNowMdFromServices(services.v2);
        return toCallToolResult(ok({ task }));
      } catch (err) {
        return toCallToolResult(fromMaestroError(err, "TASK_CLAIM_FAILED"));
      }
    },
  );

  server.registerTool(
    "maestro_task_ship",
    {
      title: "Ship a maestro task",
      description:
        "Mark a ready task as shipped (ready -> shipped). Optionally record the PR URL. The verdict-PASS path is the authoritative completion receipt. Error codes: TASK_NOT_FOUND, TASK_SHIP_FAILED.",
      inputSchema: TaskShipInput,
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
        const task = await taskShip(
          {
            taskStore: services.v2.taskStore,
            evidenceStore: services.v2.evidenceStore,
            missionStore: services.v2.missionStore,
            observabilityStore: services.v2.observabilityStore,
          },
          { id: args.id, pr_url: args.pr_url },
        );
        await refreshNowMdFromServices(services.v2);
        return toCallToolResult(ok({ task }));
      } catch (err) {
        return toCallToolResult(fromMaestroError(err, "TASK_SHIP_FAILED"));
      }
    },
  );

  server.registerTool(
    "maestro_task_block",
    {
      title: "Block a maestro task",
      description:
        "Mark a claimed/doing/verifying task as blocked with a mandatory reason (claimed|doing|verifying -> blocked). v2 block is a state transition on the task itself, not a cross-task edge graph. Error codes: TASK_NOT_FOUND, TASK_BLOCK_FAILED.",
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
        const task = await taskBlock(
          {
            taskStore: services.v2.taskStore,
            evidenceStore: services.v2.evidenceStore,
            missionStore: services.v2.missionStore,
            observabilityStore: services.v2.observabilityStore,
            handoffEmitter: services.v2.handoffEmitter,
          },
          { id: args.id, reason: args.reason },
        );
        await refreshNowMdFromServices(services.v2);
        return toCallToolResult(ok({ task }));
      } catch (err) {
        return toCallToolResult(fromMaestroError(err, "TASK_BLOCK_FAILED"));
      }
    },
  );
}
