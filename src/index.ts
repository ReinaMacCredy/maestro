#!/usr/bin/env bun
import { basename, win32 } from "node:path";
import { Command, CommanderError } from "commander";
import { formatVersionOutputForArgv } from "@/shared/version-format.js";
import { VERSION } from "@/shared/version.js";
import { MaestroError } from "@/shared/errors.js";
import { removeIfExists } from "@/shared/lib/fs.js";
import { resolveMaestroProjectRoot } from "@/shared/lib/project-root.js";
import { initServices } from "./services.js";
import { checkForUpdate, isNewerSemver } from "@/infra/usecases/check-for-update.usecase.js";
import { registerInitCommand } from "@/infra/commands/init.command.js";
import { registerStatusCommand } from "@/infra/commands/status.command.js";
import { registerDoctorCommand } from "@/infra/commands/doctor.command.js";
import { registerInstallCommand } from "@/infra/commands/install.command.js";
import { registerUpdateCommand } from "@/infra/commands/update.command.js";
import { registerUninstallCommand } from "@/infra/commands/uninstall.command.js";
import {
  resolveInstallDir,
  resolveInstalledBinaryName,
} from "@/infra/usecases/install-release-binary.usecase.js";
import { registerNoteCommand } from "./features/notes/index.js";
import { registerSessionCommand } from "./features/session/index.js";
import {
  registerMissionCommand,
  registerFeatureCommand,
  registerValidateCommand,
  registerMilestoneCommand,
  registerCheckpointCommand,
  registerPrincipleCommand,
} from "./features/mission/index.js";
import { registerMissionControlCommand } from "@/infra/commands/mission-control.command.js";
import {
  registerMemoryCorrectCommand,
  registerMemoryRecallCommand,
  registerMemorySearchCommand,
  registerMemoryLearnCommand,
  registerMemoryCompileCommand,
  registerMemoryStatsCommand,
  registerMemoryLintCommand,
} from "./features/memory/index.js";
import {
  registerRatchetCheckCommand,
  registerRatchetPromoteCommand,
} from "./features/ratchet/index.js";
import {
  registerGraphLinkCommand,
  registerGraphContextCommand,
} from "./features/graph/index.js";
import { registerHandoffCommand } from "./features/handoff/index.js";
import { registerTaskCommand } from "./features/task/index.js";
import { registerReplyCommand } from "./features/reply/index.js";
import { registerBundleCommand } from "./features/bundle/index.js";

export const program = new Command()
  .name("maestro")
  .description("Conductor CLI -- shared mission, feature, and memory state for cross-agent workflows")
  .version(formatVersionOutputForArgv())
  .option("--json", "Output as JSON")
  .exitOverride()
  .hook("preAction", () => {
    initServices(resolveMaestroProjectRoot(process.cwd()));
  });

registerInitCommand(program);
registerStatusCommand(program);
registerDoctorCommand(program);
registerNoteCommand(program);
registerSessionCommand(program);
registerInstallCommand(program);
registerUpdateCommand(program);
registerUninstallCommand(program);
registerMissionCommand(program);
registerFeatureCommand(program);
registerValidateCommand(program);
registerMilestoneCommand(program);
registerCheckpointCommand(program);
registerMissionControlCommand(program);
registerMemoryCorrectCommand(program);
registerMemoryRecallCommand(program);
registerMemorySearchCommand(program);
registerMemoryLearnCommand(program);
registerMemoryCompileCommand(program);
registerRatchetCheckCommand(program);
registerRatchetPromoteCommand(program);
registerMemoryStatsCommand(program);
registerMemoryLintCommand(program);
registerGraphLinkCommand(program);
registerGraphContextCommand(program);
registerHandoffCommand(program);
registerTaskCommand(program);
registerReplyCommand(program);
registerPrincipleCommand(program);
registerBundleCommand(program);

export function shouldCleanupStaleWindowsBinary(
  platform: NodeJS.Platform = process.platform,
  execPath: string = process.execPath,
): boolean {
  if (platform !== "win32") return false;
  const executableName = basename(execPath.replaceAll("\\", "/")).toLowerCase();
  if (executableName !== resolveInstalledBinaryName("win32")) return false;

  const expectedPath = win32.join(
    resolveInstallDir("win32"),
    resolveInstalledBinaryName("win32"),
  ).toLowerCase();
  return win32.normalize(execPath).toLowerCase() === expectedPath;
}

export async function cleanupStaleWindowsBinary(
  platform: NodeJS.Platform = process.platform,
  execPath: string = process.execPath,
  removeIfExistsImpl: typeof removeIfExists = removeIfExists,
): Promise<void> {
  if (!shouldCleanupStaleWindowsBinary(platform, execPath)) return;
  try {
    await removeIfExistsImpl(`${execPath}.old`);
  } catch {}
}

