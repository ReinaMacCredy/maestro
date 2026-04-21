import { normalizeSlashes } from "@/shared/lib/path-normalize.js";
import type { TaskReceipt } from "../task-types.js";
import type { GitTouchedFilesResult } from "../../ports/git-anchor.port.js";
import type {
  Contract,
  ContractVerdict,
  DoneWhenCriterion,
} from "./contract-types.js";

export interface ComputedContractVerdict {
  readonly verdict: ContractVerdict;
  readonly criteria: readonly DoneWhenCriterion[];
}

export function computeContractVerdict(
  contract: Contract,
  gitResult: GitTouchedFilesResult,
  receipt: TaskReceipt | undefined,
  actorId: string,
  at: string,
  opts?: {
    readonly overlapDetected?: ContractVerdict["overlapDetected"];
  },
): ComputedContractVerdict {
  const criteria = applyReceiptHints(contract.doneWhen, receipt, actorId, at);
  const actualFilesTouched = gitResult.actualFilesTouched.map((path) => normalizeSlashes(path));
  const forbiddenTouched = actualFilesTouched.filter((path) => matchesAny(contract.scope.filesForbidden, path));
  const expectedFilesMatched = actualFilesTouched.filter((path) =>
    !matchesAny(contract.scope.filesForbidden, path) && matchesAny(contract.scope.filesExpected, path),
  );
  const outOfScopeFiles = actualFilesTouched.filter((path) =>
    !matchesAny(contract.scope.filesForbidden, path) && !matchesAny(contract.scope.filesExpected, path),
  );
  const filesExpectedUnused = contract.scope.filesExpected.filter((pattern) =>
    !actualFilesTouched.some((path) => matches(pattern, path)),
  );

  const cap = contract.scope.maxFilesTouched ?? contract.configSnapshot.defaultMaxFilesTouched;
  const capExceeded = cap !== undefined && actualFilesTouched.length > cap
    ? { cap, actual: actualFilesTouched.length }
    : undefined;

  const metCriteria = criteria.filter((criterion) => criterion.met === true);
  const unmetCriteria = criteria.filter((criterion) => criterion.met !== true);
  const anchorFailed = gitResult.gitAvailable && gitResult.anchorFallback === "lost";
  const overlapBlocks = opts?.overlapDetected?.policy === "fail";

  return {
    criteria,
    verdict: {
      fulfilled: !anchorFailed
        && forbiddenTouched.length === 0
        && outOfScopeFiles.length === 0
        && unmetCriteria.length === 0
        && capExceeded === undefined
        && !overlapBlocks,
      computedAt: at,
      actualFilesTouched,
      expectedFilesMatched,
      outOfScopeFiles,
      forbiddenTouched,
      filesExpectedUnused,
      ...(capExceeded ? { capExceeded } : {}),
      unmetCriteria,
      metCriteria,
      ...(opts?.overlapDetected ? { overlapDetected: opts.overlapDetected } : {}),
      ...(gitResult.anchorFallback ? { anchorFallback: gitResult.anchorFallback } : {}),
      ...(receipt
        ? {
            receiptLinked: {
              summary: receipt.summary,
              surprise: receipt.surprise,
              verifiedBy: receipt.verifiedBy,
            },
          }
        : {}),
      ...(gitResult.notes ? { notes: gitResult.notes } : {}),
    },
  };
}

function applyReceiptHints(
  criteria: readonly DoneWhenCriterion[],
  receipt: TaskReceipt | undefined,
  actorId: string,
  at: string,
): readonly DoneWhenCriterion[] {
  const verifiedBy = receipt?.verifiedBy ?? [];
  if (verifiedBy.length === 0) {
    return criteria;
  }

  return criteria.map((criterion) => {
    if (criterion.met === true) {
      return criterion;
    }

    const matchedVerifier = verifiedBy.find((value) => looselyMatches(criterion.text, value));
    if (!matchedVerifier) {
      return criterion;
    }

    return {
      ...criterion,
      met: true,
      metAt: at,
      metBy: actorId,
      metEvidence: `receipt.verifiedBy:${matchedVerifier}`,
    };
  });
}

function looselyMatches(left: string, right: string): boolean {
  const normalizedLeft = left.trim().toLowerCase();
  const normalizedRight = right.trim().toLowerCase();
  return normalizedLeft.includes(normalizedRight) || normalizedRight.includes(normalizedLeft);
}

function matchesAny(patterns: readonly string[], path: string): boolean {
  return patterns.some((pattern) => matches(pattern, path));
}

function matches(pattern: string, path: string): boolean {
  return new Bun.Glob(normalizeSlashes(pattern)).match(path);
}
