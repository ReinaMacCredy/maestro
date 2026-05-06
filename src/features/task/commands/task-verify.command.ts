import type { Command } from "commander";
import { MaestroError } from "@/shared/errors.js";
import { resolveJsonFlag } from "@/shared/lib/output.js";
import { resolveDefaultBase, resolveHeadSha } from "@/shared/lib/git-base.js";
import { recordEvidence } from "@/features/evidence/index.js";
import type { TrustFinding } from "@/features/verify/domain/types.js";
import { readCurrentContractWithBackfill } from "@/features/task/usecases/read-current-contract-with-backfill.js";
import { getServices, type Services } from "@/services.js";

interface TaskVerifyDeps {
  readonly getServices: () => Pick<
    Services,
    "contractVersionStore" | "contractStore" | "evidenceStore" | "gitAnchor" | "runTrustVerifier"
  >;
}

export function registerTaskVerifyCommand(
  taskCmd: Command,
  program: Command,
  deps: TaskVerifyDeps = { getServices },
): void {
  taskCmd
    .command("verify")
    .description("Run the Trust Verifier locally against the current diff and print findings")
    .requiredOption("--task <id>", "Task id")
    .option("--base <ref>", "Base git ref for the diff (default: contract lock-commit; falls back to merge-base with main/master/upstream)")
    .option("--json", "Output as JSON")
    .action(async (opts): Promise<void> => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, program);
      const taskId: string = opts.task;

      // 1. Resolve current contract
      const contract = await readCurrentContractWithBackfill(
        services.contractVersionStore,
        services.contractStore,
        taskId,
      );
      if (contract === undefined) {
        throw new MaestroError(`No contract proposed for task ${taskId}`, [
          "Run 'maestro contract amend' or propose via maestro-plan skill",
        ]);
      }

      // Prefer the contract's lock-commit so brownfield repos don't pull
      // pre-existing files into the diff and trigger spurious scope errors.
      // Fall back to branch heuristics only when the contract has no anchor.
      const baseRef = typeof opts.base === "string" && opts.base.length > 0
        ? opts.base
        : (contract.claimedAtCommit ?? (await resolveDefaultBase()));
      const headSha = await resolveHeadSha();

      // 4. Build diff
      const cwd = process.cwd();
      const [changedPaths, addedLines] = await Promise.all([
        services.gitAnchor.collectChangedPaths(cwd, baseRef, headSha),
        services.gitAnchor.collectAddedLines(cwd, baseRef, headSha),
      ]);

      // 5. Run trust verifier
      const result = await services.runTrustVerifier({
        contract,
        diff: { changedPaths, addedLines, base: baseRef, head: headSha },
      });

      // 6. Write one verifier-kind Evidence row per finding
      await Promise.all(
        result.findings.map((finding) =>
          recordEvidence(services.evidenceStore, {
            task_id: taskId,
            kind: "verifier",
            witness_level: "agent-claimed-locally",
            payload: {
              check: finding.check,
              severity: finding.severity,
              paths: finding.paths,
              details: finding.details,
            },
          }),
        ),
      );

      // 7. Print output
      const counts = countBySeverity(result.findings);

      if (isJson) {
        process.stdout.write(
          JSON.stringify({ findings: result.findings, counts }) + "\n",
        );
      } else {
        printTextFindings(result.findings, counts);
      }

      // 8. Exit code
      const exitCode = deriveExitCode(counts);
      if (exitCode !== 0) {
        process.exit(exitCode);
      }
    });
}

// ─── helpers ─────────────────────────────────────────────────────────────────

interface FindingCounts {
  readonly error: number;
  readonly warn: number;
  readonly info: number;
}

function countBySeverity(findings: readonly TrustFinding[]): FindingCounts {
  let error = 0;
  let warn = 0;
  let info = 0;
  for (const f of findings) {
    if (f.severity === "error") error++;
    else if (f.severity === "warn") warn++;
    else info++;
  }
  return { error, warn, info };
}

function deriveExitCode(counts: FindingCounts): number {
  if (counts.error > 0) return 1;
  if (counts.warn > 0 || counts.info > 0) return 2;
  return 0;
}

function printTextFindings(findings: readonly TrustFinding[], counts: FindingCounts): void {
  const total = findings.length;
  if (total === 0) {
    console.log("Trust Verifier: no findings");
    return;
  }
  console.log(
    `Trust Verifier: ${total} finding${total !== 1 ? "s" : ""} (${counts.error} error${counts.error !== 1 ? "s" : ""}, ${counts.warn} warning${counts.warn !== 1 ? "s" : ""}, ${counts.info} info)`,
  );
  for (const finding of findings) {
    const pathsSuffix = finding.paths.length > 0 ? `: ${finding.paths.join(", ")}` : "";
    console.log(`  [${finding.severity}] ${finding.check}${pathsSuffix}`);
    if (finding.details) {
      console.log(`    ${finding.details}`);
    }
  }
}
