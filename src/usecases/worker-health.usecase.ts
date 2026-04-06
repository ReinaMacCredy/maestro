import { execArgv } from "../lib/shell.js";
import { fetchA2aAgentCard, resolveA2aJsonRpcEndpoint } from "../lib/a2a.js";
import { cachedWhich } from "../lib/snapshot-poll-cache.js";
import type { WorkerConfig } from "../domain/worker-types.js";
import { formatWorkerLabel, getWorkerGuidance } from "../domain/worker-presentation.js";
import type {
  MissionControlWorkerHealthCheck,
  MissionControlWorkerHealthRow,
  MissionControlWorkerHealthStatus,
} from "../tui/state/types.js";

interface CliProbeResult {
  readonly status: "ready" | "missing" | "degraded";
  readonly detail?: string;
  readonly checks: readonly MissionControlWorkerHealthCheck[];
}

interface A2aProbeResult {
  readonly status: "ready" | "missing" | "degraded";
  readonly detail?: string;
  readonly checks: readonly MissionControlWorkerHealthCheck[];
}

interface WorkerProbeCacheEntry {
  readonly expiresAtMs: number;
  readonly probe: CliProbeResult | A2aProbeResult;
  readonly lastCheckedAt: string;
}

const DEFAULT_PROBE_CACHE_TTL_MS = 120_000;
const workerProbeCache = new Map<string, WorkerProbeCacheEntry>();

export function clearWorkerProbeCache(): void {
  workerProbeCache.clear();
}

export interface WorkerHealthOptions {
  readonly activeWorkers?: readonly string[];
  readonly probe?: boolean;
  readonly nowIso?: string;
  readonly nowMs?: number;
  readonly cacheTtlMs?: number;
  readonly probeCli?: (slug: string, worker: Extract<WorkerConfig, { transport: "cli" }>) => Promise<CliProbeResult>;
  readonly probeA2a?: (slug: string, worker: Extract<WorkerConfig, { transport: "a2a" }>) => Promise<A2aProbeResult>;
}

export async function getWorkerHealthRows(
  workers: Readonly<Record<string, WorkerConfig>> | undefined,
  opts: WorkerHealthOptions = {},
): Promise<readonly MissionControlWorkerHealthRow[]> {
  const activeWorkers = new Set(opts.activeWorkers ?? []);
  const shouldProbe = opts.probe !== false;
  const nowMs = opts.nowMs ?? Date.now();
  const nowIso = opts.nowIso ?? new Date().toISOString();
  const cacheTtlMs = opts.cacheTtlMs ?? DEFAULT_PROBE_CACHE_TTL_MS;
  const probeCli = opts.probeCli ?? defaultProbeCli;
  const probeA2a = opts.probeA2a ?? defaultProbeA2a;
  const entries = Object.entries(workers ?? {});

  return Promise.all(entries.map(async ([slug, worker]) => {
    const guidance = getWorkerGuidance(slug);
    if (!worker.enabled) {
      return {
        slug,
        label: formatWorkerLabel(slug),
        status: "disabled",
        detail: "disabled",
        lastCheckedAt: nowIso,
        checks: [{ label: "enabled", ok: false, detail: "Worker is disabled in config" }],
        summary: guidance.summary,
        bestFor: guidance.bestFor,
        tradeoffs: guidance.tradeoffs,
      } satisfies MissionControlWorkerHealthRow;
    }

      const probe = shouldProbe
      ? await getCachedProbeResult(slug, worker, nowMs, nowIso, cacheTtlMs, probeCli, probeA2a)
      : getPassiveProbeResult(slug, worker, nowIso);
      const status: MissionControlWorkerHealthStatus = probe.status === "ready" && activeWorkers.has(slug)
      ? "busy"
      : probe.status;

    return {
      slug,
      label: formatWorkerLabel(slug),
      status,
      detail: status === "busy" ? "active on current mission" : probe.detail ?? status,
      lastCheckedAt: probe.lastCheckedAt,
      checks: probe.checks,
      summary: guidance.summary,
      bestFor: guidance.bestFor,
      tradeoffs: guidance.tradeoffs,
    } satisfies MissionControlWorkerHealthRow;
  }));
}

