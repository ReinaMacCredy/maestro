import type { Command } from "commander";
import { resolveJsonFlag } from "@/shared/lib/output.js";
import { MaestroError } from "@/shared/errors.js";
import { getServices, type Services } from "@/services.js";
import type { Verdict } from "../domain/types.js";
import { exitCodeForDecision, printVerdict } from "../presentation.js";
import { requestVerdict } from "../usecases/request-verdict.usecase.js";

interface VerdictCommandDeps {
  readonly getServices: () => Pick<
    Services,
    | "verdictStore"
    | "contractVersionStore"
    | "runStateStore"
    | "evidenceStore"
    | "getEffectiveRiskPolicy"
    | "getEffectiveAutopilotPolicy"
    | "getEffectiveReleasePolicy"
    | "computeRisk"
    | "deriveRiskClassFromDiff"
    | "runTrustVerifier"
    | "gitAnchor"
    | "projectRoot"
  >;
}

export function registerVerdictCommand(
  program: Command,
  deps: VerdictCommandDeps = { getServices },
): void {
  const verdictCmd = program
    .command("verdict")
    .description("Show or request a Verdict for a task");

  verdictCmd
    .command("show")
    .description("Show the current verdict for a task")
    .requiredOption("--task <id>", "Task ID")
    .option("--version <verdictId>", "Show a specific verdict by ID (default: latest)")
    .option("--latest", "Show the latest verdict (default)")
    // --pr is a query-time filter: when provided alongside --task, the latest
    // verdict for that task is filtered by tree SHA + PR number. The tree SHA
    // is resolved from the current HEAD, so this reflects the current worktree
    // content. Requires --task to scope the search.
    .option("--pr <number>", "Filter by PR number (finds verdict by current HEAD tree SHA)", parseInt)
    .option("--json", "Output as JSON")
    .action(async (opts): Promise<void> => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, program);
      const taskId: string = opts.task;

      // --pr: resolve current tree SHA and find matching verdicts
      if (typeof opts.pr === "number") {
        const treeSha = await services.gitAnchor.resolveTreeSha(process.cwd());
        const matches = await services.verdictStore.findByTreeSha(treeSha);
        const filtered = matches.filter(
          (v) => v.subject?.pr === opts.pr && v.taskId === taskId,
        );
        if (filtered.length === 0) {
          console.log(`No verdict found for PR ${opts.pr} at tree SHA ${treeSha}`);
          return;
        }
        // Return the latest match (highest computedAt)
        const verdict = filtered[filtered.length - 1];
        if (isJson) {
          console.log(JSON.stringify(verdict, null, 2));
        } else {
          printVerdict(verdict!);
        }
        return;
      }

      let verdict: Verdict | undefined;

      if (typeof opts.version === "string" && opts.version.length > 0) {
        verdict = await services.verdictStore.readVersion(taskId, opts.version);
        if (verdict === undefined) {
          throw new MaestroError(`Verdict ${opts.version} not found for task ${taskId}`, [
            "Run 'maestro verdict show --task <id>' (without --version) to see the latest",
          ]);
        }
      } else {
        verdict = await services.verdictStore.readLatest(taskId);
        if (verdict === undefined) {
          console.log("No verdict yet. Run 'maestro verdict request --task <id>' to generate one.");
          return;
        }
      }

      if (isJson) {
        console.log(JSON.stringify(verdict, null, 2));
      } else {
        printVerdict(verdict);
      }
    });

  verdictCmd
    .command("request")
    .description("Compute a new Verdict for a task and persist it")
    .requiredOption("--task <id>", "Task ID")
    .option("--base <ref>", "Base git ref for the diff (default: merge-base with main or upstream)")
    .option("--json", "Output as JSON")
    .action(async (opts): Promise<void> => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, program);
      const taskId: string = opts.task;

      const verdict = await requestVerdict(
        { taskId, base: typeof opts.base === "string" ? opts.base : undefined },
        {
          contractVersionStore: services.contractVersionStore,
          runStateStore: services.runStateStore,
          evidenceStore: services.evidenceStore,
          verdictStore: services.verdictStore,
          getEffectiveRiskPolicy: services.getEffectiveRiskPolicy,
          getEffectiveAutopilotPolicy: services.getEffectiveAutopilotPolicy,
          getEffectiveReleasePolicy: services.getEffectiveReleasePolicy,
          riskServices: {
            computeRisk: services.computeRisk,
            deriveRiskClassFromDiff: services.deriveRiskClassFromDiff,
          },
          runTrustVerifier: services.runTrustVerifier,
          gitAnchor: services.gitAnchor,
          projectRoot: services.projectRoot,
        },
      );

      if (isJson) {
        console.log(JSON.stringify(verdict, null, 2));
      } else {
        printVerdict(verdict);
      }

      const exitCode = exitCodeForDecision(verdict.decision);
      if (exitCode !== 0) {
        process.exit(exitCode);
      }
    });
}
