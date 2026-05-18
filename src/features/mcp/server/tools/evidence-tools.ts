import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import {
  listEvidence,
  recordEvidence,
  type EvidenceListFilter,
} from "@/features/evidence/index.js";
import { summarizeEvidence } from "@/shared/lib/projection.js";
import { fail, fromMaestroError, ok, toCallToolResult, type CallToolResult } from "../errors.js";
import { paginate } from "../pagination.js";
import { EvidenceListInput, EvidenceRecordInput, EvidenceRecordShape } from "../schemas/inputs.js";
import type { RegisterDeps } from "./types.js";

export function registerEvidenceTools(server: McpServer, deps: RegisterDeps): void {
  server.registerTool(
    "maestro_evidence_list",
    {
      title: "List evidence rows",
      description:
        "List evidence rows for a task. Filters: kind, witnessLevel. Paginated (default limit 20) — `limit` applies per stream: user-recorded rows return under `items[]`, system-generated rows (transition / lint-violation) return under `system_items[]` when present. Combine them client-side if you need a single view. view='summary' (default) returns id+task_id+kind+witness_level+created_at; view='full' includes the typed payload. Read-only.",
      inputSchema: EvidenceListInput,
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
        const filter: EvidenceListFilter = {
          task_id: args.taskId,
          ...(args.kind !== undefined ? { kind: args.kind } : {}),
        };
        let rows = await listEvidence(services.legacyEvidenceStore, filter);
        if (args.witnessLevel !== undefined) {
          rows = rows.filter((r) => r.witness_level === args.witnessLevel);
        }
        // Also union system-generated rows (transition / lint-violation) so a system-only task is not invisible. witnessLevel filter does not apply to those rows. They ride on a sibling `system_items` key so existing `items[]` consumers keep working.
        const systemRows = services.evidenceStore !== undefined
          ? await services.evidenceStore.list({ task_id: args.taskId })
          : [];
        const page = paginate(rows, args.limit, args.offset);
        const systemPage = paginate(systemRows, args.limit, args.offset);
        const projectedItems = args.view === "full" ? page.items : page.items.map(summarizeEvidence);
        return toCallToolResult(
          ok({
            ...page,
            items: projectedItems,
            ...(systemRows.length > 0 ? { system_items: systemPage.items } : {}),
          }),
        );
      } catch (err) {
        return toCallToolResult(fromMaestroError(err, "EVIDENCE_LIST_FAILED"));
      }
    },
  );

  server.registerTool(
    "maestro_evidence_record",
    {
      title: "Record evidence for a task",
      description:
        "Append an evidence row. Provide exactly one of: `command`+`exitCode` for a command run, or `note` for a manual note. Passing both, neither, or `command` without `exitCode` returns INVALID_ARG. Default witness level is agent-claimed-locally.",
      inputSchema: EvidenceRecordShape,
      annotations: {
        readOnlyHint: false,
        destructiveHint: false,
        idempotentHint: false,
        openWorldHint: false,
      },
    },
    async (rawArgs): Promise<CallToolResult> => {
      // SDK already validated against EvidenceRecordShape; re-validate via
      // EvidenceRecordInput to apply the cross-field refines that ZodEffects
      // hides from tools/list introspection.
      const parsed = EvidenceRecordInput.safeParse(rawArgs);
      if (!parsed.success) {
        const issue = parsed.error.issues[0];
        return toCallToolResult(
          fail("INVALID_ARG", issue?.message ?? "Invalid evidence_record input", {
            arg: issue?.path[0]?.toString(),
            hints: [
              "Provide command (with exitCode) for a run, or note for a manual entry",
            ],
          }),
        );
      }
      try {
        const args = parsed.data;
        const services = deps.getServices();
        const { sessionId } = deps;
        const wLevel = args.witnessLevel ?? "agent-claimed-locally";
        if (args.command !== undefined) {
          const row = await recordEvidence<"command">(services.legacyEvidenceStore, {
            task_id: args.taskId,
            session_id: sessionId,
            kind: "command",
            payload: { command: args.command, exit: args.exitCode! },
            witness_level: wLevel,
          });
          return toCallToolResult(ok({ evidence: row }));
        }
        const row = await recordEvidence<"manual-note">(services.legacyEvidenceStore, {
          task_id: args.taskId,
          session_id: sessionId,
          kind: "manual-note",
          payload: { note: args.note! },
          witness_level: wLevel,
        });
        return toCallToolResult(ok({ evidence: row }));
      } catch (err) {
        return toCallToolResult(fromMaestroError(err, "EVIDENCE_RECORD_FAILED"));
      }
    },
  );
}
