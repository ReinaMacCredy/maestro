import { join } from "node:path";
import type { Command } from "commander";
import { resolveJsonFlag } from "@/shared/lib/output.js";
import { readText } from "@/shared/lib/fs.js";
import { parseYaml } from "@/shared/lib/yaml.js";
import { execArgv } from "@/shared/lib/shell.js";
import { matchesAnyGlob } from "@/shared/lib/glob-match.js";
import { getServices, type Services } from "@/services.js";
import type { RiskClass } from "@/features/task/index.js";
import { maxRiskClass } from "@/features/risk/index.js";

interface PolicyCheckCommandDeps {
  readonly getServices: () => Pick<
    Services,
    | "contractVersionStore"
    | "getRiskPolicy"
    | "getAutopilotPolicy"
    | "getReleasePolicy"
    | "deriveRiskClassFromDiff"
    | "gitAnchor"
    | "projectRoot"
  >;
}

interface PolicyCheckResult {
  readonly taskId: string;
  readonly contractRiskClass: RiskClass;
  readonly derivedRiskClass: RiskClass;
  readonly effectiveRiskClass: RiskClass;
  readonly matchedRiskPolicyRow: { readonly signal: string; readonly description?: string } | null;
  readonly autoMergeAllowed: boolean;
  readonly requiredWitnessLevel: string;
  readonly releaseRules: {
    readonly requireSignedCommits: boolean;
    readonly requireProofMapComplete: boolean;
  };
  readonly sensitivePaths: {
    readonly globs: readonly string[];
    readonly matchedPaths: readonly string[];
  };
}

function printPolicyCheckResult(result: PolicyCheckResult): void {
  console.log(`Task:                ${result.taskId}`);
  console.log(`Contract risk class: ${result.contractRiskClass}`);
  console.log(`Derived risk class:  ${result.derivedRiskClass}`);
  console.log(`Effective risk:      ${result.effectiveRiskClass}`);
  console.log("");
  console.log("Risk policy row matched:");
  if (result.matchedRiskPolicyRow !== null) {
    console.log(`  signal: ${result.matchedRiskPolicyRow.signal}`);
    if (result.matchedRiskPolicyRow.description !== undefined) {
      console.log(`  description: ${result.matchedRiskPolicyRow.description}`);
    }
  } else {
    console.log("  (no row matched)");
  }
  console.log("");
  console.log("Autopilot policy:");
  console.log(`  auto-merge allowed:     ${result.autoMergeAllowed}`);
  console.log(`  required witness level: ${result.requiredWitnessLevel}`);
  console.log("");
  console.log("Release policy:");
  console.log(`  require signed commits:     ${result.releaseRules.requireSignedCommits}`);
  console.log(`  require proof map complete: ${result.releaseRules.requireProofMapComplete}`);
  if (result.sensitivePaths.globs.length > 0) {
    console.log("");
    console.log("Sensitive-paths policy:");
    console.log(`  globs: ${result.sensitivePaths.globs.join(", ")}`);
    if (result.sensitivePaths.matchedPaths.length > 0) {
      console.log(`  matched in diff: ${result.sensitivePaths.matchedPaths.join(", ")}`);
    } else {
      console.log("  matched in diff: (none)");
    }
  }
}

