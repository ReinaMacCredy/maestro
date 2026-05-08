import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import {
  listEvidence,
  recordEvidence,
  type EvidenceListFilter,
} from "@/features/evidence/index.js";
import { fromMaestroError, ok, toCallToolResult } from "../errors.js";
import { paginate } from "../pagination.js";
import { EvidenceListInput, EvidenceRecordInput } from "../schemas/inputs.js";
import { EvidenceListOutput, EvidenceRecordOutput } from "../schemas/outputs.js";
import type { RegisterDeps } from "./types.js";

export function registerEvidenceTools(server: McpServer, deps: RegisterDeps): void {
  server.registerTool(
    "maestro_evidence_list",
    {
      title: "List evidence rows",
      description:
        "List evidence rows for a task with optional kind/witness level filters. Paginated. Read-only.",
      inputSchema: EvidenceListInput,
      outputSchema: EvidenceListOutput,
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
        const filter: EvidenceListFilter = {
          task_id: args.taskId,
          ...(args.kind !== undefined ? { kind: args.kind } : {}),
        };
        let rows = await listEvidence(services.evidenceStore, filter);
        if (args.witnessLevel !== undefined) {
          rows = rows.filter((r) => r.witness_level === args.witnessLevel);
        }
        const page = paginate(rows, args.limit, args.offset);
        return toCallToolResult(ok(page));
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
        "Append an evidence row. Provide exactly one of: `command`+`exitCode` for a command run, or `note` for a manual note. Passing both, neither, or `command` without `exitCode` is rejected at the schema layer. Default witness level is agent-claimed-locally. Each call appends a new row.",
      inputSchema: EvidenceRecordInput,
      outputSchema: EvidenceRecordOutput,
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
        const { sessionId } = deps;
        const witnessLevel = args.witnessLevel ?? "agent-claimed-locally";
        if (args.command !== undefined) {
          const row = await recordEvidence<"command">(services.evidenceStore, {
            task_id: args.taskId,
            session_id: sessionId,
            kind: "command",
            payload: { command: args.command, exit: args.exitCode ?? 0 },
            witness_level: witnessLevel,
          });
          return toCallToolResult(ok({ evidence: row }));
        }
        if (args.note === undefined) {
          return toCallToolResult(
            fromMaestroError(
              new Error("EvidenceRecordInput refine bypassed: neither command nor note set"),
              "EVIDENCE_RECORD_FAILED",
            ),
          );
        }
        const row = await recordEvidence<"manual-note">(services.evidenceStore, {
          task_id: args.taskId,
          session_id: sessionId,
          kind: "manual-note",
          payload: { note: args.note },
          witness_level: witnessLevel,
        });
        return toCallToolResult(ok({ evidence: row }));
      } catch (err) {
        return toCallToolResult(fromMaestroError(err, "EVIDENCE_RECORD_FAILED"));
      }
    },
  );
}
