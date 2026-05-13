import type { Command } from "commander";
import { createInterface } from "node:readline/promises";
import { type Services } from "@/services.js";
import { initMaestro } from "../usecases/init.usecase.js";
import { output } from "@/shared/lib/output.js";

interface InitCommandDeps {
  readonly getServices: () => Pick<Services, "config">;
}

export function registerInitCommand(
  program: Command,
  deps: InitCommandDeps,
): void {
  program
    .command("init")
    .description("Initialize maestro in the current project or globally")
    .option("--global", "Initialize global config at ~/.maestro/")
    .option("--json", "Output as JSON")
    .action(async (opts): Promise<void> => {
      const services = deps.getServices();
      const isJson = opts.json ?? program.opts().json ?? false;
      const replacementPrompter = shouldPromptForReplacement(isJson)
        ? createReplacementPrompter()
        : undefined;

      try {
        const result = await initMaestro(services.config, {
          global: opts.global ?? false,
          dir: process.cwd(),
          confirmReplace: replacementPrompter?.confirmReplace,
        });

        output(isJson, result, (r) => {
          const lines = [
            `[ok] Initialized ${r.scope} ${r.bootstrapGenerated ? "bootstrap" : "config"}`,
            ...r.created.map((p) => `  --> ${p}`),
            ...r.skipped.map((p) => `  [skip] ${p}`),
            "",
            "Next: run `maestro install` to install the bundled agent skills into",
            "      ~/.claude/skills/ and ~/.codex/skills/ for Claude Code / Codex.",
          ];
          if (r.scope === "project" && r.created.length > 0) {
            lines.push(
              "",
              "Tip: commit the substrate maestro just wrote (`.claude/`, `.codex/`,",
              "     `.maestro/`, `.gitignore`) before locking your first contract.",
              "     Bundled `maestro:` skill paths are exempt from scope checks, but",
              "     committing them up-front keeps your task diff minimal.",
            );
          }
          return lines;
        });
      } finally {
        replacementPrompter?.close();
      }
    });
}

function shouldPromptForReplacement(isJson: boolean): boolean {
  return !isJson && Boolean(process.stdin.isTTY && process.stdout.isTTY);
}

function createReplacementPrompter(): {
  readonly confirmReplace: (path: string) => Promise<boolean>;
  readonly close: () => void;
} {
  let rl: ReturnType<typeof createInterface> | undefined;
  let defaultDecision: boolean | undefined;

  return {
    confirmReplace: async (path: string): Promise<boolean> => {
      if (defaultDecision !== undefined) {
        return defaultDecision;
      }

      rl ??= createInterface({
        input: process.stdin,
        output: process.stdout,
      });

      const answer = (await rl.question(
        `Replace existing file ${path}? [y]es/[n]o/[a]ll yes/[s]kip all: `,
      )).trim().toLowerCase();

      if (answer === "a") {
        defaultDecision = true;
        return true;
      }

      if (answer === "s") {
        defaultDecision = false;
        return false;
      }

      return answer === "y" || answer === "yes";
    },
    close: () => {
      rl?.close();
      rl = undefined;
    },
  };
}
