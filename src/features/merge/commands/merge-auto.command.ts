import type { Command } from "commander";
import { MaestroError } from "@/shared/errors.js";
import { resolveJsonFlag, stringifyForOutput } from "@/shared/lib/output.js";
import { resolveDefaultBase, resolveHeadSha } from "@/shared/lib/git-base.js";
import { loadSensitivePathsGlobs } from "@/features/policy/index.js";
import { readCurrentContractWithBackfill } from "@/service/contract-helpers.js";
import { type Services } from "@/services.js";
import { autoMergeEligible } from "../usecases/auto-merge-eligible.usecase.js";

interface MergeAutoCommandDeps {
  readonly getServices: () => Pick<
    Services,
    | "verdictStore"
    | "legacyEvidenceStore"
    | "contractVersionStore"
    | "contractStore"
    | "gitAnchor"
    | "getEffectiveAutopilotPolicy"
    | "trustSpecStore"
    | "githubApi"
    | "projectRoot"
  >;
}

export function registerMergeAutoCommand(
  parent: Command,
  program: Command,
  deps: MergeAutoCommandDeps,
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

      // Bind the verdict to the current HEAD tree SHA. Reusing the task's
      // latest verdict by name lets a stale PASS for older content trigger
      // auto-merge after a force-push or unrelated rebase. Mirror the
      // pattern from `verdict show --pr`: load all verdicts that match the
      // current tree, filter by task (and PR when the verdict was tagged
      // with one), and take the latest.
      //
      // Verdicts produced via `verdict request` without --pr have no
      // subject.pr; those are tree-bound but PR-agnostic and remain valid
      // for any PR that points at the same tree. Verdicts produced with a
      // specific PR (e.g. `ci verify` in a GitHub Actions context) MUST
      // match the requested PR.
      const cwd = process.cwd();
      const treeSha = await services.gitAnchor.resolveTreeSha(cwd);
      const treeMatches = await services.verdictStore.findByTreeSha(treeSha);
      const eligibleVerdicts = treeMatches
        .filter((v) => v.taskId === taskId && (v.subject?.pr === undefined || v.subject.pr === pr))
        .sort((a, b) => a.computedAt.localeCompare(b.computedAt));
      const verdict = eligibleVerdicts.at(-1);
      if (verdict === undefined) {
        throw new MaestroError(
          `No verdict found for task ${taskId} on PR ${pr} at tree ${treeSha}`,
          [
            "Run `maestro ci verify --task <id> --pr <n>` (or `maestro verdict request --task <id>` locally) on the current HEAD",
            "Verdicts are bound to (pr, tree_sha) — squashes preserve identity, force-push to a different tree invalidates them",
          ],
        );
      }

      // 2. Resolve contract
      const contract = await readCurrentContractWithBackfill(
        services.contractVersionStore,
        services.contractStore,
        taskId,
      );
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
      const changedPaths = await services.gitAnchor.collectChangedPaths(cwd, baseRef, headSha);

      // 4. Resolve all remaining inputs in parallel
      const [evidenceRows, sensitiveGlobs, autopilotPolicy, spec] = await Promise.all([
        services.legacyEvidenceStore.list({ task_id: taskId }),
        loadSensitivePathsGlobs(services.projectRoot),
        services.getEffectiveAutopilotPolicy(),
        contract.missionId !== undefined
          ? services.trustSpecStore.read(contract.missionId)
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
          stringifyForOutput({
            eligible: result.eligible,
            reasons: result.reasons,
            merged,
          }) + "\n",
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
