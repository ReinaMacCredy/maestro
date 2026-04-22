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
  const storedActualFilesTouched = gitResult.actualFilesTouchedTruncated
    ? actualFilesTouched.slice(0, gitResult.actualFilesTouchedTruncated.stored)
    : actualFilesTouched;
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
  const amendmentScopeNote = buildAmendmentScopeNote(contract, outOfScopeFiles);
  const notes = [gitResult.notes, amendmentScopeNote]
    .filter((note): note is string => typeof note === "string" && note.trim().length > 0)
    .join(" ");

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
      actualFilesTouched: storedActualFilesTouched,
      ...(gitResult.actualFilesTouchedTruncated ? { actualFilesTouchedTruncated: gitResult.actualFilesTouchedTruncated } : {}),
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
      ...(notes ? { notes } : {}),
    },
  };
}

function buildAmendmentScopeNote(contract: Contract, outOfScopeFiles: readonly string[]): string | undefined {
  if (outOfScopeFiles.length === 0 || contract.amendments.length === 0) {
    return undefined;
  }

  const previouslyInScope = Array.from(new Set(outOfScopeFiles.filter((path) =>
    contract.amendments.some((amendment) =>
      matchesScope(amendment.before.scope, path) || matchesScope(amendment.after.scope, path),
    ),
  ))).sort();
  if (previouslyInScope.length === 0) {
    return undefined;
  }

  return `Previously in scope under amendments: ${previouslyInScope.join(", ")}.`;
}

function applyReceiptHints(
  criteria: readonly DoneWhenCriterion[],
  receipt: TaskReceipt | undefined,
  actorId: string,
  at: string,
): readonly DoneWhenCriterion[] {
  const verifiedBy = receipt?.verifiedBy
    ?.map((value) => ({
      raw: value,
      normalized: normalizeReceiptText(value),
    }))
    .filter((value) => value.normalized.length > 0) ?? [];
  if (verifiedBy.length === 0) {
    return criteria;
  }

  return criteria.map((criterion) => {
    if (criterion.met === true || criterion.kind !== "receipt-hint") {
      return criterion;
    }

    const matchedVerifier = verifiedBy.find((value) => looselyMatches(criterion.text, value.normalized));
    if (!matchedVerifier) {
      return criterion;
    }

    return {
      ...criterion,
      met: true,
      metAt: at,
      metBy: actorId,
      metEvidence: `receipt.verifiedBy:${matchedVerifier.raw}`,
    };
  });
}

function looselyMatches(left: string, right: string): boolean {
  const normalizedLeft = normalizeReceiptText(left);
  const normalizedRight = normalizeReceiptText(right);
  if (normalizedLeft.length === 0 || normalizedRight.length === 0) {
    return false;
  }
  if (normalizedLeft === normalizedRight) {
    return true;
  }
  if (normalizedLeft.length < 3) {
    return false;
  }
  return normalizedRight.includes(normalizedLeft);
}

function normalizeReceiptText(value: string): string {
  return value
    .trim()
    .toLowerCase()
    .replace(/\s+/g, " ");
}

function matchesAny(patterns: readonly string[], path: string): boolean {
  return patterns.some((pattern) => matches(pattern, path));
}

function matchesScope(
  scope: Contract["scope"] | undefined,
  path: string,
): boolean {
  if (!scope) {
    return false;
  }
  if (matchesAny(scope.filesForbidden, path)) {
    return false;
  }
  return matchesAny(scope.filesExpected, path);
}

// Verdict computation matches every (pattern × path) pair, so caching the
// compiled Glob avoids reallocating on every cross-product lookup.
const globCache = new Map<string, Bun.Glob>();

function matches(pattern: string, path: string): boolean {
  const normalized = normalizeSlashes(pattern);
  let glob = globCache.get(normalized);
  if (!glob) {
    glob = new Bun.Glob(normalized);
    globCache.set(normalized, glob);
  }
  return glob.match(path);
}
