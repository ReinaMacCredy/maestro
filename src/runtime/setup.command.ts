import { Command } from "commander";
import { createInterface } from "node:readline/promises";
import { setupCheck, type SetupCheckEntry } from "../service/setup-check.usecase.js";
import { runSetup, type SetupReport } from "../service/setup.usecase.js";
import type { Services } from "@/services.js";

export interface SetupCommandOptions {
  readonly resolveRepoRoot: () => string;
  readonly getServices: () => Pick<Services, "config">;
}

function findOrCreateSetupCommand(program: Command): Command {
  const existing = program.commands.find((c) => c.name() === "setup");
  if (existing) return existing;
  return program.command("setup").description("Initialize the .maestro/ layout");
}

function formatEntry(entry: SetupCheckEntry): string {
  const marker =
    entry.status === "ok" ? "[ok]   " : entry.status === "warn" ? "[warn] " : "[miss] ";
  const detail = entry.detail ? ` — ${entry.detail}` : "";
  return `${marker}${entry.path}${detail}`;
}

const STATUS_MARKER: Record<string, string> = {
  ok: "[ok]",
  changed: "[chg]",
  skipped: "[skp]",
  error: "[err]",
};

export function formatReport(report: SetupReport): string[] {
  const lines: string[] = [];
  const prefix = report.dryRun ? "(dry-run) " : "";
  for (const step of report.steps) {
    const marker = STATUS_MARKER[step.status] ?? "[?]";
    lines.push(`${prefix}${marker} ${step.id}: ${step.label}${step.detail ? ` — ${step.detail}` : ""}`);
    for (const entry of step.paths) {
      lines.push(`  ${entry.action.padEnd(16)} ${entry.path}${entry.detail ? ` (${entry.detail})` : ""}`);
    }
  }
  lines.push(report.ok ? `${prefix}setup: OK` : `${prefix}setup: errors`);
  return lines;
}

export interface SetupFlags {
  json?: boolean;
  dryRun?: boolean;
  global?: boolean;
  resyncSkills?: boolean;
  resetTemplates?: boolean;
  // Commander parses `--no-git-ok` into `gitOk: false`; default is undefined.
  gitOk?: boolean;
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
      if (defaultDecision !== undefined) return defaultDecision;
      rl ??= createInterface({ input: process.stdin, output: process.stdout });
      const answer = (
        await rl.question(`Replace existing file ${path}? [y]es/[n]o/[a]ll yes/[s]kip all: `)
      )
        .trim()
        .toLowerCase();
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

export async function runSetupCommand(
  flags: SetupFlags,
  deps: SetupCommandOptions,
): Promise<SetupReport> {
  const services = deps.getServices();
  const isJson = flags.json === true;
  const prompter = shouldPromptForReplacement(isJson) ? createReplacementPrompter() : undefined;

  try {
    const report = await runSetup({
      // Walk to the canonical project root (linked-worktree-aware, gitfallback,
      // existing-.maestro-aware) instead of writing to wherever the agent's cwd
      // happens to be. Without this, running `maestro setup` from a subdir of a
      // git repo would scatter .maestro/ into the subdir.
      dir: deps.resolveRepoRoot(),
      global: flags.global === true,
      config: services.config,
      dryRun: flags.dryRun === true,
      resyncSkills: flags.resyncSkills === true,
      resetTemplates: flags.resetTemplates === true,
      noGitOk: flags.gitOk === false,
      confirmReplace: prompter?.confirmReplace,
    });
    return report;
  } finally {
    prompter?.close();
  }
}

export function registerSetupCommands(program: Command, opts: SetupCommandOptions): void {
  const setup = findOrCreateSetupCommand(program);

  setup
    .option("--global", "Initialize global config at ~/.maestro/")
    .option("--dry-run", "Show what would change without writing")
    .option("--resync-skills", "Reconcile .claude/skills and .codex/skills with shipped templates")
    .option("--reset-templates", "Replace customized bootstrap templates")
    .option("--no-git-ok", "Allow setup outside a git working tree")
    .option("--json", "Emit JSON instead of text")
    .action(async function (this: Command, flags: SetupFlags): Promise<void> {
      try {
        const report = await runSetupCommand(flags, opts);
        if (flags.json === true || this.optsWithGlobals().json === true) {
          console.log(JSON.stringify(report, null, 2));
        } else {
          for (const line of formatReport(report)) console.log(line);
        }
        if (!report.ok) process.exitCode = 1;
      } catch (err) {
        console.error(`maestro setup: ${(err as Error).message}`);
        process.exitCode = 1;
      }
    });

  setup
    .command("check")
    .description("Audit the .maestro/ directory layout (.maestro/{tasks,missions,evidence,runs}, docs/principles)")
    .option("--json", "emit JSON instead of text")
    .action(async function (this: Command, flags: { json?: boolean }): Promise<void> {
      try {
        const repoRoot = opts.resolveRepoRoot();
        const report = await setupCheck({ repoRoot });
        const wantJson = flags.json === true || this.optsWithGlobals().json === true;
        if (wantJson) {
          console.log(JSON.stringify({ ...report, project_root: repoRoot }, null, 2));
        } else {
          console.log(`project root: ${repoRoot}`);
          for (const entry of report.entries) console.log(formatEntry(entry));
          console.log(report.ok ? "setup check: OK" : "setup check: action required");
        }
        if (!report.ok) process.exitCode = 1;
      } catch (err) {
        console.error(`maestro setup check: ${(err as Error).message}`);
        process.exitCode = 1;
      }
    });
}
