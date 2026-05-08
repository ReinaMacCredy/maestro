import { Glob } from "bun";
import { join } from "node:path";
import type { TrustFinding } from "../../domain/types.js";

export type ArchitectureRuleId =
  | "no-runner-inversion"
  | "single-opentui-render"
  | "mission-control-readonly"
  | "no-hand-edit-generated";

export type ArchitectureSeverity = "info" | "warn" | "error";

export interface ArchitectureViolation {
  readonly ruleId: ArchitectureRuleId;
  readonly severity: ArchitectureSeverity;
  readonly file: string;
  readonly line?: number;
  readonly snippet?: string;
  readonly message: string;
  readonly remediation: string;
}

export interface ArchitectureLintInput {
  readonly repoRoot: string;
  readonly diff?: {
    readonly base: string;
    readonly changedPaths: readonly string[];
  };
}

const REMEDIATION: Record<ArchitectureRuleId, string> = {
  "no-runner-inversion":
    "Maestro must not spawn Claude or Codex CLIs as subprocesses. " +
    "The agent calls maestro; the inverse breaks the harness model. " +
    "Replace child_process / Bun.spawn of \"claude\"/\"codex\" with a " +
    "skill or CLI verb the agent invokes itself. For scheduled agent " +
    "work, emit external triggers (GH Actions, Claude Code hooks), " +
    "never an in-process spawn.",
  "single-opentui-render":
    "root.render() must be called at most once per process. Repeated " +
    "calls corrupt OpenTUI internal state. Use a bridge component with " +
    "useState/setState to update the tree. See src/tui/opentui/app/" +
    "interactive.tsx for the canonical pattern.",
  "mission-control-readonly":
    "Mission Control snapshot/preview/render-check paths must not write. " +
    "Move the write into a use-case under src/features/<f>/usecases/. " +
    "Re-read state in the snapshot path; do not produce new state there. " +
    "Agents inspect Mission Control assuming side-effect-free reads.",
  "no-hand-edit-generated":
    "Templates files are generated. Hand-edits are silently overwritten. " +
    "Edit source under skills/built-in/ or skills/bundled/ and run " +
    "`bun run sync:built-in-skills` or `bun run sync:bundled-skills`. " +
    "`bun run check:bundled-skills` enforces parity in CI.",
};

const SCAN_GLOB = "src/**/*.{ts,tsx}";
const TUI_RENDER_GLOB = "src/tui/**/*.{ts,tsx}";

const RUNNER_NAME_RE = /^(claude|codex|claude-code|codex-cli)$/;
const RUNNER_SPAWN_RE = /(?:Bun\.spawn(?:Sync)?|child_process\.(?:spawn|spawnSync|exec|execFile|execSync|execFileSync)|spawn|spawnSync|execFile|execFileSync|exec|execSync)\s*\(\s*\[?\s*["'`]([^"'`]+)["'`]/g;

const RENDER_CALL_RE = /\broot\s*\.\s*render\s*\(/g;

const WRITE_METHOD_NAMES = new Set([
  "append",
  "write",
  "record",
  "create",
  "update",
  "delete",
  "claim",
  "unclaim",
  "block",
  "unblock",
  "heartbeat",
  "syncMetadata",
  "backfillSlug",
  "backfillSlugs",
  "createBatch",
  "releaseOwned",
  "reopen",
  "increment",
]);

const MISSION_CONTROL_FUNCTIONS = ["buildSnapshot", "buildHomeSnapshot"];

const GENERATED_TEMPLATE_PATHS = [
  "src/infra/domain/built-in-skill-templates.ts",
  "src/infra/domain/bundled-skill-templates.ts",
];

const GENERATED_SOURCE_PREFIXES = ["skills/built-in/", "skills/bundled/"];

const ALLOW_RE = /\/\/\s*lint-arch-allow:\s*([a-z-,\s]+)/i;

function toPosix(p: string): string {
  return p.replace(/\\/g, "/");
}

function stripComments(text: string): string {
  // Strip line comments only — block-comment removal is fragile and not
  // necessary because the rules below match call expressions that don't
  // appear inside JSDoc.
  return text.replace(/^\s*\/\/.*$/gm, "");
}

function lineOf(text: string, index: number): { line: number; snippet: string } {
  const before = text.slice(0, index);
  const line = before.split("\n").length;
  const lineStart = before.lastIndexOf("\n") + 1;
  const lineEnd = text.indexOf("\n", index);
  const snippet = text.slice(lineStart, lineEnd === -1 ? text.length : lineEnd).trim();
  return { line, snippet };
}

function isAllowedAt(text: string, lineIndex: number, ruleId: ArchitectureRuleId): boolean {
  const lines = text.split("\n");
  const i = Math.min(lineIndex - 1, lines.length - 1);
  if (i < 0) return false;
  const here = lines[i] ?? "";
  const above = lines[i - 1] ?? "";
  for (const candidate of [here, above]) {
    const m = candidate.match(ALLOW_RE);
    if (!m) continue;
    const ids = m[1]?.split(/[,\s]+/) ?? [];
    if (ids.includes(ruleId)) return true;
  }
  return false;
}

function shouldSkipPath(relPath: string): boolean {
  const posix = toPosix(relPath);
  if (posix.startsWith("tests/")) return true;
  if (posix.startsWith("scripts/")) return true;
  if (posix.includes("/node_modules/")) return true;
  return false;
}

async function checkNoRunnerInversion(
  repoRoot: string,
): Promise<ArchitectureViolation[]> {
  const out: ArchitectureViolation[] = [];
  const glob = new Glob(SCAN_GLOB);
  for await (const relPath of glob.scan({ cwd: repoRoot })) {
    if (shouldSkipPath(relPath)) continue;
    const text = await Bun.file(join(repoRoot, relPath)).text();
    const stripped = stripComments(text);
    RUNNER_SPAWN_RE.lastIndex = 0;
    let match: RegExpExecArray | null;
    while ((match = RUNNER_SPAWN_RE.exec(stripped)) !== null) {
      const command = match[1] ?? "";
      if (!RUNNER_NAME_RE.test(command)) continue;
      const { line, snippet } = lineOf(stripped, match.index);
      // Re-resolve allow comment against original text — allow comments
      // are line comments and survive in the original.
      const origLine = lineOf(text, text.indexOf(match[0])).line;
      if (isAllowedAt(text, origLine, "no-runner-inversion")) continue;
      out.push({
        ruleId: "no-runner-inversion",
        severity: "error",
        file: toPosix(relPath),
        line,
        snippet,
        message: `Forbidden subprocess spawn of \`${command}\` runner`,
        remediation: REMEDIATION["no-runner-inversion"],
      });
    }
  }
  return out;
}

