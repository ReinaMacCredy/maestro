import type { RuntimeSignal } from "@/features/spec";
import type { RuntimeSignalResult } from "../domain/types.js";
import type { RuntimeMonitorPort } from "../ports/monitor.port.js";

export interface CheckRuntimeSignalsInput {
  readonly signals: readonly RuntimeSignal[];
  readonly monitor: RuntimeMonitorPort;
  readonly now: () => Date;
}

export interface RuntimeSignalCheckOutcome {
  readonly signal: RuntimeSignal;
  readonly result?: RuntimeSignalResult;
  readonly note?: string;
}

export async function checkRuntimeSignals(
  input: CheckRuntimeSignalsInput,
): Promise<readonly RuntimeSignalCheckOutcome[]> {
  const outcomes: RuntimeSignalCheckOutcome[] = [];

  for (const signal of input.signals) {
    if (signal.provider !== "prometheus") {
      outcomes.push({ signal, note: "unsupported provider" });
      continue;
    }

    try {
      const result = await input.monitor.query(signal);
      outcomes.push({ signal, result });
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      outcomes.push({ signal, note: `error: ${message}` });
    }
  }

  return outcomes;
}
