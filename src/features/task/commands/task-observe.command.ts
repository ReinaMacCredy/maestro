import type { Command } from "commander";
import { resolveJsonFlag } from "@/shared/lib/output.js";
import { type Services } from "@/services.js";
import {
  DevPrometheusAdapter,
  LogTailAdapter,
  type DevObservabilityPort,
} from "@/features/runtime/index.js";
import { recordEvidence } from "@/features/evidence/index.js";

const DEFAULT_PROMETHEUS_URL = "http://localhost:9090";

interface TaskObserveDeps {
  readonly getServices: () => Pick<Services, "evidenceStore">;
}

export interface TaskObserveCommandOverrides {
  readonly metricsAdapter?: DevObservabilityPort;
  readonly logsAdapter?: DevObservabilityPort;
}

/**
 * Registers `maestro task observe metrics <promql>` and `maestro task observe
 * logs`. Dev-time per-worktree observability for the agent; distinct from
 * `maestro runtime check` which gates L7 deploys via `Spec.runtime_signals`.
 *
 * See `docs/dev-observability.md` and `skills/bundled/maestro-task/SKILL.md`.
 */
export function registerTaskObserveCommand(
  taskCmd: Command,
  program: Command,
  deps: TaskObserveDeps,
  overrides: TaskObserveCommandOverrides = {},
): void {
  const observe = taskCmd
    .command("observe")
    .description("Dev-time per-worktree observability (metrics + log tail)");

  observe
    .command("metrics <promql>")
    .description("Query a Prometheus metric (dev-time, not the deploy gate)")
    .requiredOption("--task <id>", "Task id (used by --record)")
    .option("--provider-base-url <url>", "Prometheus base URL")
    .option("--record", "Persist the result as `manual-note` evidence")
    .option("--json", "Output as JSON")
    .action(async (promql: string, opts): Promise<void> => {
      const isJson = resolveJsonFlag(opts, program);
      const baseUrl: string = opts.providerBaseUrl
        ?? process.env.MAESTRO_PROMETHEUS_URL
        ?? DEFAULT_PROMETHEUS_URL;
      const adapter: DevObservabilityPort = overrides.metricsAdapter
        ?? new DevPrometheusAdapter(baseUrl);
      const sample = await adapter.queryMetric(promql);
      if (opts.record === true) {
        const services = deps.getServices();
        await recordEvidence(services.evidenceStore, {
          task_id: opts.task,
          kind: "manual-note",
          witness_level: "agent-claimed-locally",
          payload: {
            note: `[dev-observation] metric query=${promql} value=${sample.value} sampled_at=${sample.sampledAt} source=${sample.source}`,
          },
        });
      }
      if (isJson) {
        process.stdout.write(JSON.stringify({ ...sample, query: promql }) + "\n");
      } else {
        console.log(`${promql} = ${sample.value}  (source: ${sample.source})`);
      }
    });

  observe
    .command("logs")
    .description("Tail a log file with an optional substring filter")
    .requiredOption("--task <id>", "Task id (used by --record)")
    .option("--log-file <path>", "Path to log file (overrides MAESTRO_DEV_LOG_FILE)")
    .option("--lines <n>", "Number of trailing lines to print", (v) => parseInt(v, 10))
    .option("--filter <substring>", "Only print lines containing this substring")
    .option("--record", "Persist a summary as `manual-note` evidence")
    .option("--json", "Output as JSON")
    .action(async (opts): Promise<void> => {
      const isJson = resolveJsonFlag(opts, program);
      const adapter: DevObservabilityPort = overrides.logsAdapter
        ?? new LogTailAdapter(opts.logFile);
      const tail = await adapter.tailLogs(opts.filter, opts.lines);
      if (opts.record === true) {
        const services = deps.getServices();
        await recordEvidence(services.evidenceStore, {
          task_id: opts.task,
          kind: "manual-note",
          witness_level: "agent-claimed-locally",
          payload: {
            note: `[dev-observation] log tail source=${tail.source} lines=${tail.lines.length}${opts.filter ? ` filter=${opts.filter}` : ""}`,
          },
        });
      }
      if (isJson) {
        process.stdout.write(JSON.stringify({
          source: tail.source,
          lines: tail.lines.map((l) => l.text),
        }) + "\n");
      } else {
        for (const line of tail.lines) console.log(line.text);
      }
    });
}
