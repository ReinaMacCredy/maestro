import type { Command } from "commander";
import { MaestroError } from "@/shared/errors.js";
import { output, resolveJsonFlag } from "@/shared/lib/output.js";
import { getServices, type Services } from "@/services.js";
import { recordEvidence, type RecordEvidenceInput } from "../usecases/record-evidence.usecase.js";
import { listEvidence } from "../usecases/list-evidence.usecase.js";
import { isEvidenceId } from "../domain/evidence-id.js";
import { readText } from "@/shared/lib/fs.js";
import {
  WITNESS_LEVEL_ORDER,
  isWitnessLevel,
  type AIReviewFinding,
  type AIReviewPayload,
  type AIReviewerKind,
  type CommandPayload,
  type CrossTaskConflictPayload,
  type DeployReadinessPayload,
  type EvidenceKind,
  type EvidenceRow,
  type ManualNotePayload,
  type PlanCheckPayload,
  type RollbackExercisedPayload,
  type RuntimeSignalPayload,
  type ThreatModelPayload,
  type WitnessLevel,
} from "../domain/types.js";
import { parseYaml } from "@/shared/lib/yaml.js";
import type { EvidenceListFilter } from "../ports/storage.js";

interface EvidenceCommandDeps {
  readonly getServices: () => Pick<Services, "evidenceStore" | "taskStore" | "sessionDetect" | "specStore">;
  readonly recordEvidence: typeof recordEvidence;
}

const EVIDENCE_KINDS: readonly EvidenceKind[] = ["command", "manual-note", "ai-review", "plan-check", "threat-model"];
const AI_REVIEWER_KINDS: readonly AIReviewerKind[] = ["bug", "security", "architecture"];

