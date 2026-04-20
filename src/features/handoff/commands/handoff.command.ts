import type { Command } from "commander";
import { getServices } from "@/services.js";
import {
  DEFAULT_HANDOFF_MODELS,
  launchHandoff,
  type HandoffProvider,
} from "@/features/handoff";
import { MaestroError } from "@/shared/errors.js";
import { output, resolveJsonFlag } from "@/shared/lib/output.js";

export function registerHandoffCommand(program: Command): void {
  program
    .command("handoff <task>")
    .description("Launch a fresh Codex or Claude handoff with a self-contained markdown briefing")
    .option("--provider <provider>", "Provider (codex|claude)", "codex")
    .option("--model <model>", "Override the provider default model")
    .option("--worktree [slug]", "Create and use a sibling git worktree for the handoff")
    .option("--base <branch>", "Base branch to use with --worktree")
    .option("--name <title>", "Display name for the launched session")
    .option("--wait", "Wait for the external agent to finish before returning")
    .option("--json", "Output as JSON")
    .action(async (task: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const provider = parseProvider(opts.provider);
      const result = await launchHandoff({
        missionStore: services.missionStore,
        featureStore: services.featureStore,
        assertionStore: services.assertionStore,
        git: services.git,
        launchStore: services.launchStore,
        launchers: services.handoffLaunchers,
      }, {
        cwd: process.cwd(),
        task,
        provider,
        model: typeof opts.model === "string" ? opts.model : undefined,
        name: typeof opts.name === "string" ? opts.name : undefined,
        wait: Boolean(opts.wait),
        worktree: opts.worktree as string | boolean | undefined,
        baseBranch: typeof opts.base === "string" ? opts.base : undefined,
      });

      output(isJson, result.record, (record) => formatLaunchRecord(record, provider));
    });
}

function parseProvider(value: unknown): HandoffProvider {
  if (value === "codex" || value === "claude") {
    return value;
  }

  throw new MaestroError(`Invalid --provider '${String(value)}'`, [
    "Valid providers: codex, claude",
    `Defaults: codex=${DEFAULT_HANDOFF_MODELS.codex}, claude=${DEFAULT_HANDOFF_MODELS.claude}`,
  ]);
}

function formatLaunchRecord(
  record: {
    readonly id: string;
    readonly provider: HandoffProvider;
    readonly model: string;
    readonly status: string;
    readonly targetDir: string;
    readonly promptPath: string;
    readonly outputPath: string;
    readonly worktree?: { readonly branch: string; readonly baseBranch: string; readonly path: string };
    readonly pid?: number;
    readonly exitCode?: number;
  },
  provider: HandoffProvider,
): string[] {
  const lines = [
    `[ok] Handoff launched: ${record.id}`,
    `  Provider: ${provider}/${record.model}`,
    `  Status: ${record.status}`,
    `  Target: ${record.targetDir}`,
    `  Prompt: ${record.promptPath}`,
    `  Log: ${record.outputPath}`,
  ];

  if (record.worktree) {
    lines.push(`  Worktree: ${record.worktree.path} (${record.worktree.branch} from ${record.worktree.baseBranch})`);
  }

  if (record.pid !== undefined) {
    lines.push(`  PID: ${record.pid}`);
  }

  if (record.exitCode !== undefined) {
    lines.push(`  Exit code: ${record.exitCode}`);
  }

  return lines;
}
