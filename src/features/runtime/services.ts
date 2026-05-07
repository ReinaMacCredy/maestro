import type { RuntimeMonitorPort } from "./ports/monitor.port.js";
import { PrometheusRuntimeMonitor } from "./adapters/prometheus.adapter.js";

export interface RuntimeServices {
  readonly runtimeMonitor: RuntimeMonitorPort;
}

export function buildRuntimeServices(opts?: { baseUrl?: string }): RuntimeServices {
  const baseUrl = opts?.baseUrl ?? process.env.MAESTRO_PROMETHEUS_URL ?? "http://localhost:9090";
  return { runtimeMonitor: new PrometheusRuntimeMonitor(baseUrl) };
}