async function getCachedProbeResult(
  slug: string,
  worker: WorkerConfig,
  nowMs: number,
  nowIso: string,
  cacheTtlMs: number,
  probeCli: (slug: string, worker: Extract<WorkerConfig, { transport: "cli" }>) => Promise<CliProbeResult>,
  probeA2a: (slug: string, worker: Extract<WorkerConfig, { transport: "a2a" }>) => Promise<A2aProbeResult>,
): Promise<(CliProbeResult | A2aProbeResult) & { readonly lastCheckedAt: string }> {
  const cacheKey = `${slug}:${JSON.stringify(worker)}`;
  const cached = workerProbeCache.get(cacheKey);
  if (cached && cached.expiresAtMs > nowMs) {
    return {
      ...cached.probe,
      lastCheckedAt: cached.lastCheckedAt,
    };
  }

  const probe = worker.transport === "a2a"
    ? await probeA2a(slug, worker)
    : await probeCli(slug, worker);
  workerProbeCache.set(cacheKey, {
    probe,
    lastCheckedAt: nowIso,
    expiresAtMs: nowMs + cacheTtlMs,
  });
  return {
    ...probe,
    lastCheckedAt: nowIso,
  };
}

function getPassiveProbeResult(
  slug: string,
  worker: WorkerConfig,
  nowIso: string,
): (CliProbeResult | A2aProbeResult) & { readonly lastCheckedAt: string } {
  const cacheKey = `${slug}:${JSON.stringify(worker)}`;
  const cached = workerProbeCache.get(cacheKey);
  if (cached) {
    return {
      ...cached.probe,
      lastCheckedAt: cached.lastCheckedAt,
    };
  }

  return {
    status: "ready",
    detail: "configured; not checked in read-only mode",
    checks: [{ label: "probe skipped", ok: true, detail: "read-only mode" }],
    lastCheckedAt: nowIso,
  };
}

async function defaultProbeCli(
  _slug: string,
  worker: Extract<WorkerConfig, { transport: "cli" }>,
): Promise<CliProbeResult> {
  if (!cachedWhich(worker.command)) {
    return {
      status: "missing",
      detail: `Command not found: ${worker.command}`,
      checks: [{ label: "command found", ok: false, detail: `Command not found: ${worker.command}` }],
    };
  }

  const launch = await execArgv([worker.command, "--version"], { timeout: 2_000 });
  if (launch.exitCode !== 0) {
    const detail = launch.stderr || launch.stdout || `${worker.command} did not answer --version cleanly`;
    return {
      status: "degraded",
      detail,
      checks: [
        { label: "command found", ok: true },
        { label: "launch test", ok: false, detail },
      ],
    };
  }

  return {
    status: "ready",
    checks: [
      { label: "command found", ok: true },
      { label: "launch test", ok: true, detail: launch.stdout || "ok" },
    ],
  };
}

async function defaultProbeA2a(
  _slug: string,
  worker: Extract<WorkerConfig, { transport: "a2a" }>,
): Promise<A2aProbeResult> {
  return probeA2aWorkerReadiness(worker);
}

export async function probeA2aWorkerReadiness(
  worker: Extract<WorkerConfig, { transport: "a2a" }>,
): Promise<A2aProbeResult> {
  try {
    const agentCard = await fetchA2aAgentCard(worker.url, {
      agentCardPath: worker.agentCardPath,
      headers: worker.headers,
      signal: AbortSignal.timeout(2_000),
    });
    const endpoint = resolveA2aJsonRpcEndpoint(worker.url, agentCard);
    return {
      status: "ready",
      detail: "agent card and JSON-RPC endpoint ready",
      checks: [
        { label: "agent card", ok: true, detail: "reachable" },
        { label: "json-rpc endpoint", ok: true, detail: endpoint },
      ],
    };
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    const checkLabel = detail.includes("JSON-RPC endpoint")
      ? "json-rpc endpoint"
      : "agent card";
    return {
      status: "degraded",
      detail,
      checks: [{ label: checkLabel, ok: false, detail }],
    };
  }
}