export function registerEvidenceCommand(
  program: Command,
  deps: EvidenceCommandDeps = { getServices, recordEvidence },
): void {
  const evidenceCmd = program
    .command("evidence")
    .description("Record and inspect task evidence")
    .option("--json", "Output as JSON");
  registerRecordCommand(evidenceCmd, program, deps);
  registerListCommand(evidenceCmd, program, deps);
  registerShowCommand(evidenceCmd, program, deps);
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
  maestro evidence record --task tsk-aaaaaa --kind ai-review --reviewer security --findings '[{"severity":"error","message":"SQL injection"}]' --confidence 0.9
  maestro evidence record --task tsk-aaaaaa --kind threat-model --threat-model-file ./threat-model.json
`)
    .requiredOption("--task <id>", "Task this evidence belongs to")
    .option("--kind <kind>", `Evidence kind (${EVIDENCE_KINDS.join("|")})`, "command")
    .option("--command <str>", "Command that was run (with --kind command)")
    .option("--exit <int>", "Exit code (with --kind command)", parseNonNegativeInt)
    .option("--log <path>", "Path to a log file")
    .option("--duration <ms>", "Duration in milliseconds", parseNonNegativeInt)
    .option("--criterion <id>", "Criterion id this evidence references")
    .option("--note <text>", "Free-form note (with --kind manual-note)")
    .option("--reviewer <kind>", `Reviewer kind for --kind ai-review (${AI_REVIEWER_KINDS.join("|")})`)
    .option("--findings <json>", "JSON array of findings or path to a JSON/YAML file (with --kind ai-review)")
    .option("--confidence <n>", "Confidence score 0-1 for --kind ai-review (default 0.5)", parseFloat)
    .option("--threat-model-file <path>", "Path to a JSON or YAML threat-model file (with --kind threat-model)")
    .option("--witness <level>", "Override witness level (default: agent-claimed-locally)")
    .option("--session <id>", "Override session id (default: detected session)")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, root) || (parent.opts().json as boolean | undefined) === true;

      const input = await buildRecordInput(services, opts);
      const row = await deps.recordEvidence(services.evidenceStore, input);
      output(isJson, row, (r) => formatEvidenceRow(r, "Evidence recorded"));
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
  readonly reviewer?: string;
  readonly findings?: string;
  readonly confidence?: number;
  readonly threatModelFile?: string;
  readonly witness?: string;
  readonly session?: string;
}

async function buildRecordInput(
  services: Pick<Services, "evidenceStore" | "taskStore" | "sessionDetect" | "specStore">,
  opts: RecordOpts,
): Promise<RecordEvidenceInput> {
  const taskId = opts.task;
  const task = await services.taskStore.get(taskId);
  if (!task) {
    throw new MaestroError(`Task not found: ${taskId}`, [
      "Run `maestro task list` to see available tasks",
    ]);
  }

  // When the task belongs to a Mission that has a Spec with at least one
  // criterion, --criterion is required and must match a known criterion id.
  if (task.missionId) {
    const spec = await services.specStore.read(task.missionId);
    if (spec && spec.acceptance_criteria.length > 0) {
      const ids = spec.acceptance_criteria.map((c) => c.id);
      if (!opts.criterion) {
        throw new MaestroError("--criterion required when task's mission has a Spec", [
          `Available: ${ids.join(", ")}`,
        ]);
      }
      if (!ids.includes(opts.criterion)) {
        throw new MaestroError(`Unknown criterion id: ${opts.criterion}`, [
          `Available: ${ids.join(", ")}`,
        ]);
      }
    }
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
      witness_level: parseWitnessLevel(opts.witness, "agent-claimed-locally"),
    };
  }

  if (kind === "ai-review") {
    if (typeof opts.reviewer !== "string" || !AI_REVIEWER_KINDS.includes(opts.reviewer as AIReviewerKind)) {
      throw new MaestroError(
        `--kind ai-review requires --reviewer (one of: ${AI_REVIEWER_KINDS.join(", ")})`,
        [`maestro evidence record --task ${taskId} --kind ai-review --reviewer security --findings '[...]'`],
      );
    }
    if (typeof opts.findings !== "string" || opts.findings.length === 0) {
      throw new MaestroError("--kind ai-review requires --findings", [
        `maestro evidence record --task ${taskId} --kind ai-review --reviewer ${opts.reviewer} --findings '[{"severity":"info","message":"..."}]'`,
      ]);
    }
    const confidence = opts.confidence ?? 0.5;
    if (!Number.isFinite(confidence) || confidence < 0 || confidence > 1) {
      throw new MaestroError(`--confidence must be between 0 and 1, got: ${confidence}`, [
        "Pass a decimal value between 0 and 1 (e.g. 0.8)",
      ]);
    }
    const findings = await parseFindings(opts.findings, taskId);
    const payload: AIReviewPayload = {
      reviewer: opts.reviewer as AIReviewerKind,
      findings,
      confidence,
      ...(opts.criterion !== undefined ? { criterion_id: opts.criterion } : {}),
    };
    return {
      task_id: taskId,
      ...(sessionId !== undefined ? { session_id: sessionId } : {}),
      kind: "ai-review",
      payload,
      witness_level: parseWitnessLevel(opts.witness, "agent-claimed-locally"),
    };
  }

  if (kind === "threat-model") {
    if (typeof opts.threatModelFile !== "string" || opts.threatModelFile.length === 0) {
      throw new MaestroError("--kind threat-model requires --threat-model-file", [
        `maestro evidence record --task ${taskId} --kind threat-model --threat-model-file ./threat-model.json`,
      ]);
    }
    const payload = await parseThreatModelFile(opts.threatModelFile, taskId, opts.criterion);
    const witnessLevel = parseWitnessLevel(opts.witness, "agent-claimed-locally");
    return {
      task_id: taskId,
      ...(sessionId !== undefined ? { session_id: sessionId } : {}),
      kind: "threat-model",
      payload,
      witness_level: witnessLevel,
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
    witness_level: parseWitnessLevel(opts.witness, "agent-claimed-and-not-reproducible"),
  };
}

function parseKind(value: string | undefined): EvidenceKind {
  const kind = value ?? "command";
  if (!EVIDENCE_KINDS.includes(kind as EvidenceKind)) {
    throw new MaestroError(`Invalid --kind: ${kind}`, [
      `Valid kinds: ${EVIDENCE_KINDS.join(", ")}`,
    ]);
  }
  return kind as EvidenceKind;
}

const VALID_SEVERITIES = new Set(["info", "warn", "error"]);

async function parseFindings(raw: string, taskId: string): Promise<readonly AIReviewFinding[]> {
  let parsed: unknown;
  if (raw.trimStart().startsWith("[") || raw.trimStart().startsWith("{")) {
    try {
      parsed = JSON.parse(raw);
    } catch {
      throw new MaestroError("--findings: invalid JSON", [
        "Pass a JSON array of findings or a path to a JSON/YAML file",
      ]);
    }
  } else {
    let fileContent: string | undefined;
    try {
      fileContent = await readText(raw);
    } catch (err: unknown) {
      const code = (err as NodeJS.ErrnoException).code;
      if (code === "EISDIR") {
        throw new MaestroError(`--findings: path is a directory: ${raw}`, [
          "Pass a path to a JSON or YAML file, not a directory",
        ]);
      }
      const msg = err instanceof Error ? err.message : String(err);
      throw new MaestroError(`--findings: cannot read file: ${raw}`, [msg]);
    }
    if (fileContent === undefined) {
      throw new MaestroError(`--findings: could not read file: ${raw}`, [
        `maestro evidence record --task ${taskId} --kind ai-review --reviewer security --findings ./findings.json`,
      ]);
    }
    try {
      parsed = parseYaml<unknown>(fileContent);
    } catch {
      throw new MaestroError(`--findings: could not parse file as JSON/YAML: ${raw}`, [
        "Ensure the file contains a valid JSON or YAML array of findings",
      ]);
    }
  }

  if (!Array.isArray(parsed)) {
    throw new MaestroError("--findings: expected a JSON array of findings", [
      'Example: \'[{"severity":"info","message":"looks good"}]\'',
    ]);
  }

  const findings: AIReviewFinding[] = [];
  for (let i = 0; i < parsed.length; i++) {
    const item = parsed[i] as Record<string, unknown>;
    if (typeof item !== "object" || item === null || Array.isArray(item)) {
      throw new MaestroError(`--findings[${i}]: each finding must be an object`, []);
    }
    if (!VALID_SEVERITIES.has(item["severity"] as string)) {
      throw new MaestroError(
        `--findings[${i}]: severity must be one of: info, warn, error (got: ${item["severity"]})`,
        [],
      );
    }
    if (typeof item["message"] !== "string" || (item["message"] as string).length === 0) {
      throw new MaestroError(`--findings[${i}]: message must be a non-empty string`, []);
    }
    findings.push({
      severity: item["severity"] as AIReviewFinding["severity"],
      message: item["message"] as string,
      ...(Array.isArray(item["paths"]) ? { paths: item["paths"] as string[] } : {}),
      ...(typeof item["suggestion"] === "string" ? { suggestion: item["suggestion"] } : {}),
    });
  }
  return findings;
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

const VALID_RESIDUAL_RISKS = new Set(["low", "medium", "high"]);

async function parseThreatModelFile(
  filePath: string,
  taskId: string,
  criterion?: string,
): Promise<ThreatModelPayload> {
  let fileContent: string | undefined;
  try {
    fileContent = await readText(filePath);
  } catch (err: unknown) {
    const code = (err as NodeJS.ErrnoException).code;
    if (code === "EISDIR") {
      throw new MaestroError(`--threat-model-file: path is a directory: ${filePath}`, [
        "Pass a path to a JSON or YAML file, not a directory",
      ]);
    }
    const msg = err instanceof Error ? err.message : String(err);
    throw new MaestroError(`--threat-model-file: cannot read file: ${filePath}`, [msg]);
  }
  if (fileContent === undefined) {
    throw new MaestroError(`--threat-model-file: could not read file: ${filePath}`, [
      `maestro evidence record --task ${taskId} --kind threat-model --threat-model-file ./threat-model.json`,
    ]);
  }

  let raw: unknown;
  try {
    raw = parseYaml<unknown>(fileContent);
  } catch {
    throw new MaestroError(`--threat-model-file: could not parse file as JSON/YAML: ${filePath}`, [
      "Ensure the file contains a valid JSON or YAML object matching the ThreatModelPayload schema",
    ]);
  }

  if (typeof raw !== "object" || raw === null || Array.isArray(raw)) {
    throw new MaestroError(`--threat-model-file: expected a top-level object, got ${Array.isArray(raw) ? "array" : typeof raw}`, [
      "The threat-model file must be a JSON or YAML object",
    ]);
  }

  const obj = raw as Record<string, unknown>;

  if (!Array.isArray(obj["assets"]) || !(obj["assets"] as unknown[]).every((v) => typeof v === "string")) {
    throw new MaestroError(`--threat-model-file: field "assets" must be a string array`, [
      "Example: assets: [\"session tokens\", \"password hashes\"]",
    ]);
  }

  if (!Array.isArray(obj["threatCategories"]) || !(obj["threatCategories"] as unknown[]).every((v) => typeof v === "string")) {
    throw new MaestroError(`--threat-model-file: field "threatCategories" must be a string array`, [
      "Example: threatCategories: [\"spoofing\", \"tampering\"]",
    ]);
  }

  if (!Array.isArray(obj["mitigations"])) {
    throw new MaestroError(`--threat-model-file: field "mitigations" must be an array`, [
      "Example: mitigations: [{threat: \"session-fixation\", mitigation: \"rotate token on login\"}]",
    ]);
  }
  for (let i = 0; i < (obj["mitigations"] as unknown[]).length; i++) {
    const m = (obj["mitigations"] as unknown[])[i] as Record<string, unknown>;
    if (typeof m !== "object" || m === null || Array.isArray(m)) {
      throw new MaestroError(`--threat-model-file: mitigations[${i}] must be an object`, []);
    }
    if (typeof m["threat"] !== "string" || (m["threat"] as string).length === 0) {
      throw new MaestroError(`--threat-model-file: mitigations[${i}].threat must be a non-empty string`, []);
    }
    if (typeof m["mitigation"] !== "string" || (m["mitigation"] as string).length === 0) {
      throw new MaestroError(`--threat-model-file: mitigations[${i}].mitigation must be a non-empty string`, []);
    }
  }

  if (!VALID_RESIDUAL_RISKS.has(obj["residualRisk"] as string)) {
    throw new MaestroError(
      `--threat-model-file: field "residualRisk" must be one of: low, medium, high (got: ${obj["residualRisk"]})`,
      [],
    );
  }

  return {
    assets: obj["assets"] as string[],
    threatCategories: obj["threatCategories"] as string[],
    mitigations: (obj["mitigations"] as Array<Record<string, string>>).map((m) => ({
      threat: m["threat"] as string,
      mitigation: m["mitigation"] as string,
    })),
    residualRisk: obj["residualRisk"] as "low" | "medium" | "high",
    ...(criterion !== undefined ? { criterion_id: criterion } : {}),
    source_file: filePath,
  };
}

function parseWitnessLevel(raw: string | undefined, fallback: WitnessLevel): WitnessLevel {
  if (raw === undefined) return fallback;
  if (!isWitnessLevel(raw)) {
    throw new MaestroError(
      `Invalid --witness level: ${raw}`,
      [`Valid levels: ${WITNESS_LEVEL_ORDER.join(", ")}`],
    );
  }
  return raw;
}

function formatEvidenceRow(row: EvidenceRow, label = "Evidence"): string[] {
  const lines = [
    `[ok] ${label}: ${row.id}`,
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
  } else if (row.kind === "ai-review") {
    const payload = row.payload as AIReviewPayload;
    const errorCount = payload.findings.filter((f) => f.severity === "error").length;
    const warnCount = payload.findings.filter((f) => f.severity === "warn").length;
    const infoCount = payload.findings.filter((f) => f.severity === "info").length;
    lines.push(`  Reviewer: ${payload.reviewer}`);
    lines.push(`  Findings: ${payload.findings.length} (errors: ${errorCount}, warns: ${warnCount}, infos: ${infoCount})`);
    lines.push(`  Confidence: ${payload.confidence}`);
    if (payload.criterion_id !== undefined) lines.push(`  Criterion: ${payload.criterion_id}`);
  } else if (row.kind === "plan-check") {
    const payload = row.payload as PlanCheckPayload;
    lines.push(`  SHA: ${payload.planFileSha}`);
    lines.push(`  Errors: ${payload.errorCount}  Warnings: ${payload.warnCount}`);
    for (const f of payload.findings) {
      lines.push(`  [${f.severity}] ${f.check}: ${f.message}`);
    }
  } else if (row.kind === "threat-model") {
    const payload = row.payload as ThreatModelPayload;
    lines.push(`  Residual Risk: ${payload.residualRisk}`);
    lines.push(`  Assets: ${payload.assets.length}`);
    lines.push(`  Threat Categories: ${payload.threatCategories.length}`);
    lines.push(`  Mitigations: ${payload.mitigations.length}`);
    if (payload.source_file !== undefined) lines.push(`  Source: ${payload.source_file}`);
    if (payload.criterion_id !== undefined) lines.push(`  Criterion: ${payload.criterion_id}`);
  } else if (row.kind === "manual-note") {
    const payload = row.payload as ManualNotePayload;
    lines.push(`  Note: ${payload.note}`);
    if (payload.criterion_id !== undefined) lines.push(`  Criterion: ${payload.criterion_id}`);
  } else if (row.kind === "deploy-readiness") {
    const payload = row.payload as DeployReadinessPayload;
    const { feature_flag, canary_plan, rollback, owner } = payload.checks;
    const checkSummary = [
      `feature_flag: ${feature_flag.ok ? "ok" : "fail"}`,
      `canary_plan: ${canary_plan.ok ? "ok" : "fail"}`,
      `rollback: ${rollback.ok ? "ok" : "fail"}`,
      `owner: ${owner.ok ? "ok" : "fail"}`,
    ].join(", ");
    lines.push(`  Task: ${payload.task_id}`);
    lines.push(`  Gate: ${payload.gate}`);
    lines.push(`  Checks: ${checkSummary}`);
  } else if (row.kind === "runtime-signal") {
    const payload = row.payload as RuntimeSignalPayload;
    lines.push(`  Signal: ${payload.signal_name}`);
    lines.push(`  Provider: ${payload.provider}`);
    lines.push(`  Value: ${payload.value} ${payload.operator} ${payload.threshold} => ${payload.pass ? "pass" : "fail"}`);
    lines.push(`  Sampled: ${payload.sampled_at}`);
    if (payload.note !== undefined) lines.push(`  Note: ${payload.note}`);
  } else if (row.kind === "rollback-exercised") {
    const payload = row.payload as RollbackExercisedPayload;
    lines.push(`  Command: ${payload.command}`);
    lines.push(`  Exit: ${payload.exit}`);
  } else if (row.kind === "cross-task-conflict") {
    const payload = row.payload as CrossTaskConflictPayload;
    lines.push(`  This PR: ${payload.thisPr}`);
    lines.push(`  Conflicting PRs: ${payload.conflictingPrs.join(", ")}`);
    const paths = payload.overlappingPaths.slice(0, 5);
    const truncated = payload.overlappingPaths.length > 5
      ? [...paths, `... (${payload.overlappingPaths.length - 5} more)`]
      : paths;
    lines.push(`  Overlapping Paths (${payload.overlappingPaths.length}): ${truncated.join(", ")}`);
  }
  return lines;
}

function formatEvidenceList(rows: readonly EvidenceRow[]): string[] {
  if (rows.length === 0) return ["No evidence found."];
  return rows.map(
    (row) => `${row.created_at}  ${row.id}  ${row.kind}  ${row.task_id}  ${row.witness_level}`,
  );
}

function registerListCommand(parent: Command, root: Command, deps: EvidenceCommandDeps): void {
  parent
    .command("list")
    .description("List evidence rows")
    .option("--task <id>", "Filter by task id")
    .option("--session <id>", "Filter by session id")
    .option("--kind <kind>", `Filter by kind (${EVIDENCE_KINDS.join("|")})`)
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, root) || (parent.opts().json as boolean | undefined) === true;

      const filter: EvidenceListFilter = {
        ...(opts.task !== undefined ? { task_id: opts.task as string } : {}),
        ...(opts.session !== undefined ? { session_id: opts.session as string } : {}),
        ...(opts.kind !== undefined ? { kind: parseKind(opts.kind as string) } : {}),
      };

      const rows = await listEvidence(services.evidenceStore, filter);
      output(isJson, rows, formatEvidenceList);
    });
}

function registerShowCommand(parent: Command, root: Command, deps: EvidenceCommandDeps): void {
  parent
    .command("show <id>")
    .description("Show one evidence row by id")
    .option("--json", "Output as JSON")
    .action(async (id: string, opts) => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, root) || (parent.opts().json as boolean | undefined) === true;

      if (!isEvidenceId(id)) {
        throw new MaestroError(`Invalid evidence id: ${id}`, [
          "Evidence ids look like 'evd-xxxxxx'",
        ]);
      }

      const row = await services.evidenceStore.read(id);
      if (row === undefined) {
        throw new MaestroError(`Evidence not found: ${id}`, [
          "Run `maestro evidence list --task <id>` to see available evidence",
        ]);
      }

      output(isJson, row, (r) => formatEvidenceRow(r));
    });
}
