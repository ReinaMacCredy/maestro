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
} from "@/shared/domain/task/index.js";
import { summarizeTask } from "@/shared/lib/projection.js";
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

export function registerTaskTools(server: McpServer, deps: RegisterDeps): void {
  server.registerTool(
    "maestro_task_list",
    {
      title: "List maestro tasks",
      description:
        "List tasks. Filters: mission_id, state (draft|claimed|doing|verifying|blocked|ready|shipped|abandoned). Paginated (default limit 20, max 100). view='summary' (default) returns id+slug+title+state+mission_id+assignee+blocked_by_count; view='full' returns the full Task. Sorted by created_at asc. Read-only.",
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
          tasks = await services.taskStore.listByMissionId(args.mission_id);
        } else if (args.state !== undefined) {
          tasks = await services.taskStore.listByState(args.state);
        } else {
          tasks = await services.taskStore.list();
        }
        // Apply state filter on top of mission_id (and vice versa) so both filters
        // compose when supplied together, matching the tool description.
        if (args.mission_id !== undefined && args.state !== undefined) {
          tasks = tasks.filter((t) => t.state === args.state);
        }
        const page = paginate(tasks, args.limit, args.offset);
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
    "maestro_task_from_spec",
    {
      title: "Create a task from a spec file",
      description:
        "Create a task in draft state from a product-spec markdown file. The spec must be an on-disk markdown file with a valid slug in its YAML frontmatter. Each call creates a new task; use the returned id for subsequent claim/ship. Error codes: TASK_CREATE_FAILED.",
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
            specStore: services.specStore,
            taskStore: services.taskStore,
            evidenceStore: services.evidenceStore,
            observabilityStore: services.observabilityStore,
          },
          args.spec_path,
        );
        await refreshNowMdFromServices(services);
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
            taskStore: services.taskStore,
            evidenceStore: services.evidenceStore,
            missionStore: services.missionStore,
            observabilityStore: services.observabilityStore,
            worktreeStore: services.worktreeStore,
            handoffEmitter: services.handoffEmitter,
            contractStore: new FsContractStoreAdapter(services.projectRoot),
            contractVersionStore: new FsContractVersionStoreAdapter(services.projectRoot),
            repoRoot: services.projectRoot,
          },
          { id: args.id, agentId },
        );
        await refreshNowMdFromServices(services);
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
            taskStore: services.taskStore,
            evidenceStore: services.evidenceStore,
            missionStore: services.missionStore,
            observabilityStore: services.observabilityStore,
          },
          { id: args.id, pr_url: args.pr_url },
        );
        await refreshNowMdFromServices(services);
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
        "Mark a claimed/doing/verifying task as blocked with a mandatory reason (claimed|doing|verifying -> blocked). Block is a state transition on the task itself, not a cross-task edge graph. Error codes: TASK_NOT_FOUND, TASK_BLOCK_FAILED.",
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
            taskStore: services.taskStore,
            evidenceStore: services.evidenceStore,
            missionStore: services.missionStore,
            observabilityStore: services.observabilityStore,
            handoffEmitter: services.handoffEmitter,
          },
          { id: args.id, reason: args.reason },
        );
        await refreshNowMdFromServices(services);
        return toCallToolResult(ok({ task }));
      } catch (err) {
        return toCallToolResult(fromMaestroError(err, "TASK_BLOCK_FAILED"));
      }
    },
  );
}
