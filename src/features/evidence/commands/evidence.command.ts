import type { Command } from "commander";
import { MaestroError } from "@/shared/errors.js";
import { output, resolveJsonFlag } from "@/shared/lib/output.js";
import { getServices, type Services } from "@/services.js";
import { recordEvidence, type RecordEvidenceInput } from "../usecases/record-evidence.usecase.js";
import type {
  CommandPayload,
  EvidenceKind,
  EvidenceRow,
  ManualNotePayload,
  WitnessLevel,
} from "../domain/types.js";

interface EvidenceCommandDeps {
  readonly getServices: () => Pick<Services, "evidenceStore" | "taskStore" | "sessionDetect">;
  readonly recordEvidence: typeof recordEvidence;
}

const EVIDENCE_KINDS: readonly EvidenceKind[] = ["command", "manual-note"];

export function registerEvidenceCommand(
  program: Command,
  deps: EvidenceCommandDeps = { getServices, recordEvidence },
): void {
  const evidenceCmd = program
    .command("evidence")
    .description("Record and inspect task evidence")
    .option("--json", "Output as JSON");
  registerRecordCommand(evidenceCmd, program, deps);
}

function registerRecordCommand(parent: Command, root: Command, deps: EvidenceCommandDeps): void {
  parent
    .command("record")
    .description("Record a piece of evidence for a task")
    .addHelpText("after", `
Examples:
  maestro evidence record --task tsk-aaaaaa --command "bun test" --exit 0
  maestro evidence record --task tsk-aaaaaa --command "bun run build" --exit 0 --duration 12345 --log ./build.log
  maestro evidence record --task tsk-aaaaaa --kind manual-note --note "Verified UI on staging"
`)
    .requiredOption("--task <id>", "Task this evidence belongs to")
    .option("--kind <kind>", `Evidence kind (${EVIDENCE_KINDS.join("|")})`, "command")
    .option("--command <str>", "Command that was run (with --kind command)")
    .option("--exit <int>", "Exit code (with --kind command)", parseNonNegativeInt)
    .option("--log <path>", "Path to a log file")
    .option("--duration <ms>", "Duration in milliseconds", parseNonNegativeInt)
    .option("--criterion <id>", "Criterion id this evidence references")
    .option("--note <text>", "Free-form note (with --kind manual-note)")
    .option("--session <id>", "Override session id (default: detected session)")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, root) || (parent.opts().json as boolean | undefined) === true;

      const input = await buildRecordInput(services, opts);
      const row = await deps.recordEvidence(services.evidenceStore, input);
      output(isJson, row, formatRecorded);
    });
}

interface RecordOpts {
  readonly task: string;
  readonly kind?: string;
  readonly command?: string;
  readonly exit?: number;
  readonly log?: string;
  readonly duration?: number;
  readonly criterion?: string;
  readonly note?: string;
  readonly session?: string;
}

async function buildRecordInput(
  services: Pick<Services, "evidenceStore" | "taskStore" | "sessionDetect">,
  opts: RecordOpts,
): Promise<RecordEvidenceInput> {
  const taskId = opts.task;
  const task = await services.taskStore.get(taskId);
  if (!task) {
    throw new MaestroError(`Task not found: ${taskId}`, [
      "Run `maestro task list` to see available tasks",
    ]);
  }

  const kind = parseKind(opts.kind);
  const sessionId = opts.session
    ?? (await services.sessionDetect.detect(process.cwd()))?.sessionId;

  if (kind === "command") {
    if (typeof opts.command !== "string" || opts.command.length === 0
      || typeof opts.exit !== "number") {
      throw new MaestroError("--kind command requires --command and --exit", [
        `maestro evidence record --task ${taskId} --command "bun test" --exit 0`,
      ]);
    }
    const payload: CommandPayload = {
      command: opts.command,
      exit: opts.exit,
      ...(opts.log !== undefined ? { log_path: opts.log } : {}),
      ...(opts.duration !== undefined ? { duration_ms: opts.duration } : {}),
      ...(opts.criterion !== undefined ? { criterion_id: opts.criterion } : {}),
    };
    return {
      task_id: taskId,
      ...(sessionId !== undefined ? { session_id: sessionId } : {}),
      kind: "command",
      payload,
      witness_level: "agent-claimed-locally" satisfies WitnessLevel,
    };
  }

  if (typeof opts.note !== "string" || opts.note.length === 0) {
    throw new MaestroError("--kind manual-note requires --note", [
      `maestro evidence record --task ${taskId} --kind manual-note --note "verified manually"`,
    ]);
  }
  const payload: ManualNotePayload = {
    note: opts.note,
    ...(opts.criterion !== undefined ? { criterion_id: opts.criterion } : {}),
  };
  return {
    task_id: taskId,
    ...(sessionId !== undefined ? { session_id: sessionId } : {}),
    kind: "manual-note",
    payload,
    witness_level: "agent-claimed-and-not-reproducible" satisfies WitnessLevel,
  };
}

function parseKind(value: string | undefined): EvidenceKind {
  const kind = value ?? "command";
  if (kind !== "command" && kind !== "manual-note") {
    throw new MaestroError(`Invalid --kind: ${kind}`, [
      `Valid kinds: ${EVIDENCE_KINDS.join(", ")}`,
    ]);
  }
  return kind;
}

function parseNonNegativeInt(raw: string): number {
  const n = Number(raw);
  if (!Number.isFinite(n) || !Number.isInteger(n) || n < 0) {
    throw new MaestroError(`Invalid integer: ${raw}`, [
      "Pass a non-negative integer (0 or greater)",
    ]);
  }
  return n;
}

function formatRecorded(row: EvidenceRow): string[] {
  const lines = [
    `[ok] Evidence recorded: ${row.id}`,
    `  Task: ${row.task_id}`,
    `  Kind: ${row.kind}`,
    `  Witness: ${row.witness_level}`,
    `  Created: ${row.created_at}`,
  ];
  if (row.session_id) lines.push(`  Session: ${row.session_id}`);
  if (row.kind === "command") {
    const payload = row.payload as CommandPayload;
    lines.push(`  Command: ${payload.command}`, `  Exit: ${payload.exit}`);
    if (payload.duration_ms !== undefined) lines.push(`  Duration: ${payload.duration_ms}ms`);
    if (payload.log_path !== undefined) lines.push(`  Log: ${payload.log_path}`);
    if (payload.criterion_id !== undefined) lines.push(`  Criterion: ${payload.criterion_id}`);
  } else {
    const payload = row.payload as ManualNotePayload;
    lines.push(`  Note: ${payload.note}`);
    if (payload.criterion_id !== undefined) lines.push(`  Criterion: ${payload.criterion_id}`);
  }
  return lines;
}
