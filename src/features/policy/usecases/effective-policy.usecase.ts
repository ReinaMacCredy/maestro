/**
 * Effective-policy loaders that apply Rule 9: loosenings committed within the
 * past 30 days are "pending" and are NOT yet honored. The effective policy
 * reverts each pending loosening by substituting the old (pre-loosening) YAML
 * back into the loaded policy for the affected field.
 *
 * Reversal strategy: full-YAML. Each PendingLoosening carries the complete
 * YAML content that existed before the loosening commit. Rather than a
 * field-level patch, we reconstruct the "tighter" policy by loading the
 * oldYaml and applying only the tightening fields back onto the current policy.
 * This is heavier but obviously correct — no inversion logic to get wrong.
 */
import type { RiskPolicy, AutopilotPolicy, ReleasePolicy } from "../domain/policy-types.js";
import type { PendingLoosening } from "./detect-pending-loosenings.usecase.js";
import type { WitnessLevel } from "@/features/evidence/index.js";
import type { RiskClass } from "@/types/product-spec.js";
import { RISK_CLASS_ORDER } from "@/features/risk/index.js";

function revertRiskPolicyLoosening(
  policy: RiskPolicy,
  loosening: PendingLoosening,
): RiskPolicy {
  const { edit } = loosening;

  if (!edit.path) return policy;

  // "rows[signal]" — row was removed; re-add it
  const removedMatch = /^rows\[([^\]]+)\]$/.exec(edit.path);
  if (removedMatch) {
    const signal = removedMatch[1];
    const oldRow = edit.oldValue as { signal: string; derived_class: string; description?: string } | undefined;
    if (!oldRow) return policy;
    const alreadyPresent = policy.rows.some((r) => r.signal === signal);
    if (alreadyPresent) return policy;
    return {
      ...policy,
      rows: [
        ...policy.rows,
        {
          signal: oldRow.signal,
          derivedClass: oldRow.derived_class as RiskClass,
          ...(oldRow.description !== undefined ? { description: oldRow.description } : {}),
        },
      ],
    };
  }

  // "rows[signal].derived_class" — row's class was lowered; raise it back
  const classMatch = /^rows\[([^\]]+)\]\.derived_class$/.exec(edit.path);
  if (classMatch) {
    const signal = classMatch[1];
    const oldClass = edit.oldValue as string;
    return {
      ...policy,
      rows: policy.rows.map((r) =>
        r.signal === signal ? { ...r, derivedClass: oldClass as RiskClass } : r,
      ),
    };
  }

  return policy;
}

function revertAutopilotPolicyLoosening(
  policy: AutopilotPolicy,
  loosening: PendingLoosening,
): AutopilotPolicy {
  const { edit } = loosening;

  if (!edit.path) return policy;

  // "autoMergeAllowed.{cls}" — was false→true; revert to false
  const mergeMatch = /^autoMergeAllowed\.(\w+)$/.exec(edit.path);
  if (mergeMatch) {
    const cls = mergeMatch[1] as RiskClass;
    if (!(RISK_CLASS_ORDER as readonly string[]).includes(cls)) return policy;
    return {
      ...policy,
      autoMergeAllowed: {
        ...policy.autoMergeAllowed,
        [cls]: false,
      },
    };
  }

  // "requiredWitnessLevel.{cls}" — was lowered; raise back to oldValue
  const witnessMatch = /^requiredWitnessLevel\.(\w+)$/.exec(edit.path);
  if (witnessMatch) {
    const cls = witnessMatch[1] as RiskClass;
    if (!(RISK_CLASS_ORDER as readonly string[]).includes(cls)) return policy;
    const oldLevel = edit.oldValue as WitnessLevel;
    if (!oldLevel) return policy;
    return {
      ...policy,
      requiredWitnessLevel: {
        ...policy.requiredWitnessLevel,
        [cls]: oldLevel,
      },
    };
  }

  return policy;
}

function revertReleasePolicyLoosening(
  policy: ReleasePolicy,
  loosening: PendingLoosening,
): ReleasePolicy {
  const { edit } = loosening;

  if (!edit.path) return policy;

  // "requireSignedCommits" or "requireProofMapComplete" — was true→false; revert to true
  if (edit.path === "requireSignedCommits") {
    return { ...policy, requireSignedCommits: true };
  }
  if (edit.path === "requireProofMapComplete") {
    return { ...policy, requireProofMapComplete: true };
  }

  return policy;
}

