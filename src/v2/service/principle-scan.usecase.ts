import type { Principle } from "../types/principle.js";
import type { PrinciplesStorePort } from "../repo/principles-store.port.js";
import type { ProcessRunnerPort } from "../repo/process-runner.port.js";

export type PrincipleScanFindingKind = "violation" | "scan-error";

export interface PrincipleScanFinding {
  readonly principle_slug: string;
  readonly kind: PrincipleScanFindingKind;
  readonly file?: string;
  readonly line?: number;
  readonly message: string;
}

export interface PrincipleScanReport {
  readonly principlesScanned: number;
  readonly findings: readonly PrincipleScanFinding[];
}

export interface PrincipleScanDeps {
  readonly principlesStore: PrinciplesStorePort;
  readonly processRunner: ProcessRunnerPort;
  readonly repoRoot: string;
}

export interface PrincipleScanInput {
  readonly only?: readonly string[];
}

export async function principlesScan(
  deps: PrincipleScanDeps,
  input: PrincipleScanInput = {},
): Promise<PrincipleScanReport> {
  const all = await deps.principlesStore.list();
  const wanted = input.only ? new Set(input.only) : undefined;
  const principles = wanted
    ? all.filter((p) => wanted.has(p.slug))
    : all;

  const findings: PrincipleScanFinding[] = [];
  for (const principle of principles) {
    findings.push(...(await scanOne(deps, principle)));
  }
  return { principlesScanned: principles.length, findings };
}

async function scanOne(
  deps: PrincipleScanDeps,
  principle: Principle,
): Promise<readonly PrincipleScanFinding[]> {
  const command = principle.scan_command.trim();
  if (command.length === 0) {
    return [
      {
        principle_slug: principle.slug,
        kind: "scan-error",
        message: "Principle has empty scan_command",
      },
    ];
  }
  const result = await deps.processRunner.run(command, { cwd: deps.repoRoot });
  if (result.exitCode === 0) return [];
  const stdoutLines = result.stdout.split("\n").map((l) => l.trim()).filter((l) => l.length > 0);
  if (stdoutLines.length === 0) {
    return [
      {
        principle_slug: principle.slug,
        kind: "scan-error",
        message:
          result.stderr.trim().length > 0
            ? `Scan command failed (exit ${result.exitCode}): ${result.stderr.trim()}`
            : `Scan command failed (exit ${result.exitCode}) with empty stdout`,
      },
    ];
  }
  return stdoutLines.map((line) => parseScanLine(principle.slug, line));
}

const SCAN_LINE_RE = /^(?<file>[^:]+):(?<line>\d+):\s*(?<message>.*)$/;

export function parseScanLine(
  principleSlug: string,
  raw: string,
): PrincipleScanFinding {
  const match = SCAN_LINE_RE.exec(raw);
  if (!match || !match.groups) {
    return {
      principle_slug: principleSlug,
      kind: "violation",
      message: raw,
    };
  }
  return {
    principle_slug: principleSlug,
    kind: "violation",
    file: match.groups.file,
    line: Number.parseInt(match.groups.line!, 10),
    message: match.groups.message ?? "",
  };
}
