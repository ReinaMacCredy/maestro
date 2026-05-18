import type { Command } from "commander";
import { MaestroError } from "@/shared/errors.js";
import { output, resolveJsonFlag } from "@/shared/lib/output.js";
import { type Services } from "@/services.js";
import { recordEvidence as defaultRecordEvidence } from "@/features/evidence/index.js";
import type { RuntimeSignalPayload } from "@/features/evidence/index.js";
import { PrometheusRuntimeMonitor } from "../adapters/prometheus.adapter.js";
import { checkRuntimeSignals } from "../usecases/check-runtime-signals.usecase.js";
import type { RuntimeMonitorPort } from "../ports/monitor.port.js";

interface RuntimeCheckCommandDeps {
  readonly getServices: () => Pick<Services, "legacyEvidenceStore" | "legacyTaskStore" | "trustSpecStore">;
  readonly recordEvidence?: typeof defaultRecordEvidence;
  readonly buildMonitor?: (baseUrl: string) => RuntimeMonitorPort;
}

const defaultBuildMonitor = (baseUrl: string): RuntimeMonitorPort =>
  new PrometheusRuntimeMonitor(baseUrl);

export function registerRuntimeCheckCommand(
  parent: Command,
  program: Command,
  deps: RuntimeCheckCommandDeps,
): void {
  parent
    .command("check")
    .description("Query runtime signals from Spec and record Evidence rows")
    .requiredOption("--task <id>", "Task ID")
    .option("--provider-base-url <url>", "Prometheus base URL (overrides MAESTRO_PROMETHEUS_URL env)")
    .option("--json", "Output as JSON")
    .action(async (opts: { task: string; providerBaseUrl?: string; json?: boolean }): Promise<void> => {
      const { task: taskId, providerBaseUrl } = opts;
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, program);

      const task = await services.legacyTaskStore.get(taskId);
      if (task === undefined) {
        throw new MaestroError(`Task not found: ${taskId}`, [
          "Run `maestro task list` to see available tasks",
        ]);
      }

      if (task.missionId === undefined) {
        throw new MaestroError(`Task ${taskId} has no associated mission`, [
          "runtime check requires a mission with a Spec containing runtime_signals",
        ]);
      }

      const spec = await services.trustSpecStore.read(task.missionId);
      if (spec === undefined) {
        throw new MaestroError(`No Spec found for mission: ${task.missionId}`, [
          "Run `maestro spec show --mission <id>` to inspect the spec",
        ]);
      }

      const signals = spec.runtime_signals;
      if (signals.length === 0) {
        output(isJson, { taskId, outcomes: [] }, () => [
          `[ok] No runtime signals defined in Spec for mission ${task.missionId}.`,
        ]);
        return;
      }

      const baseUrl =
        providerBaseUrl ??
        process.env.MAESTRO_PROMETHEUS_URL ??
        "http://localhost:9090";

      const monitor = (deps.buildMonitor ?? defaultBuildMonitor)(baseUrl);
      const now = () => new Date();

      const outcomes = await checkRuntimeSignals({ signals, monitor, now });

      const evidenceIds: string[] = [];

      for (const outcome of outcomes) {
        const { signal, result, note } = outcome;

        if (note === "unsupported provider") {
          console.log(`[skip] provider ${signal.provider} not supported`);
        }

        const payload: RuntimeSignalPayload = result !== undefined
          ? {
              signal_name: signal.name,
              provider: signal.provider,
              query: signal.query,
              value: result.value,
              threshold: result.threshold,
              operator: result.operator,
              pass: result.pass,
              sampled_at: result.sampled_at,
            }
          : {
              signal_name: signal.name,
              provider: signal.provider,
              query: signal.query,
              value: 0,
              threshold: signal.threshold.value,
              operator: signal.threshold.operator,
              pass: false,
              sampled_at: now().toISOString(),
              note,
            };

        const row = await (deps.recordEvidence ?? defaultRecordEvidence)(services.legacyEvidenceStore, {
          task_id: taskId,
          kind: "runtime-signal",
          payload,
          witness_level: "agent-claimed-locally",
        });

        evidenceIds.push(row.id);
      }

      const passCount = outcomes.filter((o) => o.result?.pass === true).length;
      const failCount = outcomes.filter((o) => o.result !== undefined && !o.result.pass).length;
      const skipCount = outcomes.filter((o) => o.result === undefined).length;

      output(
        isJson,
        {
          taskId,
          outcomes: outcomes.map((o, i) => ({
            signal_name: o.signal.name,
            provider: o.signal.provider,
            pass: o.result?.pass ?? false,
            note: o.note,
            evidence_id: evidenceIds[i],
          })),
          summary: { pass: passCount, fail: failCount, skip: skipCount },
        },
        (r) => [
          `Runtime signal check for task ${r.taskId}:`,
          ...r.outcomes.map((o) => {
            const status = o.note !== undefined ? `[skip]` : o.pass ? `[pass]` : `[fail]`;
            const detail = o.note !== undefined ? ` (${o.note})` : "";
            return `  ${status} ${o.signal_name}${detail}`;
          }),
          `  pass: ${r.summary.pass}  fail: ${r.summary.fail}  skip: ${r.summary.skip}`,
        ],
      );
    });
}
