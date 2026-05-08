import { join } from "node:path";
import type { EvidenceStorePort, EvidenceRow } from "@/features/evidence";
import type { VerdictStorePort, Verdict } from "@/features/verdict";
import { dirExists, fileExists, readText } from "@/shared/lib/fs.js";

export interface InspectRunDeps {
  readonly evidenceStore: EvidenceStorePort;
  readonly verdictStore: VerdictStorePort;
}

export interface InspectRunArgs {
  readonly projectRoot: string;
  readonly taskId: string;
  readonly tail?: number;
}

export interface RunArtifact {
  readonly file: string;
  readonly excerpt: string;
}

export interface InspectRunResult {
  readonly taskId: string;
  readonly runDir: string;
  readonly runDirExists: boolean;
  readonly artifacts: readonly RunArtifact[];
  readonly evidence: readonly Pick<
    EvidenceRow,
    "id" | "kind" | "witness_level" | "created_at"
  >[];
  readonly verdicts: readonly Pick<Verdict, "id" | "decision" | "computedAt">[];
}

const ARTIFACT_FILES = ["orient.md", "progress.md", "state.json", "plan.md"];
const ARTIFACT_PREVIEW_BYTES = 1200;

export async function inspectRun(
  deps: InspectRunDeps,
  args: InspectRunArgs,
): Promise<InspectRunResult> {
  const runDir = join(args.projectRoot, ".maestro/runs", args.taskId);
  const runDirExists = await dirExists(runDir);
  const artifacts: RunArtifact[] = [];

  if (runDirExists) {
    const known = await Promise.all(
      ARTIFACT_FILES.map(async (name) => {
        const full = join(runDir, name);
        if (!(await fileExists(full))) return undefined;
        const text = (await readText(full)) ?? "";
        return {
          file: name,
          excerpt:
            text.length <= ARTIFACT_PREVIEW_BYTES
              ? text
              : text.slice(0, ARTIFACT_PREVIEW_BYTES) + "\n…(truncated)",
        };
      }),
    );
    for (const a of known) if (a) artifacts.push(a);
  }

  const tail = args.tail ?? 10;
  const allEvidence = await deps.evidenceStore.list({ task_id: args.taskId });
  const allVerdicts = await deps.verdictStore.history(args.taskId);

  const evidence = allEvidence
    .slice(-tail)
    .map((e) => ({
      id: e.id,
      kind: e.kind,
      witness_level: e.witness_level,
      created_at: e.created_at,
    }));

  const verdicts = [...allVerdicts]
    .sort((a, b) => a.computedAt.localeCompare(b.computedAt))
    .slice(-tail)
    .map((v) => ({ id: v.id, decision: v.decision, computedAt: v.computedAt }));

  return { taskId: args.taskId, runDir, runDirExists, artifacts, evidence, verdicts };
}

export function formatInspectRunLines(r: InspectRunResult): string[] {
  const lines: string[] = [];
  lines.push(`Inspecting run for task ${r.taskId}`);
  lines.push(`  Run dir: ${r.runDir} (${r.runDirExists ? "exists" : "missing"})`);
  if (r.artifacts.length === 0) {
    lines.push(`  Artifacts: none`);
  } else {
    lines.push(`  Artifacts:`);
    for (const a of r.artifacts) {
      lines.push(`    --- ${a.file} ---`);
      for (const line of a.excerpt.split("\n")) lines.push(`    ${line}`);
    }
  }
  lines.push("");
  lines.push(`  Recent evidence (${r.evidence.length}):`);
  for (const e of r.evidence) {
    lines.push(`    ${e.created_at} ${e.kind} ${e.id} (${e.witness_level})`);
  }
  lines.push("");
  lines.push(`  Verdict history (${r.verdicts.length}):`);
  for (const v of r.verdicts) {
    lines.push(`    ${v.computedAt} ${v.decision} ${v.id}`);
  }
  return lines;
}
