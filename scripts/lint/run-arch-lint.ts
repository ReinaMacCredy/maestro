#!/usr/bin/env bun
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import {
  checkArchitectureRules,
  type ArchitectureViolation,
} from "@/features/verify/usecases/checks/check-architecture-lints.js";

interface ParsedArgs {
  readonly base?: string;
  readonly json: boolean;
  readonly repoRoot: string;
}

function parseArgs(argv: readonly string[]): ParsedArgs {
  let base: string | undefined;
  let json = false;
  let repoRoot: string | undefined;
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === "--base" && argv[i + 1]) {
      base = argv[++i];
    } else if (arg === "--json") {
      json = true;
    } else if (arg === "--repo-root" && argv[i + 1]) {
      repoRoot = argv[++i];
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
  return base !== undefined
    ? { base, json, repoRoot }
    : { json, repoRoot };
}

function printHelp(): void {
  console.log(`maestro architecture lint

Usage: bun scripts/lint/run-arch-lint.ts [--base <ref>] [--json] [--repo-root <path>]

Rules:
  no-runner-inversion       error  Maestro must not spawn Claude/Codex CLIs
  single-opentui-render     error  root.render() at most once per process
  mission-control-readonly  warn   MC snapshot/preview/render-check stays read-only
  no-hand-edit-generated    error  generated templates require touching their source (diff-aware)

Exit codes:
  0  no error-severity violations
  1  one or more error-severity violations
`);
}

async function resolveDiff(repoRoot: string, base: string): Promise<{ base: string; changedPaths: string[] }> {
  const proc = Bun.spawnSync({
    cmd: ["git", "diff", "--name-only", `${base}...HEAD`],
    cwd: repoRoot,
    stdout: "pipe",
    stderr: "pipe",
  });
  if (proc.exitCode !== 0) {
    const stderr = new TextDecoder().decode(proc.stderr);
    throw new Error(`git diff failed: ${stderr.trim()}`);
  }
  const stdout = new TextDecoder().decode(proc.stdout);
  const changedPaths = stdout.split("\n").map((s) => s.trim()).filter(Boolean);
  return { base, changedPaths };
}

function printText(violations: readonly ArchitectureViolation[]): void {
  if (violations.length === 0) {
    console.log("Architecture lint: clean");
    return;
  }
  const counts = { error: 0, warn: 0, info: 0 };
  for (const v of violations) counts[v.severity]++;
  console.log(
    `Architecture lint: ${violations.length} finding${violations.length !== 1 ? "s" : ""} (${counts.error} error, ${counts.warn} warn, ${counts.info} info)`,
  );
  for (const v of violations) {
    const loc = v.file ? `${v.file}${v.line !== undefined ? `:${v.line}` : ""}` : "(no file)";
    console.log(`  [${v.severity}] ${v.ruleId} — ${loc}`);
    console.log(`    ${v.message}`);
    if (v.snippet) console.log(`    > ${v.snippet}`);
  }
  if (counts.error > 0) {
    console.log("");
    console.log("Remediation:");
    const seen = new Set<string>();
    for (const v of violations) {
      if (v.severity !== "error" || seen.has(v.ruleId)) continue;
      seen.add(v.ruleId);
      console.log(`  ${v.ruleId}: ${v.remediation}`);
    }
  }
}

async function main(): Promise<void> {
  const args = parseArgs(process.argv.slice(2));
  const diff = args.base ? await resolveDiff(args.repoRoot, args.base) : undefined;
  const violations = await checkArchitectureRules({
    repoRoot: args.repoRoot,
    ...(diff ? { diff } : {}),
  });
  if (args.json) {
    process.stdout.write(JSON.stringify({ violations }, null, 2) + "\n");
  } else {
    printText(violations);
  }
  const hasError = violations.some((v) => v.severity === "error");
  process.exit(hasError ? 1 : 0);
}

await main();
