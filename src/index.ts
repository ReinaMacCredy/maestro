#!/usr/bin/env bun
import { rm } from "node:fs/promises";
import { basename } from "node:path";
import { Command, CommanderError } from "commander";
import { formatVersionOutputForArgv } from "@/shared/version-format.js";
import { MaestroError } from "@/shared/errors.js";
import { initServices } from "./services.js";
import { registerInitCommand } from "@/infra/commands/init.command.js";
import { registerStatusCommand } from "@/infra/commands/status.command.js";
import { registerDoctorCommand } from "@/infra/commands/doctor.command.js";
import { registerInstallCommand } from "@/infra/commands/install.command.js";
import { registerUpdateCommand } from "@/infra/commands/update.command.js";
import { registerUninstallCommand } from "@/infra/commands/uninstall.command.js";
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
    initServices(process.cwd());
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
  const executableName = basename(execPath.replaceAll("\\", "/")).toLowerCase();
  return platform === "win32" && executableName === "maestro.exe";
}

async function cleanupStaleWindowsBinary(): Promise<void> {
  if (!shouldCleanupStaleWindowsBinary()) return;
  await rm(`${process.execPath}.old`, { force: true }).catch(() => undefined);
}

async function main(): Promise<void> {
  try {
    await cleanupStaleWindowsBinary();
    assertNoDeprecatedMissionControlFlags(process.argv);
    await program.parseAsync(process.argv);
  } catch (err) {
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
