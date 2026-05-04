import type { Command } from "commander";
import { MaestroError } from "@/shared/errors.js";
import { resolveJsonFlag } from "@/shared/lib/output.js";
import { resolveDefaultBase, resolveHeadSha } from "@/shared/lib/git-base.js";
import { loadSensitivePathsGlobs } from "@/features/policy/index.js";
import { getServices, type Services } from "@/services.js";
import { autoMergeEligible } from "../usecases/auto-merge-eligible.usecase.js";

interface MergeAutoCommandDeps {
  readonly getServices: () => Pick<
    Services,
    | "verdictStore"
    | "evidenceStore"
    | "contractVersionStore"
    | "gitAnchor"
    | "getEffectiveAutopilotPolicy"
    | "specStore"
    | "githubApi"
    | "projectRoot"
  >;
}

export function registerMergeAutoCommand(
  parent: Command,
  program: Command,
  deps: MergeAutoCommandDeps = { getServices },
): void {
  parent
    .command("auto")
    .description("Check auto-merge eligibility and trigger if eligible")
    .requiredOption("--pr <number>", "PR number", parseInt)
    .option("--task <id>", "Task ID")
    .option("--base <ref>", "Base git ref for the diff")
    .option("--repo <owner/name>", "GitHub repository (owner/name)")
    .option("--json", "Output as JSON")
    .action(async (opts): Promise<void> => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, program);
      const pr: number = opts.pr;
      const taskId: string | undefined = opts.task;
      const repoOpt: string | undefined = typeof opts.repo === "string" ? opts.repo : undefined;

      if (typeof pr !== "number" || isNaN(pr)) {
        throw new MaestroError("--pr must be a valid PR number", [
          "Example: maestro merge auto --pr 42",
        ]);
      }

      // 1. Resolve verdict — requires taskId
      if (!taskId) {
        throw new MaestroError("--task is required for merge auto", [
          "Example: maestro merge auto --pr 42 --task tsk-aaaaaa",
        ]);
      }

      const verdict = await services.verdictStore.readLatest(taskId);
      if (verdict === undefined) {
        throw new MaestroError(
          `No verdict found for task ${taskId}`,
          ["Run `maestro verdict request --task <id>` first"],
        );
      }

      // 2. Resolve contract
      const contract = await services.contractVersionStore.readCurrent(taskId);
      if (contract === undefined) {
        throw new MaestroError(
          `No contract found for task ${taskId}`,
          ["Run `maestro contract amend --task <id>` first"],
        );
      }

      // 3. Resolve diff (changed paths)
      const baseRef = typeof opts.base === "string" && opts.base.length > 0
        ? opts.base
        : await resolveDefaultBase();
      const headSha = await resolveHeadSha();
      const cwd = process.cwd();
      const changedPaths = await services.gitAnchor.collectChangedPaths(cwd, baseRef, headSha);

      // 4. Resolve all remaining inputs in parallel
      const [evidenceRows, sensitiveGlobs, autopilotPolicy, spec] = await Promise.all([
        services.evidenceStore.list({ task_id: taskId }),
        loadSensitivePathsGlobs(services.projectRoot),
        services.getEffectiveAutopilotPolicy(),
        contract.missionId !== undefined
          ? services.specStore.read(contract.missionId)
          : Promise.resolve(undefined),
      ]);

      // 5. Run eligibility gate
      const result = autoMergeEligible({
        verdict,
        evidenceRows,
        changedPaths,
        sensitiveGlobs,
        contract,
        autopilotPolicy,
        spec,
      });

      let merged = false;

      if (result.eligible) {
        // 6. Trigger auto-merge via the GitHub API port
        const repository = repoOpt ?? process.env.GITHUB_REPOSITORY;
        if (!repository) {
          throw new MaestroError(
            "Cannot determine repository for auto-merge",
            [
              "Pass --repo owner/name or set GITHUB_REPOSITORY",
            ],
          );
        }
        await services.githubApi.triggerAutoMerge({ repository, pr });
        merged = true;
      }

      if (isJson) {
        process.stdout.write(
          JSON.stringify(
            {
              eligible: result.eligible,
              reasons: result.reasons,
              merged,
            },
            null,
            2,
          ) + "\n",
        );
      } else {
        if (result.eligible) {
          console.log(`[ok] Auto-merge triggered for PR #${pr}.`);
        } else {
          console.log(`[!] PR #${pr} is not eligible for auto-merge:`);
          for (const reason of result.reasons) {
            console.log(`  [${reason.code}] ${reason.message}`);
          }
        }
      }

      if (!result.eligible) {
        process.exit(1);
      }
    });
}
