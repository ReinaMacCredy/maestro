import { Command } from "commander";
import { readTextOrStdin } from "@/shared/lib/fs.js";
import { buildV2Services } from "../providers/build-services.js";
import {
  planFromSpec,
  PlanRequiresHeavyModeError,
} from "../service/plan-from-spec.usecase.js";
import { planShow } from "../service/plan-show.usecase.js";
import {
  parsePlanDecomposeBatch,
  planDecompose,
  PlanDecomposeBatchEmptyError,
  PlanDecomposeBatchInvalidError,
  PlanDecomposeDuplicateSlugInBatchError,
} from "../service/plan-decompose.usecase.js";
import {
  DuplicateExecPlanSlugError,
  ExecPlanNotFoundError,
} from "../repo/exec-plan-store.port.js";
import { DuplicateSlugError } from "../repo/task-store.port.js";
import { SpecParseError } from "../repo/spec-store.port.js";
import { ExecPlanTransitionError } from "../types/exec-plan-state.js";

export interface PlanCommandV2Options {
  readonly resolveRepoRoot: () => string;
}

function findOrCreatePlanCommand(program: Command): Command {
  const existing = program.commands.find((c) => c.name() === "plan");
  if (existing) return existing;
  return program.command("plan").description("Exec-plan lifecycle (v2)");
}

function reportError(verb: string, err: unknown): void {
  if (
    err instanceof ExecPlanNotFoundError ||
    err instanceof PlanRequiresHeavyModeError ||
    err instanceof SpecParseError ||
    err instanceof DuplicateExecPlanSlugError ||
    err instanceof DuplicateSlugError ||
    err instanceof PlanDecomposeBatchEmptyError ||
    err instanceof PlanDecomposeBatchInvalidError ||
    err instanceof PlanDecomposeDuplicateSlugInBatchError ||
    err instanceof ExecPlanTransitionError
  ) {
    console.error(`maestro ${verb}: ${(err as Error).message}`);
    process.exitCode = 1;
    return;
  }
  throw err;
}

export function registerPlanV2Commands(program: Command, opts: PlanCommandV2Options): void {
  const plan = findOrCreatePlanCommand(program);

  plan
    .command("from-spec <path>")
    .description("Create a v2 exec-plan in 'specified' from a heavy-mode product-spec markdown file")
    .action(async (pathArg: string): Promise<void> => {
      try {
        const repoRoot = opts.resolveRepoRoot();
        const services = buildV2Services({ repoRoot });
        const created = await planFromSpec(
          {
            repoRoot,
            planStore: services.planStore,
            evidenceStore: services.evidenceStore,
          },
          pathArg,
        );
        console.log(`${created.id} specified (${created.slug})`);
      } catch (err) {
        reportError("plan from-spec", err);
      }
    });

  plan
    .command("show <id>")
    .description("Show an exec-plan and its child tasks (state, slug, title)")
    .option("--json", "emit JSON instead of text")
    .action(async function (this: Command, id: string, flags: { json?: boolean }): Promise<void> {
      try {
        const repoRoot = opts.resolveRepoRoot();
        const services = buildV2Services({ repoRoot });
        const result = await planShow(
          { planStore: services.planStore, taskStore: services.taskStore },
          id,
        );
        const wantJson = flags.json === true || this.optsWithGlobals().json === true;
        if (wantJson) {
          console.log(JSON.stringify(result, null, 2));
          return;
        }
        const { plan: p, tasks } = result;
        console.log(`${p.id} ${p.state} (${p.slug}) — ${p.title}`);
        if (p.spec_path) console.log(`  spec: ${p.spec_path}`);
        if (tasks.length === 0) {
          console.log("  (no child tasks yet)");
          return;
        }
        console.log(`  tasks (${tasks.length}):`);
        for (const t of tasks) {
          console.log(`    ${t.id} ${t.state.padEnd(10)} ${t.slug} — ${t.title}`);
        }
      } catch (err) {
        reportError("plan show", err);
      }
    });

  plan
    .command("decompose <id>")
    .description(
      "Decompose a 'specified' plan into child tasks; reads a task batch JSON from --file (or '-' for stdin)",
    )
    .requiredOption(
      "--file <path>",
      "path to a JSON file with the task batch ('-' to read from stdin)",
    )
    .action(async (id: string, flags: { file: string }): Promise<void> => {
      try {
        const repoRoot = opts.resolveRepoRoot();
        const services = buildV2Services({ repoRoot });
        const raw = await readTextOrStdin(flags.file);
        if (raw === undefined) {
          console.error(`maestro plan decompose: batch file not found: ${flags.file}`);
          process.exitCode = 1;
          return;
        }
        let parsed: unknown;
        try {
          parsed = JSON.parse(raw);
        } catch (jsonErr) {
          console.error(
            `maestro plan decompose: invalid JSON in ${flags.file}: ${(jsonErr as Error).message}`,
          );
          process.exitCode = 1;
          return;
        }
        const tasks = parsePlanDecomposeBatch(parsed);
        const result = await planDecompose(
          {
            planStore: services.planStore,
            taskStore: services.taskStore,
            evidenceStore: services.evidenceStore,
          },
          { plan_id: id, tasks },
        );
        console.log(
          `${result.plan.id} planned (${result.tasks.length} task${result.tasks.length === 1 ? "" : "s"})`,
        );
        for (const t of result.tasks) {
          console.log(`  ${t.id} draft ${t.slug} — ${t.title}`);
        }
      } catch (err) {
        reportError("plan decompose", err);
      }
    });
}
