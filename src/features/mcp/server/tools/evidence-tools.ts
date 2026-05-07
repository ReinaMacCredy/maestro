import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import {
  listEvidence,
  recordEvidence,
  type EvidenceKind,
  type EvidenceListFilter,
  type WitnessLevel,
} from "@/features/evidence/index.js";
import type { Services } from "@/services.js";
import { fromMaestroError, ok, toCallToolResult } from "../errors.js";
import { paginate } from "../pagination.js";
import { detectMcpSessionId } from "../session.js";
import { EvidenceListInput, EvidenceRecordInput } from "../schemas/inputs.js";

interface RegisterDeps {
  readonly getServices: () => Services;
}

export function registerEvidenceTools(server: McpServer, deps: RegisterDeps): void {
  server.registerTool(
    "maestro_evidence_list",
    {
      title: "List evidence rows",
      description:
        "List evidence rows for a task with optional kind/witness level filters. Paginated. Read-only.",
      inputSchema: EvidenceListInput,
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
          ...(args.kind !== undefined ? { kind: args.kind as EvidenceKind } : {}),
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
        "Append an evidence row. Either pass `command`+`exitCode` for a command run or `note` for a manual note. Default witness level is agent-claimed-locally. Each call appends a new row.",
      inputSchema: EvidenceRecordInput,
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
        const sessionId = detectMcpSessionId();
        const witnessLevel: WitnessLevel =
          (args.witnessLevel as WitnessLevel | undefined) ?? "agent-claimed-locally";
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
        const note = args.note ?? "";
        const row = await recordEvidence<"manual-note">(services.evidenceStore, {
          task_id: args.taskId,
          session_id: sessionId,
          kind: "manual-note",
          payload: { note },
          witness_level: witnessLevel,
        });
        return toCallToolResult(ok({ evidence: row }));
      } catch (err) {
        return toCallToolResult(fromMaestroError(err, "EVIDENCE_RECORD_FAILED"));
      }
    },
  );
}
