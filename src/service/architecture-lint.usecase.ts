import { readFile } from "node:fs/promises";
import { dirname, normalize, posix } from "node:path";
import { Glob } from "bun";
import type {
  ArchitectureRules,
  ArchitectureRulesPort,
} from "../repo/architecture-rules.port.js";

export interface LintViolation {
  readonly rule_id: string;
  readonly severity: "error" | "warn" | "info";
  readonly file: string;
  readonly line?: number;
  readonly message: string;
  readonly remediation?: string;
}

export interface ArchitectureLintReport {
  readonly violations: readonly LintViolation[];
  readonly filesScanned: number;
}

export interface RunArchitectureLintsDeps {
  readonly rulesPort: ArchitectureRulesPort;
  readonly repoRoot: string;
}

const DEFAULT_SCOPES: readonly string[] = [
  "src/config/**/*.ts",
  "src/providers/**/*.ts",
  "src/repo/**/*.ts",
  "src/runtime/**/*.ts",
  "src/service/**/*.ts",
  "src/types/**/*.ts",
  "src/ui/**/*.ts",
];

const LAYER_NAMES = new Set([
  "config",
  "providers",
  "repo",
  "runtime",
  "service",
  "types",
  "ui",
]);

export async function runArchitectureLints(
  deps: RunArchitectureLintsDeps,
): Promise<ArchitectureLintReport> {
  const rules = await deps.rulesPort.load();
  const scopes = rules.lint_scope.length === 0 ? DEFAULT_SCOPES : rules.lint_scope;
  const files = await collectFiles(deps.repoRoot, scopes);

  const violations: LintViolation[] = [];
  for (const relFile of files) {
    if (isTestFile(relFile)) continue;
    const absFile = posix.join(deps.repoRoot, relFile);
    const text = await readFile(absFile, "utf8");
    violations.push(...lintLayerOrder(relFile, text, rules));
    violations.push(...lintPassiveHarness(relFile, text, rules));
  }

  return { violations, filesScanned: files.length };
}

async function collectFiles(
  repoRoot: string,
  scopes: readonly string[],
): Promise<readonly string[]> {
  const seen = new Set<string>();
  for (const pattern of scopes) {
    const glob = new Glob(pattern);
    for await (const file of glob.scan({ cwd: repoRoot, onlyFiles: true })) {
      seen.add(file.replaceAll("\\", "/"));
    }
  }
  return [...seen].sort();
}

function isTestFile(relFile: string): boolean {
  return relFile.endsWith(".test.ts") || relFile.endsWith(".test.tsx");
}

export function detectLayer(relFile: string): string | undefined {
  const m = relFile.match(/(?:^|\/)src\/([^/]+)\//);
  const layer = m?.[1];
  return layer && LAYER_NAMES.has(layer) ? layer : undefined;
}

export function resolveImportTargetLayer(
  importStr: string,
  sourceRelFile: string,
): string | undefined {
  if (importStr.startsWith("@/")) {
    const after = importStr.slice("@/".length);
    const first = after.split("/")[0];
    return first && LAYER_NAMES.has(first) ? first : undefined;
  }
  if (importStr.startsWith(".")) {
    const sourceDir = dirname(sourceRelFile);
    const resolved = normalize(`${sourceDir}/${importStr}`).replaceAll("\\", "/");
    return detectLayer(resolved);
  }
  return undefined;
}

const IMPORT_RE =
  /^\s*(?:import|export)\b[^"';]*?\s+from\s+["']([^"']+)["']|^\s*import\s+["']([^"']+)["']/gm;

function lintLayerOrder(
  relFile: string,
  text: string,
  rules: ArchitectureRules,
): readonly LintViolation[] {
  const sourceLayer = detectLayer(relFile);
  if (!sourceLayer) return [];
  if (rules.cross_cutting.includes(sourceLayer)) return [];
  const sourceIdx = rules.layers.indexOf(sourceLayer);
  if (sourceIdx === -1) return [];

  const violations: LintViolation[] = [];
  IMPORT_RE.lastIndex = 0;
  let match: RegExpExecArray | null;
  while ((match = IMPORT_RE.exec(text)) !== null) {
    const importStr = match[1] ?? match[2];
    if (importStr === undefined) continue;
    const targetLayer = resolveImportTargetLayer(importStr, relFile);
    if (!targetLayer || targetLayer === sourceLayer) continue;

    // cross_cutting layers (e.g. providers) are universally importable.
    if (rules.cross_cutting.includes(targetLayer)) continue;

    const targetIdx = rules.layers.indexOf(targetLayer);
    if (targetIdx === -1) continue;
    if (targetIdx > sourceIdx) {
      violations.push({
        rule_id: "layer-order",
        severity: "error",
        file: relFile,
        line: lineOf(text, match.index),
        message: `${sourceLayer} must not import from ${targetLayer} (forward_only: ${rules.layers.join(" -> ")})`,
      });
    }
  }
  return violations;
}

function lintPassiveHarness(
  relFile: string,
  text: string,
  rules: ArchitectureRules,
): readonly LintViolation[] {
  if (rules.passive_harness.forbidden_patterns.length === 0) return [];
  const violations: LintViolation[] = [];
  for (const pattern of rules.passive_harness.forbidden_patterns) {
    const re = new RegExp(`\\b${escapeRegex(pattern)}\\b`, "g");
    let match: RegExpExecArray | null;
    while ((match = re.exec(text)) !== null) {
      violations.push({
        rule_id: "passive-harness",
        severity: "error",
        file: relFile,
        line: lineOf(text, match.index),
        message: `forbidden pattern ${JSON.stringify(pattern)} found; maestro must remain passive per docs/architecture.yaml`,
      });
    }
  }
  return violations;
}

function lineOf(text: string, index: number): number {
  let line = 1;
  for (let i = 0; i < index; i++) {
    if (text.charCodeAt(i) === 10) line++;
  }
  return line;
}

function escapeRegex(input: string): string {
  return input.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