async function checkSingleOpenTuiRender(
  repoRoot: string,
): Promise<ArchitectureViolation[]> {
  const out: ArchitectureViolation[] = [];
  const glob = new Glob(TUI_RENDER_GLOB);
  for await (const relPath of glob.scan({ cwd: repoRoot })) {
    const posix = toPosix(relPath);
    if (posix.includes("/testing/")) continue;
    const text = await Bun.file(join(repoRoot, relPath)).text();
    const stripped = stripComments(text);
    const matches: RegExpExecArray[] = [];
    RENDER_CALL_RE.lastIndex = 0;
    let m: RegExpExecArray | null;
    while ((m = RENDER_CALL_RE.exec(stripped)) !== null) {
      matches.push(m);
    }
    if (matches.length <= 1) continue;
    for (const match of matches.slice(1)) {
      const { line, snippet } = lineOf(stripped, match.index);
      out.push({
        ruleId: "single-opentui-render",
        severity: "error",
        file: posix,
        line,
        snippet,
        message: `${matches.length} \`root.render(\` call expressions found in this file (max 1)`,
        remediation: REMEDIATION["single-opentui-render"],
      });
    }
  }
  return out;
}

interface FunctionBodyRange {
  readonly name: string;
  readonly bodyStart: number;
  readonly bodyEnd: number;
}

function extractFunctionBodies(
  text: string,
  names: readonly string[],
): FunctionBodyRange[] {
  const ranges: FunctionBodyRange[] = [];
  for (const name of names) {
    const re = new RegExp(
      `(?:export\\s+)?(?:async\\s+)?function\\s+${name}\\s*[<(]`,
      "g",
    );
    let m: RegExpExecArray | null;
    while ((m = re.exec(text)) !== null) {
      const openParen = text.indexOf("(", m.index);
      if (openParen === -1) continue;
      const openBrace = text.indexOf("{", openParen);
      if (openBrace === -1) continue;
      let depth = 0;
      let i = openBrace;
      for (; i < text.length; i++) {
        const ch = text[i];
        if (ch === "{") depth++;
        else if (ch === "}") {
          depth--;
          if (depth === 0) break;
        }
      }
      if (depth !== 0) continue;
      ranges.push({ name, bodyStart: openBrace, bodyEnd: i });
    }
  }
  return ranges;
}

