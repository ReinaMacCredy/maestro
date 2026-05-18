import {
  checkArchitectureRules,
  type ArchitectureRuleId,
  type ArchitectureViolation,
} from "@/shared/lib/arch-rules.js";
import {
  principlesScan,
  type PrincipleScanFinding,
} from "@/service/principle-scan.usecase.js";
import { buildCoreServices } from "@/providers/build-services.js";
import type { PrinciplesStorePort } from "@/repo/principles-store.port.js";
import type { ProcessRunnerPort } from "@/repo/process-runner.port.js";

export interface SlopCleanupArgs {
  readonly projectRoot: string;
  readonly minSeverity?: "info" | "warn" | "error";
  readonly principlesStore?: PrinciplesStorePort;
  readonly processRunner?: ProcessRunnerPort;
}

export interface SlopFileGroup {
  readonly file: string;
  readonly violations: readonly ArchitectureViolation[];
  // ruleIds is widened from ArchitectureRuleId so principle slugs can appear
  // alongside arch-lint rule ids in the same group (PR 29 rewire).
  readonly ruleIds: readonly (ArchitectureRuleId | string)[];
}

export interface SlopCleanupResult {
  readonly totalViolations: number;
  readonly filesAffected: number;
  readonly bySeverity: Readonly<Record<"error" | "warn" | "info", number>>;
  readonly byRule: Readonly<Record<string, number>>;
  readonly groups: readonly SlopFileGroup[];
  readonly principleFindings: readonly PrincipleScanFinding[];
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

  const services = args.principlesStore && args.processRunner
    ? {
        principlesStore: args.principlesStore,
        processRunner: args.processRunner,
      }
    : (() => {
        const core = buildCoreServices({ repoRoot: args.projectRoot });
        return {
          principlesStore: core.principlesStore,
          processRunner: core.processRunner,
        };
      })();

  const principleReport = await principlesScan({
    principlesStore: services.principlesStore,
    processRunner: services.processRunner,
    repoRoot: args.projectRoot,
  });

  const fileMap = new Map<string, ArchitectureViolation[]>();
  const ruleIdsByFile = new Map<string, Set<string>>();
  for (const v of violations) {
    if (!v.file) continue;
    const list = fileMap.get(v.file) ?? [];
    list.push(v);
    fileMap.set(v.file, list);
    const ids = ruleIdsByFile.get(v.file) ?? new Set<string>();
    ids.add(v.ruleId);
    ruleIdsByFile.set(v.file, ids);
  }

  const bySeverity = { error: 0, warn: 0, info: 0 };
  const byRule: Record<string, number> = {};
  for (const v of violations) {
    bySeverity[v.severity]++;
    byRule[v.ruleId] = (byRule[v.ruleId] ?? 0) + 1;
  }

  for (const finding of principleReport.findings) {
    byRule[finding.principle_slug] = (byRule[finding.principle_slug] ?? 0) + 1;
    bySeverity.error++;
    if (finding.file) {
      const list = fileMap.get(finding.file) ?? [];
      fileMap.set(finding.file, list);
      const ids = ruleIdsByFile.get(finding.file) ?? new Set<string>();
      ids.add(finding.principle_slug);
      ruleIdsByFile.set(finding.file, ids);
    }
  }

  const groups: SlopFileGroup[] = [];
  for (const [file, list] of fileMap) {
    const ruleIds = Array.from(ruleIdsByFile.get(file) ?? new Set<string>()).sort();
    groups.push({ file, violations: list, ruleIds });
  }
  groups.sort((a, b) => b.violations.length - a.violations.length);

  return {
    totalViolations: violations.length + principleReport.findings.length,
    filesAffected: groups.length,
    bySeverity,
    byRule,
    groups,
    principleFindings: principleReport.findings,
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
  if (r.principleFindings.length > 0) {
    lines.push(
      `  principle findings: ${r.principleFindings.length} (${r.principleFindings.filter((f) => f.kind === "scan-error").length} scan-error)`,
    );
  }
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
