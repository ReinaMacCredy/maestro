import type { Command } from "commander";
import { recordEvidence as defaultRecordEvidence } from "@/features/evidence/index.js";
import type { EvidenceStorePort } from "@/repo/evidence-store.port.js";
import { buildV2Services } from "@/providers/build-services.js";
import { output, resolveJsonFlag } from "@/shared/lib/output.js";
import { DevPrometheusAdapter } from "../adapters/dev-prometheus.adapter.js";
import { LogTailAdapter } from "../adapters/log-tail.adapter.js";
import type { DevObservabilityPort } from "../ports/dev-observability.port.js";

export interface TaskObserveCommandDeps {
  readonly resolveRepoRoot?: () => string;
  readonly buildPrometheusAdapter?: (baseUrl: string) => DevObservabilityPort;
  readonly buildLogTailAdapter?: (filePath?: string) => DevObservabilityPort;
  readonly recordEvidence?: typeof defaultRecordEvidence;
  readonly getEvidenceStore?: (repoRoot: string) => EvidenceStorePort;
  readonly readEnv?: () => NodeJS.ProcessEnv;
}

const defaultBuildPrometheus = (baseUrl: string): DevObservabilityPort =>
  new DevPrometheusAdapter(baseUrl);

const defaultBuildLogTail = (filePath?: string): DevObservabilityPort =>
  new LogTailAdapter(filePath);

const defaultGetEvidenceStore = (repoRoot: string): EvidenceStorePort =>
  buildV2Services({ repoRoot }).evidenceStore;

function isConfigError(message: string): boolean {
  return (
    message.startsWith("log-tail: no path") ||
    message.includes("MAESTRO_PROMETHEUS_URL") ||
    message.includes("MAESTRO_DEV_LOG_FILE")
  );
}

interface MetricsActionOpts {
  prometheusUrl?: string;
  json?: boolean;
  record?: boolean;
  task?: string;
}

interface LogsActionOpts {
  logFile?: string;
  lines?: string;
  filter?: string;
  json?: boolean;
  record?: boolean;
  task?: string;
}

export function registerTaskObserveCommand(
  task: Command,
  deps: TaskObserveCommandDeps = {},
): void {
  const observe = task
    .command("observe")
    .description("Dev-time per-worktree observability (metrics + logs)");

  observe
    .command("metrics <promql>")
    .description("Run a one-shot PromQL query against the dev metrics backend")
    .option("--prometheus-url <url>", "Override MAESTRO_PROMETHEUS_URL")
    .option("--json", "Emit JSON envelope")
    .option("--record", "Record a manual-note evidence row for the result")
    .option("--task <id>", "Task id (required with --record)")
    .action(async (promql: string, opts: MetricsActionOpts): Promise<void> => {
      const env = (deps.readEnv ?? (() => process.env))();
      const isJson = resolveJsonFlag(opts as Record<string, unknown>, task.parent ?? task);

      if (opts.record === true && (opts.task === undefined || opts.task.length === 0)) {
        console.error("maestro task observe metrics: --record requires --task <id>");
        process.exitCode = 1;
        return;
      }

      const baseUrl = opts.prometheusUrl ?? env.MAESTRO_PROMETHEUS_URL;
      if (baseUrl === undefined || baseUrl.length === 0) {
        console.error(
          "maestro task observe metrics: no metrics URL; pass --prometheus-url or set MAESTRO_PROMETHEUS_URL",
        );
        process.exitCode = 1;
        return;
      }

      let sample;
      try {
        const adapter = (deps.buildPrometheusAdapter ?? defaultBuildPrometheus)(baseUrl);
        sample = await adapter.queryMetric(promql);
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        console.error(`maestro task observe metrics: ${message}`);
        process.exitCode = 2;
        return;
      }

      if (opts.record === true && opts.task !== undefined) {
        try {
          const repoRoot = (deps.resolveRepoRoot ?? (() => process.cwd()))();
          const store = (deps.getEvidenceStore ?? defaultGetEvidenceStore)(repoRoot);
          await (deps.recordEvidence ?? defaultRecordEvidence)(store, {
            task_id: opts.task,
            kind: "manual-note",
            payload: {
              note: `[dev-observation:metrics] query="${promql}" value=${sample.value} source=${sample.source} sampledAt=${sample.sampledAt}`,
            },
            witness_level: "agent-claimed-locally",
          });
        } catch (err) {
          const message = err instanceof Error ? err.message : String(err);
          console.error(`maestro task observe metrics: record failed: ${message}`);
          process.exitCode = 2;
          return;
        }
      }

      output(
        isJson,
        {
          kind: "metrics",
          query: promql,
          value: sample.value,
          source: sample.source,
          sampledAt: sample.sampledAt,
        },
        (r) => [
          `[dev-metrics] value=${r.value}  source=${r.source}  sampled_at=${r.sampledAt}`,
        ],
      );
    });

  observe
    .command("logs")
    .description("Tail the last N lines from the dev log file")
    .option("--log-file <path>", "Override MAESTRO_DEV_LOG_FILE")
    .option("--lines <n>", "Number of trailing lines to return", "100")
    .option("--filter <text>", "Substring filter applied before tail")
    .option("--json", "Emit JSON envelope")
    .option("--record", "Record a manual-note evidence row for the result")
    .option("--task <id>", "Task id (required with --record)")
    .action(async (opts: LogsActionOpts): Promise<void> => {
      const isJson = resolveJsonFlag(opts as Record<string, unknown>, task.parent ?? task);

      if (opts.record === true && (opts.task === undefined || opts.task.length === 0)) {
        console.error("maestro task observe logs: --record requires --task <id>");
        process.exitCode = 1;
        return;
      }

      const linesArg = opts.lines !== undefined ? Number.parseInt(opts.lines, 10) : 100;
      if (!Number.isFinite(linesArg) || linesArg <= 0) {
        console.error("maestro task observe logs: --lines must be a positive integer");
        process.exitCode = 1;
        return;
      }

      let adapter: DevObservabilityPort;
      try {
        adapter = (deps.buildLogTailAdapter ?? defaultBuildLogTail)(opts.logFile);
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        const exit = isConfigError(message) ? 1 : 2;
        console.error(`maestro task observe logs: ${message}`);
        process.exitCode = exit;
        return;
      }

      let tail;
      try {
        tail = await adapter.tailLogs(opts.filter, linesArg);
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        console.error(`maestro task observe logs: ${message}`);
        process.exitCode = 2;
        return;
      }

      if (opts.record === true && opts.task !== undefined) {
        try {
          const repoRoot = (deps.resolveRepoRoot ?? (() => process.cwd()))();
          const store = (deps.getEvidenceStore ?? defaultGetEvidenceStore)(repoRoot);
          await (deps.recordEvidence ?? defaultRecordEvidence)(store, {
            task_id: opts.task,
            kind: "manual-note",
            payload: {
              note: `[dev-observation:logs] lines=${tail.lines.length} filter=${opts.filter ?? ""} source=${tail.source}`,
            },
            witness_level: "agent-claimed-locally",
          });
        } catch (err) {
          const message = err instanceof Error ? err.message : String(err);
          console.error(`maestro task observe logs: record failed: ${message}`);
          process.exitCode = 2;
          return;
        }
      }

      output(
        isJson,
        {
          kind: "logs",
          source: tail.source,
          lines: tail.lines,
        },
        (r) => [
          `[dev-logs] source=${r.source}  lines=${r.lines.length}`,
          ...r.lines.map((l) => l.text),
        ],
      );
    });
}

