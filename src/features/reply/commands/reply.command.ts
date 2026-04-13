/**
 * `maestro reply <feature-id>` CLI. Agent-friendly (flags-only; no prompts).
 *
 * Usage:
 *   maestro reply write f-42 --mission 2026-04-13-001 --outcome completed --note "tests pass"
 *   maestro reply write f-42 --mission 2026-04-13-001 --outcome completed --report-file ./report.json
 *   maestro reply list
 */
import type { Command } from "commander";
import { readFile } from "node:fs/promises";
import { getServices } from "@/services.js";
import { output, resolveJsonFlag } from "@/shared/lib/output.js";
import { MaestroError } from "@/shared/errors.js";
import { writeWorkerReply } from "../usecases/write-reply.usecase.js";
import type { ReplyOutcome, WorkerReply } from "../domain/reply-types.js";
import { REPLY_OUTCOMES } from "../domain/reply-types.js";
import type { WorkerReport } from "@/features/mission/index.js";
import { WorkerReportSchema } from "@/features/mission/index.js";

export function registerReplyCommand(program: Command): void {
  const replyCmd = program
    .command("reply")
    .description("Record a worker reply for a feature (outcome + optional report)")
    .option("--json", "Output as JSON");

  replyCmd
    .command("write <featureId>")
    .description("Write a reply for the given feature id")
    .requiredOption("--mission <id>", "Mission id for the reply (YYYY-MM-DD-NNN)")
    .option("--outcome <outcome>", `Outcome (${REPLY_OUTCOMES.join("|")})`)
    .option("--note <text>", "Free-form notes")
    .option("--report-file <path>", "Path to a JSON file containing a WorkerReport")
    .option("--source <tag>", "Free-form origin marker, e.g. 'cli' or 'agent:claude'")
    .option("--agent", "Mark this reply as agent-authored (default is human)")
    .option("--json", "Output as JSON")
    .action(async (featureId: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const outcome = parseOutcome(opts.outcome);
      const report = await loadReport(opts.reportFile);

      const reply = await writeWorkerReply(services.replyStore, {
        missionId: opts.mission,
        featureId,
        outcome,
        notes: typeof opts.note === "string" ? opts.note : undefined,
        report,
        writtenBy: opts.agent ? "agent" : "human",
        source: typeof opts.source === "string" ? opts.source : undefined,
      });

      output(isJson, reply, (r) => [
        `[ok] Reply recorded: ${r.missionId}/${r.featureId}`,
        `  Outcome: ${r.outcome}`,
        `  Written: ${r.writtenAt} (by ${r.writtenBy})`,
        ...(r.notes ? [`  Notes: ${r.notes}`] : []),
      ]);
    });

  // Support the default-subcommand shape: `maestro reply <featureId> --outcome ...`
  // commander doesn't auto-forward, so we also register the positional at the
  // parent level for ergonomic use.
  replyCmd
    .command("list")
    .description("List replies on disk (newest writtenAt first)")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const replies = [...(await services.replyStore.list())].reverse();
      output(isJson, replies, formatReplyList);
    });
}

function parseOutcome(raw: unknown): ReplyOutcome {
  if (typeof raw !== "string" || !(REPLY_OUTCOMES as readonly string[]).includes(raw)) {
    throw new MaestroError(`--outcome is required (${REPLY_OUTCOMES.join("|")})`, [
      "Example: maestro reply write f-42 --mission 2026-04-13-001 --outcome completed --note 'tests pass'",
    ]);
  }
  return raw as ReplyOutcome;
}

async function loadReport(reportFile: unknown): Promise<WorkerReport | undefined> {
  if (typeof reportFile !== "string" || reportFile.length === 0) return undefined;
  let raw: string;
  try {
    raw = await readFile(reportFile, "utf8");
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new MaestroError(`Cannot read --report-file: ${message}`, [
      `Check the path exists: ${reportFile}`,
    ]);
  }
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new MaestroError(`--report-file is not valid JSON: ${message}`, [
      "Provide a JSON file matching the WorkerReport schema",
    ]);
  }
  const result = WorkerReportSchema.safeParse(parsed);
  if (!result.success) {
    const first = result.error.issues[0];
    const path = first?.path.join(".") || "<root>";
    throw new MaestroError(`--report-file does not match WorkerReport: ${path}: ${first?.message ?? "invalid"}`, [
      "See WorkerReport in src/features/mission/domain/mission-types.ts",
    ]);
  }
  return result.data as WorkerReport;
}

function formatReplyList(replies: readonly WorkerReply[]): string[] {
  if (replies.length === 0) {
    return ["(no replies on disk)"];
  }
  const lines: string[] = [];
  for (const r of replies) {
    lines.push(`${r.missionId}/${r.featureId} [${r.outcome}] ${r.writtenAt} by ${r.writtenBy}`);
    if (r.notes) lines.push(`  note: ${r.notes}`);
  }
  return lines;
}
