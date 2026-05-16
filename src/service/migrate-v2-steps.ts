import { appendFile, mkdir, readFile, readdir, rename, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { dirExists, fileExists } from "@/shared/lib/fs.js";
import { truncateText } from "@/shared/lib/truncate.js";
import {
  mapV1TaskToV2,
  type V1TaskShape,
} from "../repo/v1-state-mapping.js";
import type { ExecPlan } from "../types/exec-plan.js";
import type { Task } from "../types/task.js";
import { DEFAULT_PRINCIPLES } from "./default-principles.js";
import { migrateCorrections } from "./migrate-corrections.usecase.js";
import type { MigrateV2StepResult } from "./migrate-v2.usecase.js";
import { setupCheck } from "./setup-check.usecase.js";

export interface MigrateStepDeps {
  readonly repoRoot: string;
  readonly clock?: () => Date;
}

// Step 4: migrate-corrections — delegate to existing usecase.
export async function runMigrateCorrections(
  deps: MigrateStepDeps,
): Promise<MigrateV2StepResult> {
  try {
    const result = await migrateCorrections({ repoRoot: deps.repoRoot });
    if (result.missing_source) {
      return {
        id: "migrate-corrections",
        label: "Promote v1 corrections to docs/principles/legacy/",
        status: "skipped",
        detail: "no .maestro/memory/corrections — nothing to migrate",
      };
    }
    return {
      id: "migrate-corrections",
      label: "Promote v1 corrections to docs/principles/legacy/",
      status: "ok",
      detail: `migrated ${result.migrated.length}, skipped ${result.skipped.length}`,
    };
  } catch (err) {
    return {
      id: "migrate-corrections",
      label: "Promote v1 corrections to docs/principles/legacy/",
      status: "error",
      detail: (err as Error).message,
    };
  }
}

// Step 5: migrate-tasks — translate v1 task rows in .maestro/tasks/tasks.jsonl
// into v2 shape, atomically. Pass-through rows that are already v2 shape so
// `--force` re-runs after a successful migration do not truncate the store.
const V2_TASK_STATES = new Set<string>([
  "draft",
  "claimed",
  "doing",
  "verifying",
  "blocked",
  "ready",
  "shipped",
  "abandoned",
]);

function isAlreadyV2Task(parsed: unknown): parsed is Task {
  if (typeof parsed !== "object" || parsed === null) return false;
  const row = parsed as Record<string, unknown>;
  return (
    typeof row.id === "string" &&
    typeof row.state === "string" &&
    V2_TASK_STATES.has(row.state)
  );
}

export async function runMigrateTasks(
  deps: MigrateStepDeps,
): Promise<MigrateV2StepResult> {
  const source = join(deps.repoRoot, ".maestro/tasks/tasks.jsonl");
  if (!(await fileExists(source))) {
    return {
      id: "migrate-tasks",
      label: "Translate v1 tasks into tasks.jsonl",
      status: "skipped",
      detail: "no v1 tasks.jsonl",
    };
  }
  const dest = source;
  await mkdir(join(deps.repoRoot, ".maestro/tasks"), { recursive: true });
  const raw = await readFile(source, "utf8");
  const lines = raw.split("\n").filter((l) => l.trim().length > 0);
  const rows: string[] = [];
  let migrated = 0;
  let preserved = 0;
  let skipped = 0;
  // Bound the sample array regardless of how many bad rows the input has;
  // formatSkipReasons reports the count separately from the previews.
  const SKIP_SAMPLE_LIMIT = 5;
  const skippedSamples: string[] = [];
  const recordSkip = (reason: string): void => {
    skipped++;
    if (skippedSamples.length < SKIP_SAMPLE_LIMIT) skippedSamples.push(reason);
  };
  for (let lineIdx = 0; lineIdx < lines.length; lineIdx++) {
    const line = lines[lineIdx]!;
    let parsed: unknown;
    try {
      parsed = JSON.parse(line);
    } catch {
      const preview = truncateText(line.replace(/\s+/g, " "), 60);
      recordSkip(`line ${lineIdx + 1}: invalid JSON (${preview})`);
      continue;
    }
    if (isAlreadyV2Task(parsed)) {
      rows.push(`${JSON.stringify(parsed)}\n`);
      preserved++;
      continue;
    }
    const v1 = parsed as V1TaskShape;
    if (typeof v1.id !== "string" || typeof v1.title !== "string" || typeof v1.status !== "string") {
      const identifier = typeof v1.id === "string" ? v1.id : `line ${lineIdx + 1}`;
      recordSkip(`${identifier}: missing required v1 field (id/title/status)`);
      continue;
    }
    const v2: Task = mapV1TaskToV2(v1);
    rows.push(`${JSON.stringify(v2)}\n`);
    migrated++;
  }
  await writeFile(`${dest}.tmp`, rows.join(""), "utf8");
  // Atomic swap so a crash mid-write cannot leave a truncated store.
  await rename(`${dest}.tmp`, dest);
  const detailSuffix = skipped > 0 ? ` (skipped: ${formatSkipReasons(skippedSamples, skipped)})` : "";
  return {
    id: "migrate-tasks",
    label: "Translate v1 tasks into tasks.jsonl",
    status: "ok",
    detail: `migrated ${migrated}, preserved ${preserved}, skipped ${skipped}${detailSuffix}`,
  };
}

function formatSkipReasons(samples: readonly string[], totalSkipped: number): string {
  const head = samples.join("; ");
  return totalSkipped > samples.length ? `${head}; +${totalSkipped - samples.length} more` : head;
}

// Step 6: migrate-plans — translate v1 mission directories into plans.jsonl.
// Skips missions whose id already appears in the destination JSONL so `--force`
// re-runs do not append duplicate rows.
export async function runMigratePlans(
  deps: MigrateStepDeps,
): Promise<MigrateV2StepResult> {
  const sourceDir = join(deps.repoRoot, ".maestro/missions");
  if (!(await dirExists(sourceDir))) {
    return {
      id: "migrate-plans",
      label: "Translate v1 missions/plans into plans.jsonl",
      status: "skipped",
      detail: "no .maestro/missions/",
    };
  }
  await mkdir(join(deps.repoRoot, ".maestro/plans"), { recursive: true });
  const dest = join(deps.repoRoot, ".maestro/plans/plans.jsonl");
  const knownIds = new Set<string>();
  if (await fileExists(dest)) {
    const raw = await readFile(dest, "utf8");
    for (const line of raw.split("\n")) {
      if (line.trim().length === 0) continue;
      try {
        const row = JSON.parse(line) as { id?: unknown };
        if (typeof row.id === "string") knownIds.add(row.id);
      } catch {
        // ignore malformed rows; they'll fall through unchanged
      }
    }
  }
  const entries = await readdir(sourceDir);
  let migrated = 0;
  let skipped = 0;
  for (const id of entries) {
    const missionFile = join(sourceDir, id, "mission.json");
    if (!(await fileExists(missionFile))) {
      skipped++;
      continue;
    }
    try {
      const raw = await readFile(missionFile, "utf8");
      const v1 = JSON.parse(raw) as {
        id?: string;
        slug?: string;
        title?: string;
        status?: string;
        createdAt?: string;
        updatedAt?: string;
      };
      if (!v1.id || !v1.title) {
        skipped++;
        continue;
      }
      if (knownIds.has(v1.id)) {
        skipped++;
        continue;
      }
      const now = (deps.clock ?? (() => new Date()))().toISOString();
      const plan: ExecPlan = {
        id: v1.id,
        slug: v1.slug ?? v1.id,
        title: v1.title,
        state: v1.status === "completed" ? "completed" : "specified",
        created_at: v1.createdAt ?? now,
        updated_at: v1.updatedAt ?? now,
      };
      await appendFile(dest, `${JSON.stringify(plan)}\n`, "utf8");
      knownIds.add(v1.id);
      migrated++;
    } catch {
      skipped++;
    }
  }
  return {
    id: "migrate-plans",
    label: "Translate v1 missions/plans into plans.jsonl",
    status: "ok",
    detail: `migrated ${migrated}, skipped ${skipped}`,
  };
}

// Step 7: migrate-evidence — copy legacy evidence JSONL files into v2 layout.
// v1 already wrote to .maestro/evidence/<date>.jsonl with shapes compatible
// with v2's EvidenceRow union, so this is effectively a manifest check.
export async function runMigrateEvidence(
  deps: MigrateStepDeps,
): Promise<MigrateV2StepResult> {
  const dir = join(deps.repoRoot, ".maestro/evidence");
  if (!(await dirExists(dir))) {
    return {
      id: "migrate-evidence",
      label: "Rewrite legacy evidence rows into date-stamped JSONL",
      status: "skipped",
      detail: "no .maestro/evidence/",
    };
  }
  const entries = await readdir(dir);
  const jsonlFiles = entries.filter((name) => name.endsWith(".jsonl"));
  return {
    id: "migrate-evidence",
    label: "Rewrite legacy evidence rows into date-stamped JSONL",
    status: "ok",
    detail: `${jsonlFiles.length} evidence file${jsonlFiles.length === 1 ? "" : "s"} present`,
  };
}

// Step 8: migrate-policies — .maestro/policies/ already lives at the canonical
// path in v2. This step is a no-op marker so the assertion table can confirm.
export async function runMigratePolicies(
  deps: MigrateStepDeps,
): Promise<MigrateV2StepResult> {
  const dir = join(deps.repoRoot, ".maestro/policies");
  if (!(await dirExists(dir))) {
    return {
      id: "migrate-policies",
      label: "Carry .maestro/policies/ forward unchanged",
      status: "skipped",
      detail: "no .maestro/policies/",
    };
  }
  return {
    id: "migrate-policies",
    label: "Carry .maestro/policies/ forward unchanged",
    status: "ok",
    detail: "policies dir already in canonical location",
  };
}

// Step 9: seed-principles — if docs/principles/ has no markdown, write the
// default pack so principlesScan has something to act on after migration.
export async function runSeedPrinciples(
  deps: MigrateStepDeps,
): Promise<MigrateV2StepResult> {
  const dir = join(deps.repoRoot, "docs/principles");
  await mkdir(dir, { recursive: true });
  const existing = await readdir(dir);
  const present = existing.filter((name) => name.endsWith(".md") && name !== ".gitkeep");
  if (present.length > 0) {
    return {
      id: "seed-principles",
      label: "Seed default principles pack if docs/principles/ is empty",
      status: "skipped",
      detail: `${present.length} principle file${present.length === 1 ? "" : "s"} already present`,
    };
  }
  for (const p of DEFAULT_PRINCIPLES) {
    await writeFile(join(dir, `${p.slug}.md`), p.content, "utf8");
  }
  return {
    id: "seed-principles",
    label: "Seed default principles pack if docs/principles/ is empty",
    status: "ok",
    detail: `seeded ${DEFAULT_PRINCIPLES.length} principles`,
  };
}

// Step 11: verify — run setupCheck and report ok status.
export async function runVerify(
  deps: MigrateStepDeps,
): Promise<MigrateV2StepResult> {
  const report = await setupCheck({ repoRoot: deps.repoRoot });
  return {
    id: "verify",
    label: "Re-run setup check and confirm OK",
    status: report.ok ? "ok" : "error",
    detail: report.ok
      ? "setup check passed"
      : `setup check still has ${report.entries.filter((e) => e.status !== "ok").length} non-ok entr${report.entries.filter((e) => e.status !== "ok").length === 1 ? "y" : "ies"}`,
  };
}
