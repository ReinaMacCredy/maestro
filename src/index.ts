#!/usr/bin/env bun
import { basename, win32 } from "node:path";
import { Command, CommanderError } from "commander";
import { formatVersionOutputForArgv } from "@/shared/version-format.js";
import { VERSION } from "@/shared/version.js";
import { MaestroError } from "@/shared/errors.js";
import { removeIfExists } from "@/shared/lib/fs.js";
import { resolveMaestroProjectRoot } from "@/shared/lib/project-root.js";
import { assertNoDeprecatedVersionFlag } from "@/shared/lib/deprecated-version-flag.js";
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
  registerReplyCommand,
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
} from "./features/memory-ratchet/index.js";
import {
  registerGraphLinkCommand,
  registerGraphContextCommand,
} from "./features/graph/index.js";
import { registerHandoffCommand } from "./features/handoff/index.js";
import { registerTaskCommand } from "./features/task/index.js";
import { registerBundleCommand } from "./features/bundle/index.js";
import { registerEvidenceCommand } from "./features/evidence/index.js";
import { registerSpecCommand } from "./features/spec/index.js";
import { registerContractL2Command } from "./features/task/commands/contract-l2.command.js";
import { registerPolicyCommand } from "./features/policy/commands/policy.command.js";
import { registerVerdictCommand } from "./features/verdict/index.js";
import { registerPlanCheckCommand } from "./features/plan/index.js";
import { registerCiVerifyCommand } from "./features/ci/index.js";
import { registerReviewCommand } from "./features/review/index.js";
import { registerMergeAutoCommand } from "./features/merge/index.js";
import { registerDeployCommand } from "./features/deploy/index.js";
import { registerRuntimeCheckCommand } from "./features/runtime/index.js";

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
registerEvidenceCommand(program);
registerSpecCommand(program);
registerContractL2Command(program);
registerPolicyCommand(program);
registerVerdictCommand(program);

const planCmd = program
  .command("plan")
  .description("Plan-time checks for agent tasks");
registerPlanCheckCommand(planCmd, program);

const ciCmd = program
  .command("ci")
  .description("CI integration — runs the verdict pipeline in CI mode");
registerCiVerifyCommand(ciCmd, program);

registerReviewCommand(program);

const mergeCmd = program
  .command("merge")
  .description("Merge controls — auto-merge eligibility and trigger");
registerMergeAutoCommand(mergeCmd, program);

const deployCmd = program
  .command("deploy")
  .description("Deploy safety commands — rollback and gate controls");
registerDeployCommand(deployCmd, program);

const runtimeCmd = program
  .command("runtime")
  .description("Runtime signal checks — query providers and record Evidence");
registerRuntimeCheckCommand(runtimeCmd, program);

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
    assertNoDeprecatedVersionFlag(process.argv);
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
  return !isUpdateCheckSuppressed(argv, env);
}

export function maybePrintUpdateBanner(
  cached: { readonly latestVersion: string; readonly latestTag: string } | undefined,
  argv: readonly string[],
  env: NodeJS.ProcessEnv,
): void {
  if (!cached) return;
  if (!isNewerSemver(cached.latestVersion, VERSION)) return;
  if (isUpdateCheckSuppressed(argv, env)) return;
  if (!process.stderr.isTTY) return;
  console.error(
    `[maestro] ${cached.latestTag} available (you have ${VERSION}). Run \`maestro update\` to upgrade.`,
  );
}

function isUpdateCheckSuppressed(
  argv: readonly string[],
  env: NodeJS.ProcessEnv,
): boolean {
  if (env.MAESTRO_NO_UPDATE_CHECK) return true;
  if (env.CI) return true;
  if (env.NODE_ENV === "test") return true;
  const parsed = parseCommandIntent(argv);
  if (parsed.infoOnly) return true;
  // Skip on `update` itself: that command does its own fetch (e.g., --check,
  // or installReleaseBinary), so an ambient refresh would race a duplicate.
  return parsed.command === "update";
}

function parseCommandIntent(argv: readonly string[]): {
  readonly command?: string;
  readonly infoOnly: boolean;
} {
  for (let i = 2; i < argv.length; i++) {
    const token = argv[i];
    if (!token) continue;
    if (token === "--") return { command: argv[i + 1], infoOnly: false };
    if (token === "--version" || token === "-V" || token === "--help" || token === "-h") {
      return { infoOnly: true };
    }
    if (token === "--json") continue;
    if (token.startsWith("--json=")) continue;
    if (token.startsWith("-")) continue;
    if (token === "help") return { command: token, infoOnly: true };
    for (let j = i + 1; j < argv.length; j++) {
      const nested = argv[j];
      if (nested === "--") break;
      if (nested === "--version" || nested === "-V" || nested === "--help" || nested === "-h") {
        return { command: token, infoOnly: true };
      }
    }
    return { command: token, infoOnly: false };
  }
  return { infoOnly: true };
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
