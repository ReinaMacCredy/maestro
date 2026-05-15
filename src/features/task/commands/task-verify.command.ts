import type { Command } from "commander";
import { MaestroError } from "@/shared/errors.js";
import { resolveJsonFlag } from "@/shared/lib/output.js";
import { resolveDefaultBase, resolveHeadSha } from "@/shared/lib/git-base.js";
import { matchesAnyGlob } from "@/shared/lib/glob-match.js";
import { recordEvidence } from "@/features/evidence/index.js";
import type { TrustFinding } from "@/v2/types/trust.js";
import type { Contract } from "@/features/task/domain/contract/contract-types.js";
import {
  readCurrentContractWithBackfill,
  readDraftContract,
} from "@/features/task/usecases/read-current-contract-with-backfill.js";
import { type Services } from "@/services.js";
import {
  isArchitectureRuleId,
  type ArchitectureRuleId,
} from "@/shared/lib/arch-rules.js";
import type { LintViolationPayload } from "@/features/evidence";

interface TaskVerifyDeps {
  readonly getServices: () => Pick<
    Services,
    "contractVersionStore" | "contractStore" | "evidenceStore" | "gitAnchor" | "runTrustVerifier"
  >;
}

export function registerTaskVerifyCommand(
  taskCmd: Command,
  program: Command,
  deps: TaskVerifyDeps,
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
        const draft = await readDraftContract(services.contractStore, taskId);
        if (draft !== undefined) {
          // Drafted-but-unlocked is a real misconfiguration — keep error semantics.
          throw new MaestroError(
            `Contract ${draft.id} for task ${taskId} is in draft status — lock it first`,
            [`maestro task contract lock ${taskId}`],
          );
        }
        // No contract at all is the tiny-lane case (intake → patch directly).
        // Emit an advisory and exit 0 — verifier "skipped" is not a failure
        // condition for the calling agent. Agents under `set -e` (and plain
        // shell pipelines like `task verify && next-step`) were aborting on
        // the previous exit(2). Callers who *require* a contract should run
        // plan check / verdict request, both of which correctly fail when
        // no contract exists.
        const advisory = {
          warning: "no-contract",
          taskId,
          message: `No contract proposed for task ${taskId}; verifier skipped`,
          hint: "Run 'maestro task contract new <taskId>' if this task needs a contract; tiny-lane changes from `maestro intake` typically do not.",
        } as const;
        if (isJson) {
          process.stdout.write(JSON.stringify({ findings: [], counts: { error: 0, warn: 1, info: 0 }, advisory }) + "\n");
        } else {
          console.log(`Trust Verifier: skipped — ${advisory.message}`);
          console.log(`  ${advisory.hint}`);
        }
        process.exit(0);
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
      const [changedPaths, addedLines, untrackedFiles] = await Promise.all([
        services.gitAnchor.collectChangedPaths(cwd, baseRef, headSha),
        services.gitAnchor.collectAddedLines(cwd, baseRef, headSha),
        services.gitAnchor.collectUntrackedFiles(cwd),
      ]);

      // 5. Run trust verifier
      const result = await services.runTrustVerifier({
        contract,
        diff: { changedPaths, addedLines, base: baseRef, head: headSha },
      });

      // 6. Check untracked files against scope (R28 Obs 5 fix: warn users
      // before completion blocks on untracked out-of-scope files).
      const untrackedOutOfScope = untrackedFiles.filter(
        (path) => !matchesAnyGlob(contract.scope.filesExpected, path),
      );
      const allFindings: TrustFinding[] = [...result.findings];
      if (untrackedOutOfScope.length > 0) {
        allFindings.push({
          check: "untracked-out-of-scope",
          severity: "warn",
          paths: untrackedOutOfScope,
          details: `${untrackedOutOfScope.length} untracked file${untrackedOutOfScope.length !== 1 ? "s" : ""} in working tree not covered by scope.filesExpected — will block completion if not committed or removed`,
        });
      }

      // 7. Write one verifier-kind Evidence row per finding
      await Promise.all(
        allFindings.map((finding) =>
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

      // 7b. Mirror architecture-lint findings as `lint-violation` rows so
      // C-1's `task introspect` can list "open lints" without parsing
      // verifier-kind text. Mirrors witness level of the verifier rows above.
      await Promise.all(
        allFindings
          .filter((f) => isArchitectureRuleId(f.check))
          .map((finding) =>
            recordEvidence(services.evidenceStore, {
              task_id: taskId,
              kind: "lint-violation",
              witness_level: "agent-claimed-locally",
              payload: lintPayloadFromFinding(finding),
            }),
          ),
      );

      // 8. Print output
      const counts = countBySeverity(allFindings);

      if (isJson) {
        process.stdout.write(
          JSON.stringify({ findings: allFindings, counts }) + "\n",
        );
      } else {
        printTextFindings(allFindings, counts);
        if (counts.error > 0) {
          printRecoveryHints(allFindings, contract, taskId);
        }
      }

      // 9. Exit code
      const exitCode = deriveExitCode(counts);
      if (exitCode !== 0) {
        process.exit(exitCode);
      }
    });
}

