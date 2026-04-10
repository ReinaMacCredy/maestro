/**
 * Task command handler.
 * Round one: registers the `task` parent command with an empty stub body.
 * Subcommands are added in subsequent commits.
 */
import type { Command } from "commander";

export function registerTaskCommand(program: Command): void {
  program
    .command("task")
    .description("Task lifecycle management (br-style issue graph)")
    .option("--json", "Output as JSON");
}
