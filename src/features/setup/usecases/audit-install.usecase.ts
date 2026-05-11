import { join } from "node:path";
import { readFile } from "node:fs/promises";
import { dirExists, fileExists, listFilesRecursive } from "@/shared/lib/fs.js";
import { detectHostRuntimes } from "./detect-host-runtime.usecase.js";
import { checkTocBudget, DEFAULT_TOC_BUDGET, type TocBudget } from "./enforce-toc-budget.usecase.js";
import { checkSkillBinaryParity, type SkillBinaryParityReport } from "./check-skill-binary-parity.usecase.js";

export interface AuditFinding {
  readonly code: string;
  readonly severity: "info" | "warn" | "error";
  readonly message: string;
}

export interface AuditInstallArgs {
  readonly projectRoot: string;
  readonly knownVerbs: ReadonlySet<string>;
  readonly tocBudget?: TocBudget;
}

export interface AuditInstallReport {
  readonly ok: boolean;
  readonly findings: readonly AuditFinding[];
  readonly hostRuntimes: readonly string[];
  readonly skillBinaryParity: SkillBinaryParityReport;
}

export async function auditInstall(args: AuditInstallArgs): Promise<AuditInstallReport> {
  const findings: AuditFinding[] = [];
  const tocBudget = args.tocBudget ?? DEFAULT_TOC_BUDGET;

  await checkAgentsMdSize(args.projectRoot, tocBudget, findings);
  await checkDocsPresence(args.projectRoot, findings);
  await checkOwnersYaml(args.projectRoot, findings);
  await checkOrphanRunDirs(args.projectRoot, findings);

  const hostRuntimes = await detectHostRuntimes(args.projectRoot);
  if (hostRuntimes.length === 0) {
    findings.push({
      code: "host-runtime-missing",
      severity: "info",
      message: "No host runtimes detected (.claude/.codex/.cursor); session hooks not installed",
    });
  }

  const skillBinaryParity = checkSkillBinaryParity({ knownVerbs: args.knownVerbs });
  for (const drift of skillBinaryParity.findings) {
    findings.push({
      code: "skill-binary-drift",
      severity: "warn",
      message: `Skill ${drift.skill} references "maestro ${drift.verb}" but binary does not expose it`,
    });
  }

  const ok = findings.every((f) => f.severity !== "error");

  return {
    ok,
    findings,
    hostRuntimes: hostRuntimes.map((r) => r.id),
    skillBinaryParity,
  };
}

async function checkAgentsMdSize(
  root: string,
  budget: TocBudget,
  findings: AuditFinding[],
): Promise<void> {
  const path = join(root, "AGENTS.md");
  if (!(await fileExists(path))) {
    findings.push({
      code: "agents-md-missing",
      severity: "warn",
      message: "Repo-root AGENTS.md is missing",
    });
    return;
  }
  const content = await readFile(path, "utf8");
  const report = checkTocBudget(content, budget);
  if (report.status === "exceeded") {
    findings.push({
      code: "agents-md-too-large",
      severity: "error",
      message: `AGENTS.md is ${report.lines} lines, exceeds hard limit ${budget.hardLimit}`,
    });
  } else if (report.status === "warn") {
    findings.push({
      code: "agents-md-large",
      severity: "warn",
      message: `AGENTS.md is ${report.lines} lines (warn at ${budget.warnLimit}, hard ${budget.hardLimit})`,
    });
  }
}

async function checkDocsPresence(root: string, findings: AuditFinding[]): Promise<void> {
  const required = ["docs/harness-positioning.md", "docs/schedule-recipes.md"];
  for (const rel of required) {
    if (!(await fileExists(join(root, rel)))) {
      findings.push({
        code: "doc-missing",
        severity: "warn",
        message: `Expected ${rel} missing`,
      });
    }
  }
}

async function checkOwnersYaml(root: string, findings: AuditFinding[]): Promise<void> {
  const path = join(root, ".maestro/policies/owners.yaml");
  if (!(await fileExists(path))) {
    findings.push({
      code: "owners-yaml-missing",
      severity: "info",
      message: ".maestro/policies/owners.yaml not present; some verbs need it",
    });
    return;
  }
  const content = await readFile(path, "utf8");
  const roles = ["policy_approver", "ratchet_approver", "sensitive_waiver", "deploy_approver"];
  for (const role of roles) {
    if (!content.includes(role)) {
      findings.push({
        code: "owners-role-missing",
        severity: "info",
        message: `owners.yaml has no ${role} entries`,
      });
    }
  }
}

async function checkOrphanRunDirs(root: string, findings: AuditFinding[]): Promise<void> {
  const runs = join(root, ".maestro/runs");
  if (!(await dirExists(runs))) return;
  try {
    const entries = await listFilesRecursive(runs);
    const old = entries.filter((e) => e.includes("/state.json"));
    if (old.length > 50) {
      findings.push({
        code: "run-dir-orphan",
        severity: "warn",
        message: `${old.length} run-state files under .maestro/runs/; consider gc`,
      });
    }
  } catch {}
}
