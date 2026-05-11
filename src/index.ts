#!/usr/bin/env bun
import { basename, win32 } from "node:path";
import { Command, CommanderError } from "commander";
import { formatVersionOutputForArgv } from "@/shared/version-format.js";
import { VERSION } from "@/shared/version.js";
import { MaestroError } from "@/shared/errors.js";
import { removeIfExists } from "@/shared/lib/fs.js";
import { resolveMaestroProjectRoot } from "@/shared/lib/project-root.js";
import { assertNoDeprecatedVersionFlag } from "@/infra/lib/deprecated-version-flag.js";
import { createServices, type Services } from "./services.js";
import { checkForUpdate, isNewerSemver } from "@/infra/usecases/check-for-update.usecase.js";
import { registerInitCommand } from "@/infra/commands/init.command.js";
import { registerStatusCommand } from "@/infra/commands/status.command.js";
import { registerDoctorCommand } from "@/infra/commands/doctor.command.js";
import { registerInstallCommand } from "@/infra/commands/install.command.js";
import { registerUpdateCommand } from "@/infra/commands/update.command.js";
import { registerUninstallCommand } from "@/infra/commands/uninstall.command.js";
import { registerProvidersCommand } from "@/infra/commands/providers.command.js";
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
// Mission Control lazy-loads — its import graph (OpenTUI + React) costs
// ~250ms on cold start, but is only needed when the `mission-control` verb
// runs. Every other verb (and `--version`/`--help`) skips it entirely.
// See profile-imports.ts for the measurements.
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
import { registerSkillsCommand } from "./features/skills/index.js";
import { registerIntakeCommand } from "./features/intake/index.js";
import { registerMcpCommand } from "./features/mcp/index.js";
import { registerRecoverCommand } from "./features/recover/index.js";
import { registerGcCommand } from "./features/gc/index.js";
import { registerRalphCommand } from "./features/ralph/index.js";
import { registerStateCommand } from "./features/state/index.js";
import { registerWorktreeCommand } from "./features/worktree/index.js";
import { registerInspectCommand } from "./features/inspect/index.js";
import { registerSetupCommand } from "./features/setup/index.js";

// One process-wide cache for the composed Services graph. The thunk stays
// lazy so `--version`, `--help`, and other info-only paths never bootstrap
// the per-feature service builders.
let cachedServices: Services | undefined;
const getServices = (): Services => {
  if (!cachedServices) {
    cachedServices = createServices(resolveMaestroProjectRoot(process.cwd()));
  }
  return cachedServices;
};
const deps = { getServices };

export const program = new Command()
  .name("maestro")
  .description("Conductor CLI -- shared mission, feature, and memory state for cross-agent workflows")
  .version(formatVersionOutputForArgv())
  .option("--json", "Output as JSON")
  .exitOverride();

registerInitCommand(program, deps);
registerStatusCommand(program, deps);
registerDoctorCommand(program, deps);
registerNoteCommand(program, deps);
registerSessionCommand(program, deps);
registerInstallCommand(program, deps);
registerUpdateCommand(program);
registerUninstallCommand(program);
registerProvidersCommand(program);
registerSkillsCommand(program);
registerMcpCommand(program);
registerMissionCommand(program, deps);
registerFeatureCommand(program, deps);
registerValidateCommand(program, deps);
registerMilestoneCommand(program, deps);
registerCheckpointCommand(program, deps);
registerMemoryCorrectCommand(program, deps);
registerMemoryRecallCommand(program, deps);
registerMemorySearchCommand(program, deps);
registerMemoryLearnCommand(program, deps);
registerMemoryCompileCommand(program, deps);
registerRatchetCheckCommand(program, deps);
registerRatchetPromoteCommand(program, deps);
registerMemoryStatsCommand(program, deps);
registerMemoryLintCommand(program, deps);
registerGraphLinkCommand(program, deps);
registerGraphContextCommand(program, deps);
registerHandoffCommand(program, deps);
registerTaskCommand(program, deps);
registerReplyCommand(program, deps);
registerPrincipleCommand(program, deps);
registerBundleCommand(program, deps);
registerEvidenceCommand(program, deps);
registerSpecCommand(program, deps);
registerContractL2Command(program, deps);
registerPolicyCommand(program, deps);
registerVerdictCommand(program, deps);

const planCmd = program
  .command("plan")
  .description("Plan-time checks for agent tasks");
registerPlanCheckCommand(planCmd, program, deps);

const ciCmd = program
  .command("ci")
  .description("CI integration — runs the verdict pipeline in CI mode");
registerCiVerifyCommand(ciCmd, program, deps);

registerReviewCommand(program, deps);
registerIntakeCommand(program, deps);
registerRecoverCommand(program, deps);
registerGcCommand(program, deps);
registerRalphCommand(program, deps);
registerStateCommand(program, deps);
registerWorktreeCommand(program, deps);
registerInspectCommand(program, deps);
registerSetupCommand(program, deps);

const mergeCmd = program
  .command("merge")
  .description("Merge controls — auto-merge eligibility and trigger");
registerMergeAutoCommand(mergeCmd, program, deps);

const deployCmd = program
  .command("deploy")
  .description("Deploy safety commands — rollback and gate controls");
registerDeployCommand(deployCmd, program, deps);

const runtimeCmd = program
  .command("runtime")
  .description("Runtime signal checks — query providers and record Evidence");
registerRuntimeCheckCommand(runtimeCmd, program, deps);

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

// mission-control is the only verb that pulls OpenTUI/React (~250ms cold).
// Skip its registration when argv targets a different verb. For `--help`,
// `--version`, and the bare `mission-control` invocation, register it so
// help text and the verb itself still work.
async function maybeRegisterMissionControl(argv: readonly string[]): Promise<void> {
  let needsMissionControl = true;
  for (let i = 2; i < argv.length; i++) {
    const token = argv[i];
    if (!token) continue;
    if (token === "--help" || token === "-h") { needsMissionControl = true; break; }
    if (token === "--version" || token === "-V") { needsMissionControl = false; break; }
    if (token.startsWith("-")) continue;
    needsMissionControl = token === "mission-control";
    break;
  }
  if (needsMissionControl) {
    const mod = await import("@/infra/commands/mission-control.command.js");
    mod.registerMissionControlCommand(program, deps);
  }
}

async function main(): Promise<void> {
  const refreshController = new AbortController();
  try {
    await cleanupStaleWindowsBinary();
    assertNoDeprecatedMissionControlFlags(process.argv);
    assertNoDeprecatedVersionFlag(process.argv);
    await maybeRegisterMissionControl(process.argv);
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