async function checkMissionControlReadonly(
  repoRoot: string,
): Promise<ArchitectureViolation[]> {
  const out: ArchitectureViolation[] = [];
  const targets = [
    "src/tui/state/snapshot.ts",
    "src/infra/commands/mission-control.command.ts",
  ];
  const writeMethodCallRe = /\bawait\s+[A-Za-z_$][\w$.]*\.([A-Za-z_$][\w$]*)\s*\(/g;
  for (const relPath of targets) {
    const abs = join(repoRoot, relPath);
    const file = Bun.file(abs);
    if (!(await file.exists())) continue;
    const text = await file.text();
    const fnNames =
      relPath === "src/tui/state/snapshot.ts"
        ? MISSION_CONTROL_FUNCTIONS
        : ["runPreview", "runRenderCheck", "previewScreen"];
    const bodies = extractFunctionBodies(text, fnNames);
    if (bodies.length === 0) continue;
    for (const body of bodies) {
      const region = text.slice(body.bodyStart, body.bodyEnd);
      writeMethodCallRe.lastIndex = 0;
      let m: RegExpExecArray | null;
      while ((m = writeMethodCallRe.exec(region)) !== null) {
        const method = m[1] ?? "";
        if (!WRITE_METHOD_NAMES.has(method)) continue;
        const absoluteIndex = body.bodyStart + m.index;
        const { line, snippet } = lineOf(text, absoluteIndex);
        if (isAllowedAt(text, line, "mission-control-readonly")) continue;
        out.push({
          ruleId: "mission-control-readonly",
          severity: "warn",
          file: toPosix(relPath),
          line,
          snippet,
          message: `Mission Control function \`${body.name}\` calls write-shaped method \`.${method}(\``,
          remediation: REMEDIATION["mission-control-readonly"],
        });
      }
    }
  }
  return out;
}

function checkNoHandEditGenerated(
  diff: ArchitectureLintInput["diff"],
): ArchitectureViolation[] {
  if (diff === undefined) {
    return [
      {
        ruleId: "no-hand-edit-generated",
        severity: "info",
        file: "",
        message:
          "diff-aware rule skipped (no diff supplied — pass --base <ref> to enable)",
        remediation: REMEDIATION["no-hand-edit-generated"],
      },
    ];
  }
  const changed = diff.changedPaths.map(toPosix);
  const out: ArchitectureViolation[] = [];
  const sourceTouched = changed.some((p) =>
    GENERATED_SOURCE_PREFIXES.some((prefix) => p.startsWith(prefix)),
  );
  for (const generated of GENERATED_TEMPLATE_PATHS) {
    if (!changed.includes(generated)) continue;
    if (sourceTouched) continue;
    out.push({
      ruleId: "no-hand-edit-generated",
      severity: "error",
      file: generated,
      message: `Generated template \`${generated}\` was edited without touching its source under skills/`,
      remediation: REMEDIATION["no-hand-edit-generated"],
    });
  }
  return out;
}

export async function checkArchitectureRules(
  input: ArchitectureLintInput,
): Promise<ArchitectureViolation[]> {
  const { repoRoot, diff } = input;
  const [runner, render, readonly] = await Promise.all([
    checkNoRunnerInversion(repoRoot),
    checkSingleOpenTuiRender(repoRoot),
    checkMissionControlReadonly(repoRoot),
  ]);
  const generated = checkNoHandEditGenerated(diff);
  return [...runner, ...render, ...readonly, ...generated];
}

export function violationToTrustFinding(
  v: ArchitectureViolation,
): TrustFinding {
  const detailParts: string[] = [v.message];
  if (v.line !== undefined) detailParts.push(`line ${v.line}`);
  if (v.snippet !== undefined) detailParts.push(`> ${v.snippet}`);
  detailParts.push(v.remediation);
  return {
    check: v.ruleId,
    severity: v.severity,
    paths: v.file ? [v.file] : [],
    details: detailParts.join(" — "),
  };
}

/**
 * Wrapper used by `runTrustVerifier` as the 8th check. Adapts diff input,
 * runs all four rules, and returns TrustFinding[].
 */
export async function checkArchitectureLints(
  diff: { readonly base: string; readonly changedPaths: readonly string[] },
  projectRoot: string,
): Promise<readonly TrustFinding[]> {
  const violations = await checkArchitectureRules({
    repoRoot: projectRoot,
    diff: { base: diff.base, changedPaths: diff.changedPaths },
  });
  return violations.map(violationToTrustFinding);
}

export function isArchitectureRuleId(value: string): value is ArchitectureRuleId {
  return (
    value === "no-runner-inversion" ||
    value === "single-opentui-render" ||
    value === "mission-control-readonly" ||
    value === "no-hand-edit-generated"
  );
}