export function registerPolicyCheckCommand(
  policyCmd: Command,
  program: Command,
  deps: PolicyCheckCommandDeps = { getServices },
): void {
  policyCmd
    .command("check")
    .description("List which policy rules apply for the current task diff (read-only)")
    .requiredOption("--task <id>", "Task ID")
    .option("--base <ref>", "Base git ref for the diff (default: merge-base with main or upstream)")
    .option("--json", "Output as JSON")
    .action(async (opts): Promise<void> => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, program);
      const taskId: string = opts.task;

      const projectRoot = services.projectRoot;

      // Load contract
      const contract = await services.contractVersionStore.readCurrent(taskId);
      if (contract === undefined) {
        throw new Error(`No contract found for task ${taskId}. Run 'maestro contract amend' first.`);
      }

      // Resolve base ref
      const baseRef = typeof opts.base === "string" && opts.base.length > 0
        ? opts.base
        : await resolveDefaultBase();

      const headResult = await execArgv(["git", "rev-parse", "HEAD"]);
      const headSha = headResult.exitCode === 0 && headResult.stdout
        ? headResult.stdout
        : "HEAD";

      const cwd = process.cwd();

      // Collect diff paths
      const changedPaths = await services.gitAnchor.collectChangedPaths(cwd, baseRef, headSha);
      const addedLines = await services.gitAnchor.collectAddedLines(cwd, baseRef, headSha);

      // Load policies
      const [riskPolicy, autopilotPolicy, releasePolicy] = await Promise.all([
        services.getRiskPolicy(),
        services.getAutopilotPolicy(),
        services.getReleasePolicy(),
      ]);

      // Load sensitive-paths globs
      const sensitiveGlobs = await loadSensitivePathsGlobs(projectRoot);

      // Derive risk class
      const derivedRiskResult = services.deriveRiskClassFromDiff(
        {
          changedPaths,
          addedLines: addedLines.map((line) => ({ path: "", lines: [line] })),
          sensitivePathsPolicy: sensitiveGlobs,
        },
        riskPolicy,
      );

      const contractRiskClass: RiskClass = contract.riskClass ?? "medium";
      const effectiveRiskClass = maxRiskClass(contractRiskClass, derivedRiskResult.class);

      // Which sensitive paths match the diff?
      const matchedSensitivePaths = sensitiveGlobs.length > 0
        ? changedPaths.filter((p) => matchesAnyGlob(sensitiveGlobs, p))
        : [];

      const result: PolicyCheckResult = {
        taskId,
        contractRiskClass,
        derivedRiskClass: derivedRiskResult.class,
        effectiveRiskClass,
        matchedRiskPolicyRow: derivedRiskResult.matchedRow
          ? { signal: derivedRiskResult.matchedRow.signal, description: derivedRiskResult.matchedRow.description }
          : null,
        autoMergeAllowed: autopilotPolicy.autoMergeAllowed[effectiveRiskClass] ?? false,
        requiredWitnessLevel: autopilotPolicy.requiredWitnessLevel[effectiveRiskClass],
        releaseRules: {
          requireSignedCommits: releasePolicy.requireSignedCommits,
          requireProofMapComplete: releasePolicy.requireProofMapComplete,
        },
        sensitivePaths: {
          globs: sensitiveGlobs,
          matchedPaths: matchedSensitivePaths,
        },
      };

      if (isJson) {
        console.log(JSON.stringify(result, null, 2));
      } else {
        printPolicyCheckResult(result);
      }
    });
}

// ─── helpers ─────────────────────────────────────────────────────────────────

async function resolveDefaultBase(): Promise<string> {
  const upstream = await execArgv(["git", "rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]);
  if (upstream.exitCode === 0 && upstream.stdout) {
    const upstreamRef = upstream.stdout;
    const mergeBase = await execArgv(["git", "merge-base", "HEAD", upstreamRef]);
    if (mergeBase.exitCode === 0 && mergeBase.stdout) {
      return mergeBase.stdout;
    }
    return upstreamRef;
  }

  const mergeBaseMain = await execArgv(["git", "merge-base", "HEAD", "main"]);
  if (mergeBaseMain.exitCode === 0 && mergeBaseMain.stdout) {
    return mergeBaseMain.stdout;
  }

  return "main";
}

interface SensitivePathsYaml {
  readonly paths?: unknown;
}

async function loadSensitivePathsGlobs(projectRoot: string): Promise<readonly string[]> {
  const policyPath = join(projectRoot, ".maestro", "policies", "sensitive-paths.yaml");
  const raw = await readText(policyPath);
  if (raw === undefined) return [];
  try {
    const parsed = parseYaml<SensitivePathsYaml>(raw);
    return Array.isArray(parsed?.paths) ? parsed.paths as string[] : [];
  } catch {
    return [];
  }
}
