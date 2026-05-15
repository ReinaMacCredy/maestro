import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { setupCheck, migrateV2 } from "@/v2/service/index.js";
import { fromMaestroError, ok, toCallToolResult, type CallToolResult } from "../errors.js";
import { SetupCheckInput, SetupMigrateV2Input } from "../schemas/inputs.js";
import type { RegisterDeps } from "./types.js";

export function registerSetupTools(server: McpServer, deps: RegisterDeps): void {
  server.registerTool(
    "maestro_setup_check",
    {
      title: "Check v2 project setup",
      description:
        "Audit whether the v2 directory tree (.maestro/tasks, .maestro/plans, .maestro/evidence, .maestro/runs, docs/principles) is present and valid. Returns ok (bool) and a list of entries with status ok|warn|missing. Read-only.",
      inputSchema: SetupCheckInput,
      annotations: {
        readOnlyHint: true,
        destructiveHint: false,
        idempotentHint: true,
        openWorldHint: false,
      },
    },
    async (_args): Promise<CallToolResult> => {
      try {
        const services = deps.getServices();
        const report = await setupCheck({ repoRoot: services.projectRoot });
        return toCallToolResult(ok(report));
      } catch (err) {
        return toCallToolResult(fromMaestroError(err, "SETUP_CHECK_FAILED"));
      }
    },
  );

  server.registerTool(
    "maestro_setup_migrate_v2",
    {
      title: "Migrate project state to v2",
      description:
        "Run the 11-step v1→v2 migration: preflight, backup, bootstrap dirs, migrate corrections/tasks/plans/evidence/policies, seed principles, write flag, verify. Pass dry_run=true to preview steps without side effects. Pass force=true to re-run even if .migrated-v2.json is present. Error codes: SETUP_MIGRATE_FAILED.",
      inputSchema: SetupMigrateV2Input,
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
        const result = await migrateV2(
          { repoRoot: services.projectRoot },
          { dryRun: args.dry_run, force: args.force },
        );
        return toCallToolResult(ok(result));
      } catch (err) {
        return toCallToolResult(fromMaestroError(err, "SETUP_MIGRATE_FAILED"));
      }
    },
  );
}
