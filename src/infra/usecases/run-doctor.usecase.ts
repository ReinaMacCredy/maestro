import { stat } from "node:fs/promises";
import { join } from "node:path";
import type { ConfigPort } from "../ports/config.port.js";
import type { TaskStorePort } from "@/repo/task-store.port.js";
import type { VerdictStorePort } from "@/features/verdict/ports/storage.js";
import type { DoctorCheck } from "@/infra/domain/status-types.js";
import { setupCheck, type SetupCheckEntry } from "@/service/setup-check.usecase.js";
import { execArgv } from "@/shared/lib/shell.js";

export interface RunDoctorDeps {
  readonly taskStore: TaskStorePort;
  readonly verdictStore: VerdictStorePort;
  readonly projectDir: string;
  readonly full?: boolean;
  readonly config?: ConfigPort;
}

const DAY_MS = 86_400_000;
const DEFAULT_STALE_DAYS = 30;
const SUBPROCESS_TIMEOUT_MS = 600_000;

export async function runDoctor(deps: RunDoctorDeps): Promise<DoctorCheck[]> {
  const checks: DoctorCheck[] = await Promise.all([
    scaffoldCheck(deps.projectDir),
    initScriptCheck(deps.projectDir),
    verdictFreshnessCheck(deps),
  ]);

  if (deps.full) {
    checks.push(await subprocessCheck("build", ["bun", "run", "build"], deps.projectDir));
    checks.push(await subprocessCheck("tests", ["bun", "test"], deps.projectDir));
  }

  return checks;
}

async function scaffoldCheck(projectDir: string): Promise<DoctorCheck> {
  const report = await setupCheck({ repoRoot: projectDir });
  const missing = report.entries.filter((e): e is SetupCheckEntry => e.status === "missing");
  if (missing.length > 0) {
    return {
      name: "scaffold",
      status: "fail",
      message: `Scaffold incomplete (${missing.length} missing): ${missing.map((e) => e.path).join(", ")}`,
      fix: "Run: maestro setup",
    };
  }
  const warns = report.entries.filter((e) => e.status === "warn");
  if (warns.length > 0) {
    return {
      name: "scaffold",
      status: "warn",
      message: `Scaffold has ${warns.length} warning(s): ${warns.map((e) => e.path).join(", ")}`,
      fix: "Run: maestro setup",
    };
  }
  return {
    name: "scaffold",
    status: "ok",
    message: "Scaffold complete",
  };
}

async function initScriptCheck(projectDir: string): Promise<DoctorCheck> {
  const path = join(projectDir, "init.sh");
  let mode: number;
  try {
    const s = await stat(path);
    mode = s.mode;
  } catch {
    return {
      name: "init-script",
      status: "warn",
      message: "init.sh not found at repo root",
      fix: "Run: maestro setup",
    };
  }
  // On win32 the execute bit has no meaning; presence is enough.
  if (process.platform !== "win32" && (mode & 0o111) === 0) {
    return {
      name: "init-script",
      status: "warn",
      message: "init.sh exists but is not executable",
      fix: "Run: chmod +x init.sh",
    };
  }
  return {
    name: "init-script",
    status: "ok",
    message: "init.sh present and executable",
  };
}

async function verdictFreshnessCheck(deps: RunDoctorDeps): Promise<DoctorCheck> {
  let tasks: Awaited<ReturnType<TaskStorePort["list"]>>;
  try {
    tasks = await deps.taskStore.list();
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    return {
      name: "verdict-freshness",
      status: "fail",
      message: `Task store unreadable: ${message}`,
      fix: "Inspect .maestro/tasks/tasks.jsonl and remove or repair malformed rows",
    };
  }
  if (tasks.length === 0) {
    return {
      name: "verdict-freshness",
      status: "ok",
      message: "No tasks yet",
    };
  }

  const verdicts = await Promise.all(
    tasks.map(async (t): Promise<Awaited<ReturnType<VerdictStorePort["readLatest"]>>> => {
      // A single corrupt verdict file must not block the dimension.
      try {
        return await deps.verdictStore.readLatest(t.id);
      } catch {
        return undefined;
      }
    }),
  );

  let newest: { taskId: string; computedAt: string } | undefined;
  for (const v of verdicts) {
    if (!v) continue;
    if (!newest || v.computedAt.localeCompare(newest.computedAt) > 0) {
      newest = { taskId: v.taskId, computedAt: v.computedAt };
    }
  }

  if (!newest) {
    return {
      name: "verdict-freshness",
      status: "fail",
      message: `${tasks.length} task(s) exist but no verdicts have been written`,
      fix: "Run: maestro task verify <id>",
    };
  }

  const staleDays = await resolveStaleDays(deps);
  const ageMs = Date.now() - Date.parse(newest.computedAt);
  if (Number.isNaN(ageMs) || ageMs > staleDays * DAY_MS) {
    return {
      name: "verdict-freshness",
      status: "warn",
      message: `Latest verdict (${newest.taskId}) is older than ${staleDays} day(s)`,
      fix: "Run: maestro task verify <id>",
    };
  }

  return {
    name: "verdict-freshness",
    status: "ok",
    message: `Latest verdict at ${newest.computedAt} (${newest.taskId})`,
  };
}

async function resolveStaleDays(deps: RunDoctorDeps): Promise<number> {
  const envValue = process.env.MAESTRO_VERDICT_STALE_DAYS;
  if (envValue !== undefined && envValue !== "") {
    const parsed = Number.parseInt(envValue, 10);
    if (!Number.isNaN(parsed) && parsed > 0) return parsed;
  }
  if (deps.config) {
    try {
      const layers = await deps.config.loadLayers(deps.projectDir);
      const fromConfig = layers.effective?.doctor?.verdictStaleDays;
      if (typeof fromConfig === "number" && fromConfig > 0) return fromConfig;
    } catch {
      // Config layer read failure falls through to the default.
    }
  }
  return DEFAULT_STALE_DAYS;
}

async function subprocessCheck(
  name: "build" | "tests",
  command: readonly string[],
  cwd: string,
): Promise<DoctorCheck> {
  const { exitCode } = await execArgv([...command], {
    cwd,
    timeout: SUBPROCESS_TIMEOUT_MS,
  });
  if (exitCode === 0) {
    return {
      name,
      status: "ok",
      message: `${command.join(" ")} succeeded`,
    };
  }
  return {
    name,
    status: "warn",
    message: `${command.join(" ")} exited ${exitCode}`,
    fix: `Run \`${command.join(" ")}\` manually to inspect the failure`,
  };
}
