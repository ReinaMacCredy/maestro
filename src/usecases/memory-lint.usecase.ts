import type { CorrectionStorePort } from "../ports/correction-store.port.js";
import type { LearningStorePort } from "../ports/learning-store.port.js";
import type { RatchetStorePort } from "../ports/ratchet-store.port.js";

export interface LintWarning {
  readonly category: "correction" | "learning" | "ratchet";
  readonly message: string;
}

export interface LintResult {
  readonly warnings: readonly LintWarning[];
  readonly healthy: boolean;
}

export async function lintMemory(
  corrStore: CorrectionStorePort,
  learnStore: LearningStorePort,
  ratchetStore: RatchetStorePort,
  maxAgeDays: number = 7,
): Promise<LintResult> {
  const warnings: LintWarning[] = [];

  const corrections = await corrStore.list();

  // Check for corrections with no trigger keywords
  for (const c of corrections) {
    if (c.trigger.keywords.length === 0 && c.trigger.fileGlobs.length === 0) {
      warnings.push({
        category: "correction",
        message: `Correction "${c.rule}" (${c.id}) has no trigger keywords or globs`,
      });
    }
  }

  // Check for duplicate keywords across corrections
  const keywordMap = new Map<string, string[]>();
  for (const c of corrections) {
    for (const kw of c.trigger.keywords) {
      const lower = kw.toLowerCase();
      const existing = keywordMap.get(lower) ?? [];
      existing.push(c.id);
      keywordMap.set(lower, existing);
    }
  }
  for (const [kw, ids] of keywordMap) {
    if (ids.length > 1) {
      warnings.push({
        category: "correction",
        message: `Keyword "${kw}" appears in ${ids.length} corrections: ${ids.join(", ")}`,
      });
    }
  }

  // Check for stale learnings
  const compiled = await learnStore.readCompiled();
  const rawCount = await learnStore.rawCount();
  if (compiled) {
    const compiledDate = new Date(compiled.compiledAt);
    const daysSince = Math.floor((Date.now() - compiledDate.getTime()) / (1000 * 60 * 60 * 24));
    if (daysSince > maxAgeDays) {
      warnings.push({
        category: "learning",
        message: `Compiled learnings are ${daysSince} days old (threshold: ${maxAgeDays} days)`,
      });
    }
  }
  if (rawCount > 0 && !compiled) {
    warnings.push({
      category: "learning",
      message: `${rawCount} raw learning(s) have never been compiled`,
    });
  }

  // Check ratchet assertions referencing deleted corrections
  const suite = await ratchetStore.getSuite();
  const correctionIds = new Set(corrections.map((c) => c.id));
  for (const assertion of suite.assertions) {
    if (!correctionIds.has(assertion.correctionId)) {
      warnings.push({
        category: "ratchet",
        message: `Ratchet assertion "${assertion.id}" references deleted correction ${assertion.correctionId}`,
      });
    }
  }

  return { warnings, healthy: warnings.length === 0 };
}
