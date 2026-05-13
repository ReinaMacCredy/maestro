import { MaestroError } from "@/shared/errors.js";

export interface TocBudget {
  readonly hardLimit: number;
  readonly warnLimit: number;
}

export const DEFAULT_TOC_BUDGET: TocBudget = {
  hardLimit: 160,
  warnLimit: 140,
};

export interface TocBudgetReport {
  readonly lines: number;
  readonly status: "ok" | "warn" | "exceeded";
  readonly hardLimit: number;
  readonly warnLimit: number;
}

export function checkTocBudget(
  content: string,
  budget: TocBudget = DEFAULT_TOC_BUDGET,
): TocBudgetReport {
  const lines = countLines(content);
  let status: TocBudgetReport["status"] = "ok";
  if (lines > budget.hardLimit) status = "exceeded";
  else if (lines > budget.warnLimit) status = "warn";
  return {
    lines,
    status,
    hardLimit: budget.hardLimit,
    warnLimit: budget.warnLimit,
  };
}

export function assertTocBudget(
  filePath: string,
  content: string,
  budget: TocBudget = DEFAULT_TOC_BUDGET,
): TocBudgetReport {
  const report = checkTocBudget(content, budget);
  if (report.status === "exceeded") {
    throw new MaestroError(
      `${filePath} exceeds TOC size budget: ${report.lines} lines > ${budget.hardLimit}`,
      [
        `Trim ${filePath} to ≤${budget.hardLimit} lines`,
        "Move encyclopedia content into docs/ and leave a one-line pointer",
        "Lower the budget via .maestro/config.json field tocSizeBudget if intentional",
      ],
      "toc-budget-exceeded",
    );
  }
  return report;
}

function countLines(content: string): number {
  if (content.length === 0) return 0;
  const trimmed = content.endsWith("\n") ? content.slice(0, -1) : content;
  if (trimmed.length === 0) return 1;
  return trimmed.split("\n").length;
}
