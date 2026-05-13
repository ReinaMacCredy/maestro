import type { Command } from "commander";
import { output, resolveJsonFlag } from "@/shared/lib/output.js";
import { type Services } from "@/services.js";
import { auditInstall, type AuditInstallReport } from "../usecases/audit-install.usecase.js";
import { runSetupSelfTest, type SelfTestReport } from "../usecases/run-self-test.usecase.js";
import { installRuntimeHooks, type RuntimeHookInstallResult } from "../usecases/install-runtime-hooks.usecase.js";

export interface SetupCommandDeps {
  readonly getServices: () => Pick<Services, "projectRoot">;
}

export function registerSetupCommand(program: Command, deps: SetupCommandDeps): void {
  const setup = program
    .command("setup")
    .description("Configure a repository as a long-running agent harness")
    .option("--check", "Audit-only mode; report drift and exit non-zero on errors")
    .option("--self-test", "Run an isolated setup self-test (tmpdir sandbox)")
    .option("--install-hooks", "Install host-runtime SessionStart/SessionEnd hooks")
    .option("--json", "Output as JSON")
    .action(async (opts): Promise<void> => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, program);

      if (opts.selfTest === true) {
        const report = await runSetupSelfTest();
        output(isJson, report, formatSelfTest);
        process.exit(report.ok ? 0 : 1);
        return;
      }

      if (opts.installHooks === true) {
        const results = await installRuntimeHooks(services.projectRoot);
        output(isJson, results, formatHookInstall);
        return;
      }

      const knownVerbs = collectKnownVerbs(program);
      const report = await auditInstall({
        projectRoot: services.projectRoot,
        knownVerbs,
      });
      output(isJson, report, formatAudit);
      const hasError = report.findings.some((f) => f.severity === "error");
      process.exit(hasError ? 1 : 0);
    });
}

function collectKnownVerbs(program: Command): ReadonlySet<string> {
  const verbs = new Set<string>();
  const walk = (cmd: Command, prefix: string): void => {
    for (const child of cmd.commands) {
      const name = child.name();
      const full = prefix ? `${prefix} ${name}` : name;
      verbs.add(name);
      verbs.add(full);
      walk(child, full);
    }
  };
  walk(program, "");
  for (const lazy of ["mission-control"]) {
    verbs.add(lazy);
  }
  return verbs;
}

function formatAudit(r: AuditInstallReport): readonly string[] {
  const lines: string[] = [];
  lines.push(`Setup audit — ${r.ok ? "OK" : "ISSUES"}`);
  lines.push(`  Host runtimes detected: ${r.hostRuntimes.join(", ") || "none"}`);
  lines.push(`  Skills checked: ${r.skillBinaryParity.skillsChecked} (${r.skillBinaryParity.findings.length} drift)`);
  if (r.findings.length === 0) {
    lines.push("  No findings");
  } else {
    for (const f of r.findings) {
      lines.push(`  [${f.severity}] ${f.code}: ${f.message}`);
    }
  }
  return lines;
}

function formatSelfTest(r: SelfTestReport): readonly string[] {
  const lines: string[] = [`Setup self-test — ${r.ok ? "OK" : "FAIL"}`];
  for (const s of r.steps) {
    lines.push(`  [${s.ok ? "ok" : "fail"}] ${s.name}${s.detail ? ` (${s.detail})` : ""}`);
  }
  return lines;
}

function formatHookInstall(results: readonly RuntimeHookInstallResult[]): readonly string[] {
  if (results.length === 0) {
    return [
      "No host runtimes detected (.claude/.codex/.cursor)",
      "  Hooks install into <runtime>/maestro-hooks.md inside the project.",
      "  Start a Claude Code, Codex, or Cursor session in this repo, or create",
      "  one of those directories first, then re-run 'maestro setup --install-hooks'.",
    ];
  }
  return results.map((r) => `  [${r.status}] ${r.runtime} -> ${r.file}`);
}
