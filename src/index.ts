#!/usr/bin/env bun
import { Command, CommanderError } from "commander";
import { formatVersionOutputForArgv } from "@/shared/version-format.js";
import { MaestroError } from "@/shared/errors.js";
import { initServices } from "./services.js";
import { registerInitCommand } from "@/infra/commands/init.command.js";
import { registerStatusCommand } from "@/infra/commands/status.command.js";
import { registerDoctorCommand } from "@/infra/commands/doctor.command.js";
import { registerNoteCommand } from "./features/notes/commands/note.command.js";
import { registerInstallCommand } from "@/infra/commands/install.command.js";
import { registerUpdateCommand } from "@/infra/commands/update.command.js";
import { registerUninstallCommand } from "@/infra/commands/uninstall.command.js";
import { registerSessionCommand } from "./features/session/commands/session.command.js";
import {
  registerMissionCommand,
  registerFeatureCommand,
  registerValidateCommand,
  registerMilestoneCommand,
  registerCheckpointCommand,
} from "./features/mission/index.js";
import { registerMissionControlCommand } from "@/infra/commands/mission-control.command.js";
import { registerMemoryCorrectCommand } from "./features/memory/commands/memory-correct.command.js";
import { registerMemoryRecallCommand } from "./features/memory/commands/memory-recall.command.js";
import { registerMemorySearchCommand } from "./features/memory/commands/memory-search.command.js";
import { registerMemoryLearnCommand } from "./features/memory/commands/memory-learn.command.js";
import { registerMemoryCompileCommand } from "./features/memory/commands/memory-compile.command.js";
import { registerRatchetCheckCommand } from "./features/ratchet/commands/ratchet-check.command.js";
import { registerRatchetPromoteCommand } from "./features/ratchet/commands/ratchet-promote.command.js";
import { registerMemoryStatsCommand } from "./features/memory/commands/memory-stats.command.js";
import { registerMemoryLintCommand } from "./features/memory/commands/memory-lint.command.js";
import { registerGraphLinkCommand } from "./features/graph/commands/graph-link.command.js";
import { registerGraphContextCommand } from "./features/graph/commands/graph-context.command.js";
import { registerHandoffCommand } from "./features/handoff/commands/handoff.command.js";

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

async function main(): Promise<void> {
  try {
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

main();
