import { Command } from "commander";
import { readTextOrStdin } from "@/shared/lib/fs.js";
import { stringifyForOutput } from "@/shared/lib/output.js";
import { buildCoreServices } from "../providers/build-services.js";
import { refreshNowMdFromServices } from "../service/refresh-now-md.js";
import {
  missionFromSpec,
  MissionRequiresHeavyModeError,
} from "../service/mission-from-spec.usecase.js";
import { missionShow } from "../service/mission-show.usecase.js";
import {
  parseMissionDecomposeBatch,
  missionDecompose,
  MissionDecomposeBatchEmptyError,
  MissionDecomposeBatchInvalidError,
  MissionDecomposeDuplicateSlugInBatchError,
  MissionDecomposeAlreadyHasTasksError,
} from "../service/mission-decompose.usecase.js";
import {
  missionNew,
  MissionNewInvalidFlagsError,
  MissionTemplateUnknownError,
  slugifyTitle,
  type MissionNewMode,
} from "../service/mission-new.usecase.js";
import {
  missionCancel,
  MissionCancelTerminalError,
} from "../service/mission-cancel.usecase.js";
import { InvalidTemplateNameError, listTemplates } from "../features/mission/templates/loader.js";
import { MissionTemplateLoadError } from "../features/mission/domain/template-types.js";
import {
  DuplicateMissionSlugError,
  MissionNotFoundError,
} from "../repo/mission-store.port.js";
import { DuplicateSlugError } from "../repo/task-store.port.js";
import { SpecParseError } from "../repo/spec-store.port.js";
import { MissionTransitionError } from "../types/mission-state.js";

export interface MissionCommandOptions {
  readonly resolveRepoRoot: () => string;
}

function findOrCreateMissionCommand(program: Command): Command {
  const existing = program.commands.find((c) => c.name() === "mission");
  if (existing) return existing;
  return program.command("mission").description("Mission lifecycle");
}

function reportError(verb: string, err: unknown): void {
  if (
    err instanceof MissionNotFoundError ||
    err instanceof MissionRequiresHeavyModeError ||
    err instanceof SpecParseError ||
    err instanceof DuplicateMissionSlugError ||
    err instanceof DuplicateSlugError ||
    err instanceof MissionDecomposeBatchEmptyError ||
    err instanceof MissionDecomposeBatchInvalidError ||
    err instanceof MissionDecomposeDuplicateSlugInBatchError ||
    err instanceof MissionDecomposeAlreadyHasTasksError ||
    err instanceof MissionTransitionError ||
    err instanceof MissionNewInvalidFlagsError ||
    err instanceof MissionTemplateUnknownError ||
    err instanceof MissionTemplateLoadError ||
    err instanceof InvalidTemplateNameError ||
    err instanceof MissionCancelTerminalError
  ) {
    console.error(`maestro ${verb}: ${(err as Error).message}`);
    process.exitCode = 1;
    return;
  }
  throw err;
}

