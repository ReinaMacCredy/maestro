import { join } from "node:path";
import { runCommand, type CommandResult } from "./command-runner.js";

const CLI = [
  "bun",
  "run",
  join(import.meta.dir, "..", "..", "src", "index.ts"),
];

export async function runCli(
  args: readonly string[],
  cwd: string,
): Promise<CommandResult> {
  return runCommand([...CLI, ...args], cwd);
}
