#!/usr/bin/env bun
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import { runArchitectureLints } from "@/v2/service/architecture-lint.usecase.js";
import { YamlArchitectureRules } from "@/v2/repo/yaml-architecture-rules.adapter.js";

interface ParsedArgs {
  readonly json: boolean;
  readonly repoRoot: string;
  readonly rulesPath?: string;
}

function parseArgs(argv: readonly string[]): ParsedArgs {
  let json = false;
  let repoRoot: string | undefined;
  let rulesPath: string | undefined;
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === "--json") {
      json = true;
    } else if (arg === "--repo-root" && argv[i + 1]) {
      repoRoot = argv[++i];
    } else if (arg === "--rules" && argv[i + 1]) {
      rulesPath = argv[++i];
    } else if (arg === "-h" || arg === "--help") {
      printHelp();
      process.exit(0);
    } else if (arg && arg.startsWith("-")) {
      console.error(`Unknown flag: ${arg}`);
      printHelp();
      process.exit(2);
    }
  }
  if (!repoRoot) {
    repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
  }
  return rulesPath !== undefined ? { json, repoRoot, rulesPath } : { json, repoRoot };
}

function printHelp(): void {
  console.log(`maestro v2 architecture lint

Usage: bun scripts/lint/run-arch-lint-v2.ts [--json] [--repo-root <path>] [--rules <path>]

Rules sourced from docs/architecture.yaml (or --rules <path>):
  layer-order       error  forward_only layer chain; cross_cutting layers exempt both directions
  passive-harness   error  forbidden patterns (no daemon/scheduler/etc.) per docs/architecture.yaml

Exit codes:
  0  no error-severity violations
  1  one or more error-severity violations
`);
}

async function main(): Promise<void> {
  const args = parseArgs(process.argv.slice(2));
  const rulesPort = new YamlArchitectureRules({
    repoRoot: args.repoRoot,
    ...(args.rulesPath !== undefined ? { relPath: args.rulesPath } : {}),
  });
  const report = await runArchitectureLints({ repoRoot: args.repoRoot, rulesPort });

  if (args.json) {
    process.stdout.write(
      JSON.stringify(
        { filesScanned: report.filesScanned, violations: report.violations },
        null,
        2,
      ) + "\n",
    );
  } else if (report.violations.length === 0) {
    console.log(`v2 architecture lint: clean (${report.filesScanned} file(s) scanned)`);
  } else {
    const counts = { error: 0, warn: 0, info: 0 };
    for (const v of report.violations) counts[v.severity]++;
    console.log(
      `v2 architecture lint: ${report.violations.length} finding(s) across ${report.filesScanned} file(s) (${counts.error} error, ${counts.warn} warn, ${counts.info} info)`,
    );
    for (const v of report.violations) {
      const loc = v.line !== undefined ? `${v.file}:${v.line}` : v.file;
      console.log(`  [${v.severity}] ${v.rule_id} ${loc}: ${v.message}`);
    }
  }

  const hasError = report.violations.some((v) => v.severity === "error");
  process.exit(hasError ? 1 : 0);
}

await main();