export function registerMissionCommands(program: Command, opts: MissionCommandOptions): void {
  const mission = findOrCreateMissionCommand(program);

  mission
    .command("new [title...]")
    .description(
      "Create a mission. Bare title -> intake; --from-spec/--from-file/--template seed further state.",
    )
    .option("--from-spec <path>", "create at 'approved' from a heavy-mode product-spec markdown file")
    .option("--from-file <path>", "create at 'planned' from a JSON task-batch file")
    .option("--template <name>", "create at 'planned' using a built-in or user template")
    .option("--slug <slug>", "explicit slug (default: slugified title)")
    .option("--list-templates", "list available templates and exit")
    .action(async function (this: Command, titleParts: string[], flags): Promise<void> {
      try {
        const repoRoot = opts.resolveRepoRoot();
        const services = buildCoreServices({ repoRoot });

        if (flags.listTemplates === true) {
          const listed = await listTemplates(repoRoot);
          console.log("built-in templates:");
          for (const t of listed.builtin) {
            console.log(`  ${t.name.padEnd(10)} ${t.description}`);
          }
          if (listed.user.length > 0) {
            console.log("");
            console.log("user templates (.maestro/templates/missions/):");
            for (const t of listed.user) {
              const overrideTag = listed.overrides.includes(t.name)
                ? "  (overrides built-in)"
                : "";
              console.log(`  ${t.name.padEnd(10)} ${t.description}${overrideTag}`);
            }
          }
          return;
        }

        const title = titleParts.join(" ").trim();
        if (title.length === 0) {
          console.error("maestro mission new: title is required (or pass --list-templates)");
          process.exitCode = 1;
          return;
        }

        const mode = pickMode(flags);

        const slug = typeof flags.slug === "string" && flags.slug.length > 0
          ? flags.slug
          : slugifyTitle(title);

        const result = await missionNew(
          {
            repoRoot,
            missionStore: services.missionStore,
            taskStore: services.taskStore,
            evidenceStore: services.evidenceStore,
          },
          {
            title,
            slug,
            mode,
            fromSpec: flags.fromSpec,
            fromFile: flags.fromFile,
            template: flags.template,
          },
        );

        console.log(`${result.mission.id} ${result.mission.state} (${result.mission.slug})`);
        if (result.tasks.length > 0) {
          for (const t of result.tasks) {
            console.log(`  ${t.id} draft ${t.slug} -- ${t.title}`);
          }
          await refreshNowMdFromServices(services);
        }
      } catch (err) {
        reportError("mission new", err);
      }
    });

  mission
    .command("cancel <id>")
    .description("Cancel a mission and cascade-abandon its active tasks")
    .option("--reason <text>", "human-readable cancel reason")
    .action(async (id: string, flags: { reason?: string }): Promise<void> => {
      try {
        const repoRoot = opts.resolveRepoRoot();
        const services = buildCoreServices({ repoRoot });
        const result = await missionCancel(
          {
            missionStore: services.missionStore,
            taskStore: services.taskStore,
            evidenceStore: services.evidenceStore,
          },
          { mission_id: id, reason: flags.reason },
        );
        if (result.alreadyCancelled) {
          console.log(`${result.mission.id} cancelled (no-op; already cancelled)`);
          return;
        }
        console.log(
          `${result.mission.id} cancelled (${result.cancelledTaskIds.length} task${result.cancelledTaskIds.length === 1 ? "" : "s"} abandoned)`,
        );
        for (const taskId of result.cancelledTaskIds) {
          console.log(`  ${taskId} abandoned`);
        }
        if (result.cascadeErrors.length > 0) {
          console.error("cascade errors:");
          for (const e of result.cascadeErrors) {
            console.error(`  ${e.taskId}: ${e.message}`);
          }
          process.exitCode = 1;
        }
      } catch (err) {
        reportError("mission cancel", err);
      }
    });

  mission
    .command("from-spec <path>")
    .description("Create a mission in 'approved' from a heavy-mode product-spec markdown file")
    .action(async (pathArg: string): Promise<void> => {
      try {
        const repoRoot = opts.resolveRepoRoot();
        const services = buildCoreServices({ repoRoot });
        const created = await missionFromSpec(
          {
            repoRoot,
            missionStore: services.missionStore,
            evidenceStore: services.evidenceStore,
          },
          pathArg,
        );
        console.log(`${created.id} approved (${created.slug})`);
      } catch (err) {
        reportError("mission from-spec", err);
      }
    });

  mission
    .command("show <id>")
    .description("Show a mission and its child tasks (state, slug, title)")
    .option("--json", "emit JSON instead of text")
    .action(async function (this: Command, id: string, flags: { json?: boolean }): Promise<void> {
      try {
        const repoRoot = opts.resolveRepoRoot();
        const services = buildCoreServices({ repoRoot });
        const result = await missionShow(
          { missionStore: services.missionStore, taskStore: services.taskStore },
          id,
        );
        const wantJson = flags.json === true || this.optsWithGlobals().json === true;
        if (wantJson) {
          console.log(stringifyForOutput(result));
          return;
        }
        const { mission: m, tasks } = result;
        console.log(`${m.id} ${m.state} (${m.slug}) -- ${m.title}`);
        if (m.spec_path) console.log(`  spec: ${m.spec_path}`);
        if (tasks.length === 0) {
          console.log("  (no child tasks yet)");
          return;
        }
        console.log(`  tasks (${tasks.length}):`);
        for (const t of tasks) {
          console.log(`    ${t.id} ${t.state.padEnd(10)} ${t.slug} -- ${t.title}`);
        }
      } catch (err) {
        reportError("mission show", err);
      }
    });

  mission
    .command("decompose <id>")
    .description(
      "Decompose an 'intake' or 'approved' mission into child tasks; reads a task batch JSON from --file (or '-' for stdin)",
    )
    .requiredOption(
      "--file <path>",
      "path to a JSON file with the task batch ('-' to read from stdin)",
    )
    .action(async (id: string, flags: { file: string }): Promise<void> => {
      try {
        const repoRoot = opts.resolveRepoRoot();
        const services = buildCoreServices({ repoRoot });
        const raw = await readTextOrStdin(flags.file);
        if (raw === undefined) {
          console.error(`maestro mission decompose: batch file not found: ${flags.file}`);
          process.exitCode = 1;
          return;
        }
        let parsed: unknown;
        try {
          parsed = JSON.parse(raw);
        } catch (jsonErr) {
          console.error(
            `maestro mission decompose: invalid JSON in ${flags.file}: ${(jsonErr as Error).message}`,
          );
          process.exitCode = 1;
          return;
        }
        const tasks = parseMissionDecomposeBatch(parsed);
        const result = await missionDecompose(
          {
            missionStore: services.missionStore,
            taskStore: services.taskStore,
            evidenceStore: services.evidenceStore,
            observabilityStore: services.observabilityStore,
          },
          { mission_id: id, tasks },
        );
        console.log(
          `${result.mission.id} planned (${result.tasks.length} task${result.tasks.length === 1 ? "" : "s"})`,
        );
        for (const t of result.tasks) {
          console.log(`  ${t.id} draft ${t.slug} -- ${t.title}`);
        }
        await refreshNowMdFromServices(services);
      } catch (err) {
        reportError("mission decompose", err);
      }
    });
}

function pickMode(flags: {
  fromSpec?: string;
  fromFile?: string;
  template?: string;
}): MissionNewMode {
  const set = [
    flags.fromSpec ? "from-spec" : null,
    flags.fromFile ? "from-file" : null,
    flags.template ? "template" : null,
  ].filter((v): v is string => v !== null);
  if (set.length > 1) {
    throw new MissionNewInvalidFlagsError(
      `--from-spec, --from-file, --template are mutually exclusive (got ${set.join(", ")})`,
    );
  }
  if (set.length === 0) return "bare";
  return set[0] as MissionNewMode;
}