// ─── helpers ─────────────────────────────────────────────────────────────────

function lintPayloadFromFinding(finding: TrustFinding): LintViolationPayload {
  const ruleId = finding.check as ArchitectureRuleId;
  const file = finding.paths[0] ?? "";
  // Trust Verifier flattens lint context into `details` as
  // "<message> — line <n> — > <snippet> — <remediation>". Recover the
  // pieces by splitting on the same separator.
  const parts = (finding.details ?? "").split(" — ");
  const message = parts[0] ?? finding.check;
  const remediation = parts[parts.length - 1] ?? "";
  let line: number | undefined;
  let snippet: string | undefined;
  for (const part of parts.slice(1, -1)) {
    const lineMatch = part.match(/^line\s+(\d+)$/);
    if (lineMatch && lineMatch[1]) {
      line = Number.parseInt(lineMatch[1], 10);
      continue;
    }
    if (part.startsWith("> ")) {
      snippet = part.slice(2);
    }
  }
  return {
    ruleId,
    file,
    ...(line !== undefined ? { line } : {}),
    ...(snippet !== undefined ? { snippet } : {}),
    message,
    remediation,
  };
}

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
  if (counts.warn > 0) return 2;
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

// Mirror the broken-contract recovery printer used by `task close`. Stdout-only
// so JSON consumers stay clean. Distinguishes forbidden-touched (revert only)
// from out-of-scope (revert OR amend) by reading the scope finding's `details`
// — the trust verifier emits two distinct strings for the two conditions.
function printRecoveryHints(
  findings: readonly TrustFinding[],
  contract: Contract,
  taskId: string,
): void {
  const forbidden: string[] = [];
  const outOfScope: string[] = [];
  for (const f of findings) {
    if (f.check !== "scope" || f.severity !== "error") continue;
    if (f.details?.includes("filesForbidden")) {
      forbidden.push(...f.paths);
    } else if (f.details?.includes("filesExpected")) {
      outOfScope.push(...f.paths);
    }
  }
  if (forbidden.length === 0 && outOfScope.length === 0) return;

  const lockCommit = contract.claimedAtCommit ?? "HEAD~1";
  console.log("");
  console.log("To fix forward:");
  if (outOfScope.length > 0) {
    console.log("  # EITHER revert each out-of-scope file:");
    for (const path of outOfScope) {
      console.log(`  #   git checkout ${lockCommit} -- ${path} 2>/dev/null || git rm -f ${path}`);
    }
    console.log("  # OR expand scope via amend (adds the path to the contract's filesExpected):");
    for (const path of outOfScope) {
      console.log(`  #   maestro contract amend --task ${taskId} --add-path ${path} --reason "<why>"`);
    }
  }
  if (forbidden.length > 0) {
    console.log("  # revert each forbidden file (cannot be amended; forbidden paths stay forbidden):");
    for (const path of forbidden) {
      console.log(`  #   git checkout ${lockCommit} -- ${path} 2>/dev/null || git rm -f ${path}`);
    }
  }
}
