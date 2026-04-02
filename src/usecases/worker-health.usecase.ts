import { execArgv } from "../lib/shell.js";
import type { WorkerConfig } from "../domain/worker-types.js";
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

export interface WorkerHealthOptions {
  readonly activeWorkers?: readonly string[];
  readonly nowIso?: string;
  readonly probeCli?: (slug: string, worker: Extract<WorkerConfig, { transport: "cli" }>) => Promise<CliProbeResult>;
  readonly probeA2a?: (slug: string, worker: Extract<WorkerConfig, { transport: "a2a" }>) => Promise<A2aProbeResult>;
}

export async function getWorkerHealthRows(
  workers: Readonly<Record<string, WorkerConfig>> | undefined,
  opts: WorkerHealthOptions = {},
): Promise<readonly MissionControlWorkerHealthRow[]> {
  const activeWorkers = new Set(opts.activeWorkers ?? []);
  const nowIso = opts.nowIso ?? new Date().toISOString();
  const probeCli = opts.probeCli ?? defaultProbeCli;
  const probeA2a = opts.probeA2a ?? defaultProbeA2a;
  const entries = Object.entries(workers ?? {});

  return Promise.all(entries.map(async ([slug, worker]) => {
    const guidance = workerGuidanceForSlug(slug);
    if (!worker.enabled) {
      return {
        slug,
        label: humanizeWorkerSlug(slug),
        status: "disabled",
        detail: "disabled",
        lastCheckedAt: nowIso,
        checks: [{ label: "enabled", ok: false, detail: "Worker is disabled in config" }],
        summary: guidance.summary,
        bestFor: guidance.bestFor,
        tradeoffs: guidance.tradeoffs,
      } satisfies MissionControlWorkerHealthRow;
    }

    const probe = worker.transport === "a2a"
      ? await probeA2a(slug, worker)
      : await probeCli(slug, worker);
    const status: MissionControlWorkerHealthStatus = probe.status === "ready" && activeWorkers.has(slug)
      ? "busy"
      : probe.status;

    return {
      slug,
      label: humanizeWorkerSlug(slug),
      status,
      detail: status === "busy" ? "active on current mission" : probe.detail ?? status,
      lastCheckedAt: nowIso,
      checks: probe.checks,
      summary: guidance.summary,
      bestFor: guidance.bestFor,
      tradeoffs: guidance.tradeoffs,
    } satisfies MissionControlWorkerHealthRow;
  }));
}

async function defaultProbeCli(
  _slug: string,
  worker: Extract<WorkerConfig, { transport: "cli" }>,
): Promise<CliProbeResult> {
  if (!Bun.which(worker.command)) {
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
  try {
    const cardUrl = new URL(worker.agentCardPath ?? "/.well-known/agent-card.json", worker.url).toString();
    const response = await fetch(cardUrl, {
      headers: worker.headers,
      signal: AbortSignal.timeout(2_000),
    });
    if (!response.ok) {
      return {
        status: "degraded",
        detail: `Agent card probe failed: ${response.status}`,
        checks: [{ label: "agent card", ok: false, detail: `HTTP ${response.status}` }],
      };
    }
    return {
      status: "ready",
      checks: [{ label: "agent card", ok: true, detail: "reachable" }],
    };
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    return {
      status: "degraded",
      detail,
      checks: [{ label: "agent card", ok: false, detail }],
    };
  }
}

function workerGuidanceForSlug(slug: string): {
  readonly summary: string;
  readonly bestFor: string;
  readonly tradeoffs: string;
} {
  switch (slug) {
    case "claude-code":
      return {
        summary: "Highest quality, slower and pricier.",
        bestFor: "hard bugs; risky refactors; architecture-heavy work; tasks where correctness matters",
        tradeoffs: "slower; highest cost",
      };
    case "gemini":
      return {
        summary: "Fast and low cost, lighter reasoning.",
        bestFor: "low-risk tasks; drafting and support work; simple follow-up tasks; cheap retries",
        tradeoffs: "weaker on complex tasks; may need more retries",
      };
    case "codex":
    default:
      return {
        summary: "Fast, strong general-purpose coding.",
        bestFor: "everyday implementation; debugging and iteration; medium to high complexity tasks",
        tradeoffs: "less exhaustive than Claude Code; higher cost than Gemini",
      };
  }
}

function humanizeWorkerSlug(slug: string): string {
  return slug
    .split("-")
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}