// Bound on how long we'll wait for an in-flight background refresh after the
// user's command finishes. Healthy networks finish well under this; slow ones
// get aborted so a stuck fetch can't pin the event loop until its own 8s
// timeout fires (verified ~9s hang on cold cache + slow network).
const REFRESH_GRACE_MS = 1500;

async function main(): Promise<void> {
  const refreshController = new AbortController();
  try {
    await cleanupStaleWindowsBinary();
    assertNoDeprecatedMissionControlFlags(process.argv);
    // Run the cache read in parallel with the user's command so the FS read
    // does not delay parsing. Stale-cache refresh is fire-and-forget inside
    // checkForUpdate() and intentionally not awaited here.
    const updateCheckPromise = shouldRunUpdateCheck(process.argv, process.env)
      ? checkForUpdate({ refreshSignal: refreshController.signal }).catch(() => undefined)
      : Promise.resolve(undefined);
    const [, updateCheck] = await Promise.all([
      program.parseAsync(process.argv),
      updateCheckPromise,
    ]);
    await drainRefresh(updateCheck?.refreshing, refreshController);
    maybePrintUpdateBanner(updateCheck?.cached, process.argv, process.env);
  } catch (err) {
    refreshController.abort();
    if (err instanceof CommanderError) {
      process.exit(err.exitCode);
    }
    if (err instanceof MaestroError) {
      const isJson = process.argv.includes("--json");
      if (isJson) {
        console.log(
          JSON.stringify(
            { error: err.message, hints: err.hints },
            null,
            2,
          ),
        );
      } else {
        console.error(`[!] ${err.message}`);
        for (const hint of err.hints) {
          console.error(`    ${hint}`);
        }
      }
      process.exit(1);
    }
    throw err;
  }
}

async function drainRefresh(
  refreshing: Promise<unknown> | undefined,
  controller: AbortController,
): Promise<void> {
  if (!refreshing) {
    controller.abort();
    return;
  }
  let timer: ReturnType<typeof setTimeout> | undefined;
  const grace = new Promise<void>((resolve) => {
    timer = setTimeout(resolve, REFRESH_GRACE_MS);
    timer.unref?.();
  });
  await Promise.race([refreshing.catch(() => undefined), grace]);
  if (timer) clearTimeout(timer);
  controller.abort();
}

export function shouldRunUpdateCheck(
  argv: readonly string[],
  env: NodeJS.ProcessEnv,
): boolean {
  if (env.MAESTRO_NO_UPDATE_CHECK) return false;
  if (env.CI) return false;
  if (env.NODE_ENV === "test") return false;
  if (isPureInfoCommand(argv)) return false;
  // Skip on `update` itself: that command does its own fetch (e.g., --check,
  // or installReleaseBinary), so an ambient refresh would race a duplicate.
  if (isUpdateCommand(argv)) return false;
  return true;
}

export function maybePrintUpdateBanner(
  cached: { readonly latestVersion: string; readonly latestTag: string } | undefined,
  argv: readonly string[],
  env: NodeJS.ProcessEnv,
): void {
  if (!cached) return;
  if (!isNewerSemver(cached.latestVersion, VERSION)) return;
  if (env.MAESTRO_NO_UPDATE_CHECK) return;
  if (env.CI) return;
  if (env.NODE_ENV === "test") return;
  if (!process.stderr.isTTY) return;
  if (isPureInfoCommand(argv)) return;
  if (isUpdateCommand(argv)) return;
  console.error(
    `[maestro] ${cached.latestTag} available (you have ${VERSION}). Run \`maestro update\` to upgrade.`,
  );
}

function isPureInfoCommand(argv: readonly string[]): boolean {
  // Look only at the first user-provided arg after the bin path so flags later
  // in the line don't accidentally trigger suppression.
  const first = argv[2];
  return first === "--version" || first === "-V" || first === "--help" || first === "-h";
}

function isUpdateCommand(argv: readonly string[]): boolean {
  return argv[2] === "update";
}

function assertNoDeprecatedMissionControlFlags(argv: readonly string[]): void {
  if (!argv.includes("mission-control") || !argv.includes("--once")) return;

  throw new MaestroError("`maestro mission-control --once` has been removed", [
    "Use `maestro mission-control --preview` for the dashboard preview",
    "Use `maestro mission-control --preview handoffs` to inspect pending handoffs",
    "Use `maestro mission-control --json` for machine-readable output",
  ]);
}

if (import.meta.main) {
  void main();
}
