import {
  checkArchitectureRules,
  type ArchitectureRuleId,
  type ArchitectureViolation,
} from "@/features/verify";

export interface SlopCleanupArgs {
  readonly projectRoot: string;
  readonly minSeverity?: "info" | "warn" | "error";
}

export interface SlopFileGroup {
  readonly file: string;
  readonly violations: readonly ArchitectureViolation[];
  readonly ruleIds: readonly ArchitectureRuleId[];
}

export interface SlopCleanupResult {
  readonly totalViolations: number;
  readonly filesAffected: number;
  readonly bySeverity: Readonly<Record<"error" | "warn" | "info", number>>;
  readonly byRule: Readonly<Record<string, number>>;
  readonly groups: readonly SlopFileGroup[];
}

const SEVERITY_RANK: Record<"info" | "warn" | "error", number> = {
  info: 0,
  warn: 1,
  error: 2,
};

export async function scanSlopCleanup(
  args: SlopCleanupArgs,
): Promise<SlopCleanupResult> {
  const minRank = SEVERITY_RANK[args.minSeverity ?? "info"];
  const violations = (
    await checkArchitectureRules({ repoRoot: args.projectRoot })
  ).filter((v) => SEVERITY_RANK[v.severity] >= minRank);

  const fileMap = new Map<string, ArchitectureViolation[]>();
  for (const v of violations) {
    if (!v.file) continue;
    const list = fileMap.get(v.file) ?? [];
    list.push(v);
    fileMap.set(v.file, list);
  }

  const groups: SlopFileGroup[] = [];
  for (const [file, list] of fileMap) {
    const ruleIds = Array.from(new Set(list.map((v) => v.ruleId))).sort();
    groups.push({ file, violations: list, ruleIds });
  }
  groups.sort((a, b) => b.violations.length - a.violations.length);

  const bySeverity = { error: 0, warn: 0, info: 0 };
  const byRule: Record<string, number> = {};
  for (const v of violations) {
    bySeverity[v.severity]++;
    byRule[v.ruleId] = (byRule[v.ruleId] ?? 0) + 1;
  }

  return {
    totalViolations: violations.length,
    filesAffected: groups.length,
    bySeverity,
    byRule,
    groups,
  };
}

export function formatSlopCleanupLines(r: SlopCleanupResult): string[] {
  const lines: string[] = [];
  lines.push(
    `Slop scan: ${r.totalViolations} violation${r.totalViolations !== 1 ? "s" : ""} across ${r.filesAffected} file${r.filesAffected !== 1 ? "s" : ""}`,
  );
  lines.push(
    `  by severity: ${r.bySeverity.error} error, ${r.bySeverity.warn} warn, ${r.bySeverity.info} info`,
  );
  if (Object.keys(r.byRule).length > 0) {
    lines.push("");
    lines.push("By rule:");
    const ruleLines = Object.entries(r.byRule)
      .sort((a, b) => b[1] - a[1])
      .map(([rule, count]) => `  ${rule}: ${count}`);
    lines.push(...ruleLines);
  }
  if (r.groups.length > 0) {
    lines.push("");
    lines.push("Top offenders:");
    for (const g of r.groups.slice(0, 10)) {
      lines.push(`  ${g.file} — ${g.violations.length} (${g.ruleIds.join(", ")})`);
    }
  }
  return lines;
}
