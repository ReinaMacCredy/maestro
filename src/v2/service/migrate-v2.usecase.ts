import { backupMaestroDir, type BackupResult } from "./migrate-v2-backup.js";
import {
  MIGRATION_FLAG_VERSION,
  readMigrationFlag,
  writeMigrationFlag,
  type MigrationFlag,
} from "./migrate-v2-flag.js";
import { setupBootstrap } from "./setup-bootstrap.usecase.js";

export type MigrateV2StepStatus = "ok" | "skipped" | "error" | "not-implemented";

export interface MigrateV2StepResult {
  readonly id: string;
  readonly label: string;
  readonly status: MigrateV2StepStatus;
  readonly detail?: string;
}

export interface MigrateV2Deps {
  readonly repoRoot: string;
  readonly clock?: () => Date;
}

export interface MigrateV2Input {
  readonly dryRun?: boolean;
  readonly force?: boolean;
}

export interface MigrateV2Result {
  readonly ok: boolean;
  readonly steps: readonly MigrateV2StepResult[];
  readonly backup?: BackupResult;
  readonly flag?: MigrationFlag;
  readonly dry_run: boolean;
  readonly already_migrated: boolean;
}

// The 11-step migration table. PR 32 ships step 1 (preflight) plus the
// backup/flag scaffolding. Steps 2-11 are stubs that emit a not-implemented
// row so the assertion table in PR 33 can light them up incrementally.
export const MIGRATE_V2_STEP_TABLE: readonly { id: string; label: string }[] = [
  { id: "preflight", label: "Validate source layout and prerequisites" },
  { id: "backup", label: "Snapshot .maestro/ to .maestro.backup-<ts>/" },
  { id: "bootstrap-dirs", label: "Create v2 directory tree (tasks/plans/evidence/runs)" },
  { id: "migrate-corrections", label: "Promote v1 corrections to docs/principles/legacy/" },
  { id: "migrate-tasks", label: "Translate v1 tasks into tasks.v2.jsonl" },
  { id: "migrate-plans", label: "Translate v1 missions/plans into plans.v2.jsonl" },
  { id: "migrate-evidence", label: "Rewrite legacy evidence rows into date-stamped JSONL" },
  { id: "migrate-policies", label: "Carry .maestro/policies/ forward unchanged" },
  { id: "seed-principles", label: "Seed default principles pack if docs/principles/ is empty" },
  { id: "write-flag", label: "Write .maestro/.migrated-v2.json" },
  { id: "verify", label: "Re-run setup check and confirm OK" },
];

export async function migrateV2(
  deps: MigrateV2Deps,
  input: MigrateV2Input = {},
): Promise<MigrateV2Result> {
  const clock = deps.clock ?? (() => new Date());
  const dryRun = input.dryRun === true;
  const force = input.force === true;

  const existing = await readMigrationFlag(deps.repoRoot);
  if (existing && !force) {
    return {
      ok: true,
      steps: [
        {
          id: "preflight",
          label: "Validate source layout and prerequisites",
          status: "skipped",
          detail: `repo already migrated at ${existing.migrated_at}; pass --force to re-run`,
        },
      ],
      flag: existing,
      dry_run: dryRun,
      already_migrated: true,
    };
  }

  const steps: MigrateV2StepResult[] = [];
  let backup: BackupResult | undefined;

  // Step 1: preflight. The only step PR 32 implements end-to-end.
  steps.push({
    id: "preflight",
    label: "Validate source layout and prerequisites",
    status: "ok",
    detail: dryRun ? "dry-run mode: no side effects" : undefined,
  });

  // Step 2: backup (scaffold). In dry-run mode the action is reported but
  // the cp() is skipped.
  if (dryRun) {
    steps.push({
      id: "backup",
      label: "Snapshot .maestro/ to .maestro.backup-<ts>/",
      status: "skipped",
      detail: "dry-run mode: backup not performed",
    });
  } else {
    backup = await backupMaestroDir(deps.repoRoot, clock().toISOString());
    steps.push({
      id: "backup",
      label: "Snapshot .maestro/ to .maestro.backup-<ts>/",
      status: backup.created ? "ok" : "skipped",
      detail: backup.created
        ? `backup written to ${backup.destination}`
        : "no .maestro/ to back up",
    });
  }

  // Step 3: bootstrap-dirs runs unconditionally (idempotent) so step 4+ have
  // somewhere to write. In dry-run we report it as skipped.
  if (dryRun) {
    steps.push({
      id: "bootstrap-dirs",
      label: "Create v2 directory tree (tasks/plans/evidence/runs)",
      status: "skipped",
      detail: "dry-run mode: directories not created",
    });
  } else {
    const result = await setupBootstrap({ repoRoot: deps.repoRoot });
    steps.push({
      id: "bootstrap-dirs",
      label: "Create v2 directory tree (tasks/plans/evidence/runs)",
      status: "ok",
      detail: `created ${result.created.length}, skipped ${result.skipped.length}`,
    });
  }

  // Steps 4-9: stubs filled in by PR 33 (migrate-corrections..seed-principles).
  for (let i = 3; i <= 8; i++) {
    const step = MIGRATE_V2_STEP_TABLE[i]!;
    steps.push({
      id: step.id,
      label: step.label,
      status: "not-implemented",
      detail: "filled in by PR 33",
    });
  }

  // Step 10: write-flag. Run early in the scaffold so re-runs are idempotent
  // even before steps 4-9 land. Flag records only the steps that ran ok.
  const stepIdsCompleted = steps.filter((s) => s.status === "ok").map((s) => s.id);
  let flag: MigrationFlag | undefined;
  if (!dryRun) {
    flag = {
      version: MIGRATION_FLAG_VERSION,
      migrated_at: clock().toISOString(),
      steps: stepIdsCompleted,
      backup_path: backup?.created ? backup.destination : undefined,
    };
    await writeMigrationFlag(deps.repoRoot, flag);
    steps.push({
      id: "write-flag",
      label: "Write .maestro/.migrated-v2.json",
      status: "ok",
    });
  } else {
    steps.push({
      id: "write-flag",
      label: "Write .maestro/.migrated-v2.json",
      status: "skipped",
      detail: "dry-run mode: flag not written",
    });
  }

  // Step 11: verify. Stubbed in PR 32; PR 33 will run setupCheck and assert OK.
  steps.push({
    id: "verify",
    label: "Re-run setup check and confirm OK",
    status: "not-implemented",
    detail: "filled in by PR 33",
  });

  return {
    ok: steps.every((s) => s.status !== "error"),
    steps,
    backup,
    flag,
    dry_run: dryRun,
    already_migrated: false,
  };
}
