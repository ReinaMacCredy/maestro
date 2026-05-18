import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import {
  missionCancel,
  missionDecompose,
  missionFromSpec,
  missionNew,
  missionShow,
  slugifyTitle,
} from "@/service/index.js";
import { refreshNowMdFromServices } from "@/service/refresh-now-md.js";
import { fail, fromMaestroError, ok, toCallToolResult, type CallToolResult } from "../errors.js";
import {
  MissionCancelInput,
  MissionDecomposeInput,
  MissionFromSpecInput,
  MissionNewInput,
  MissionNewShape,
  MissionShowInput,
} from "../schemas/inputs.js";
import type { RegisterDeps } from "./types.js";

export function registerMissionTools(server: McpServer, deps: RegisterDeps): void {
  server.registerTool(
    "maestro_mission_new",
    {
      title: "Create a maestro mission",
      description:
        "Create a mission. mode='bare' (default) creates an intake mission with no tasks; 'from-spec' requires from_spec (heavy-mode spec path) and lands at 'approved'; 'from-file' requires from_file (JSON task batch) and lands at 'planned'; 'template' requires template (built-in or user template name) and lands at 'planned'. slug defaults to slugified title. Error codes: MISSION_CREATE_FAILED.",
      inputSchema: MissionNewShape,
      annotations: {
        readOnlyHint: false,
        destructiveHint: false,
        idempotentHint: false,
        openWorldHint: false,
      },
    },
    async (rawArgs): Promise<CallToolResult> => {
      const parsed = MissionNewInput.safeParse(rawArgs);
      if (!parsed.success) {
        const issue = parsed.error.issues[0];
        return toCallToolResult(
          fail("INVALID_ARG", issue?.message ?? "Invalid mission_new input", {
            arg: issue?.path[0]?.toString(),
            hints: [
              "from_spec/from_file/template must be supplied when mode is set to a non-bare value",
            ],
          }),
        );
      }
      try {
        const args = parsed.data;
        const services = deps.getServices();
        const mode = args.mode ?? "bare";
        const slug = args.slug ?? slugifyTitle(args.title);
        const result = await missionNew(
          {
            repoRoot: services.projectRoot,
            missionStore: services.missionStore,
            taskStore: services.taskStore,
            evidenceStore: services.evidenceStore,
          },
          {
            title: args.title,
            slug,
            mode,
            fromSpec: args.from_spec,
            fromFile: args.from_file,
            template: args.template,
          },
        );
        if (result.tasks.length > 0) {
          await refreshNowMdFromServices(services);
        }
        return toCallToolResult(ok({ mission: result.mission, tasks: result.tasks }));
      } catch (err) {
        return toCallToolResult(fromMaestroError(err, "MISSION_CREATE_FAILED"));
      }
    },
  );

  server.registerTool(
    "maestro_mission_cancel",
    {
      title: "Cancel a maestro mission",
      description:
        "Cancel an active mission and cascade-abandon every non-terminal child task. Idempotent on already-cancelled missions. Cascade failures land in cascadeErrors and the mission still cancels; the structured result is non-error in that case (callers should inspect cascadeErrors and re-run `task abandon` for stragglers). Already-completed / already-failed missions return MISSION_CANCEL_FAILED. Error codes: MISSION_NOT_FOUND, MISSION_CANCEL_FAILED.",
      inputSchema: MissionCancelInput,
      annotations: {
        readOnlyHint: false,
        destructiveHint: true,
        idempotentHint: true,
        openWorldHint: false,
      },
    },
    async (args): Promise<CallToolResult> => {
      try {
        const services = deps.getServices();
        const result = await missionCancel(
          {
            missionStore: services.missionStore,
            taskStore: services.taskStore,
            evidenceStore: services.evidenceStore,
          },
          { mission_id: args.mission_id, reason: args.reason },
        );
        return toCallToolResult(
          ok({
            mission: result.mission,
            cancelled_task_ids: result.cancelledTaskIds,
            cascade_errors: result.cascadeErrors,
            already_cancelled: result.alreadyCancelled,
          }),
        );
      } catch (err) {
        return toCallToolResult(fromMaestroError(err, "MISSION_CANCEL_FAILED"));
      }
    },
  );

  server.registerTool(
    "maestro_mission_decompose",
    {
      title: "Decompose a mission into child tasks",
      description:
        "Decompose an 'intake' or 'approved' mission into one-or-more child tasks, transitioning the mission to 'planned'. Requires the mission to currently have zero tasks (use `task from-spec` to add more later). Each batch entry needs title + slug; spec_path is optional. Error codes: MISSION_NOT_FOUND, MISSION_DECOMPOSE_FAILED.",
      inputSchema: MissionDecomposeInput,
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
        const result = await missionDecompose(
          {
            missionStore: services.missionStore,
            taskStore: services.taskStore,
            evidenceStore: services.evidenceStore,
            observabilityStore: services.observabilityStore,
          },
          { mission_id: args.mission_id, tasks: args.tasks },
        );
        await refreshNowMdFromServices(services);
        return toCallToolResult(ok({ mission: result.mission, tasks: result.tasks }));
      } catch (err) {
        return toCallToolResult(fromMaestroError(err, "MISSION_DECOMPOSE_FAILED"));
      }
    },
  );

  server.registerTool(
    "maestro_mission_show",
    {
      title: "Show a maestro mission",
      description:
        "Fetch a mission and its child tasks (state, slug, title, spec_path). Returns code MISSION_NOT_FOUND when the mission does not exist. Read-only.",
      inputSchema: MissionShowInput,
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
        const result = await missionShow(
          {
            missionStore: services.missionStore,
            taskStore: services.taskStore,
          },
          args.mission_id,
        );
        return toCallToolResult(ok({ mission: result.mission, tasks: result.tasks }));
      } catch (err) {
        return toCallToolResult(fromMaestroError(err, "MISSION_SHOW_FAILED"));
      }
    },
  );

  server.registerTool(
    "maestro_mission_from_spec",
    {
      title: "Create a mission from a heavy-mode spec",
      description:
        "Create a mission in 'approved' state from a heavy-mode product-spec markdown file. Light-mode specs go through maestro_task_from_spec instead. Error codes: MISSION_CREATE_FAILED.",
      inputSchema: MissionFromSpecInput,
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
        const created = await missionFromSpec(
          {
            repoRoot: services.projectRoot,
            missionStore: services.missionStore,
            evidenceStore: services.evidenceStore,
          },
          args.spec_path,
        );
        return toCallToolResult(ok({ mission: created }));
      } catch (err) {
        return toCallToolResult(fromMaestroError(err, "MISSION_CREATE_FAILED"));
      }
    },
  );

}
