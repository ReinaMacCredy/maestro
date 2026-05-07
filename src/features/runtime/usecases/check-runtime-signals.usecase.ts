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
  return Promise.all(
    input.signals.map(async (signal): Promise<RuntimeSignalCheckOutcome> => {
      if (signal.provider !== "prometheus") {
        return { signal, note: "unsupported provider" };
      }
      try {
        const result = await input.monitor.query(signal);
        return { signal, result };
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        return { signal, note: `error: ${message}` };
      }
    }),
  );
}
