import type { Command } from "commander";
import { output, resolveJsonFlag } from "@/shared/lib/output.js";
import { type Services } from "@/services.js";
import {
  createWorktreeForTask,
  formatCreateWorktreeLines,
} from "../usecases/create-worktree.usecase.js";

interface WorktreeDeps {
  readonly getServices: () => Pick<Services, "git" | "projectRoot">;
}

export function registerWorktreeCommand(
  program: Command,
  deps: WorktreeDeps,
): void {
  const worktreeCmd = program
    .command("worktree")
    .description("Git worktree helpers (create isolated working trees)");

  worktreeCmd
    .command("create <slug>")
    .description("Create a git worktree with isolated .maestro/runs/")
    .option("--base <branch>", "Base branch (default: main)")
    .option("--prefix <prefix>", "Branch prefix (default: feat)")
    .option("--json", "Output as JSON")
    .action(async (slug: string, opts): Promise<void> => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, program);
      const result = await createWorktreeForTask(
        { git: services.git },
        {
          cwd: services.projectRoot,
          slug,
          ...(typeof opts.base === "string" ? { baseBranch: opts.base } : {}),
          ...(typeof opts.prefix === "string" ? { branchPrefix: opts.prefix } : {}),
        },
      );
      output(isJson, result, formatCreateWorktreeLines);
    });
}