function revertSensitivePathsLoosening(
  globs: readonly string[],
  loosening: PendingLoosening,
): readonly string[] {
  const { edit } = loosening;
  // "globs[<glob>]" — glob was removed; re-add it
  const removedMatch = edit.path ? /^globs\[(.+)\]$/.exec(edit.path) : null;
  if (!removedMatch || !removedMatch[1]) return globs;
  const glob = removedMatch[1];
  if (globs.includes(glob)) return globs;
  return [...globs, glob];
}

export function buildEffectivePolicyServices(args: {
  readonly projectRoot: string;
  readonly loadRiskPolicyImpl: (root: string) => Promise<RiskPolicy>;
  readonly loadAutopilotPolicyImpl: (root: string) => Promise<AutopilotPolicy>;
  readonly loadReleasePolicyImpl: (root: string) => Promise<ReleasePolicy>;
  readonly loadSensitivePathsGlobsImpl: (root: string) => Promise<readonly string[]>;
  readonly detectPendingLooseningsImpl: () => Promise<readonly PendingLoosening[]>;
}): {
  getEffectiveRiskPolicy: () => Promise<RiskPolicy>;
  getEffectiveAutopilotPolicy: () => Promise<AutopilotPolicy>;
  getEffectiveReleasePolicy: () => Promise<ReleasePolicy>;
  getEffectiveSensitivePathsGlobs: () => Promise<readonly string[]>;
} {
  const {
    projectRoot,
    loadRiskPolicyImpl,
    loadAutopilotPolicyImpl,
    loadReleasePolicyImpl,
    loadSensitivePathsGlobsImpl,
    detectPendingLooseningsImpl,
  } = args;

  // verdict request fetches all three effective policies in sequence and
  // each call walks the same `detectPendingLoosenings` path — which spawns
  // `git rev-parse HEAD` + reads the on-disk pending-loosenings cache.
  // Memoize per services-instance so the work happens once per CLI run.
  // services is built once per invocation, so this can't return stale data
  // mid-flight (the next `maestro` call gets a fresh services tree).
  let pendingPromise: Promise<readonly PendingLoosening[]> | undefined;
  const memoizedDetect = (): Promise<readonly PendingLoosening[]> => {
    pendingPromise ??= detectPendingLooseningsImpl();
    return pendingPromise;
  };

  return {
    async getEffectiveRiskPolicy(): Promise<RiskPolicy> {
      const [policy, pending] = await Promise.all([
        loadRiskPolicyImpl(projectRoot),
        memoizedDetect(),
      ]);
      const riskLoosenings = pending.filter((l) => l.kind === "risk");
      return riskLoosenings.reduce(
        (p, loosening) => revertRiskPolicyLoosening(p, loosening),
        policy,
      );
    },

    async getEffectiveAutopilotPolicy(): Promise<AutopilotPolicy> {
      const [policy, pending] = await Promise.all([
        loadAutopilotPolicyImpl(projectRoot),
        memoizedDetect(),
      ]);
      const autopilotLoosenings = pending.filter((l) => l.kind === "autopilot");
      return autopilotLoosenings.reduce(
        (p, loosening) => revertAutopilotPolicyLoosening(p, loosening),
        policy,
      );
    },

    async getEffectiveReleasePolicy(): Promise<ReleasePolicy> {
      const [policy, pending] = await Promise.all([
        loadReleasePolicyImpl(projectRoot),
        memoizedDetect(),
      ]);
      const releaseLoosenings = pending.filter((l) => l.kind === "release");
      return releaseLoosenings.reduce(
        (p, loosening) => revertReleasePolicyLoosening(p, loosening),
        policy,
      );
    },

    async getEffectiveSensitivePathsGlobs(): Promise<readonly string[]> {
      const [globs, pending] = await Promise.all([
        loadSensitivePathsGlobsImpl(projectRoot),
        memoizedDetect(),
      ]);
      const sensitiveLoosenings = pending.filter((l) => l.kind === "sensitive-paths");
      return sensitiveLoosenings.reduce(
        (gs, loosening) => revertSensitivePathsLoosening(gs, loosening),
        globs,
      );
    },
  };
}
