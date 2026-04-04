import type { WorkerConfig, WorkerResult } from "../domain/worker-types.js";
import type { TransportPort, TransportSpawnOptions } from "../ports/transport.port.js";
import { isA2aWorkerConfig, isCliWorkerConfig } from "../domain/worker-validators.js";
import { A2aTransportAdapter } from "./a2a-transport.adapter.js";
import { CliTransportAdapter } from "./cli-transport.adapter.js";

export class MultiTransportAdapter implements TransportPort {
  constructor(
    private readonly cliTransport: TransportPort = new CliTransportAdapter(),
    private readonly a2aTransport: TransportPort = new A2aTransportAdapter(),
  ) {}

  spawn(
    workerConfig: WorkerConfig,
    prompt: string,
    opts: TransportSpawnOptions,
  ): Promise<WorkerResult> {
    if (isCliWorkerConfig(workerConfig)) {
      return this.cliTransport.spawn(workerConfig, prompt, opts);
    }

    if (isA2aWorkerConfig(workerConfig)) {
      // [TEMP] A2A disabled for TUI/backend task testing
      throw new Error("A2A transport is temporarily disabled");
    }

    throw new Error(`Unsupported worker transport: ${(workerConfig as { transport?: string }).transport ?? "unknown"}`);
  }
}
