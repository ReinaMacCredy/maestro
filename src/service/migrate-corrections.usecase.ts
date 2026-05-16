import { readdir, readFile, mkdir, writeFile, access } from "node:fs/promises";
import { join } from "node:path";

// V1 correction shape as written by FsCorrectionStoreAdapter
// (.maestro/memory/corrections/<id>.json). Duplicated here on purpose
// so the v2 service does not import a feature-level type.
export interface V1CorrectionRecord {
  readonly id: string;
  readonly rule: string;
  readonly source: string;
  readonly trigger: {
    readonly keywords: readonly string[];
    readonly fileGlobs: readonly string[];
  };
  readonly severity: "soft" | "hard";
  readonly createdAt: string;
  readonly updatedAt: string;
}

export interface MigrateCorrectionsDeps {
  readonly repoRoot: string;
}

export interface MigrateCorrectionsInput {
  readonly overwrite?: boolean;
}

export interface MigrateCorrectionsResult {
  readonly scanned: number;
  readonly migrated: readonly string[];
  readonly skipped: readonly string[];
  readonly missing_source: boolean;
}

const SOURCE_REL = ".maestro/memory/corrections";
const DEST_REL = "docs/principles/legacy";

export async function migrateCorrections(
  deps: MigrateCorrectionsDeps,
  input: MigrateCorrectionsInput = {},
): Promise<MigrateCorrectionsResult> {
  const sourceDir = join(deps.repoRoot, SOURCE_REL);
  const destDir = join(deps.repoRoot, DEST_REL);

  let entries: string[];
  try {
    entries = await readdir(sourceDir);
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === "ENOENT") {
      return { scanned: 0, migrated: [], skipped: [], missing_source: true };
    }
    throw err;
  }

  const jsonFiles = entries.filter((name) => name.endsWith(".json"));
  if (jsonFiles.length === 0) {
    return { scanned: 0, migrated: [], skipped: [], missing_source: false };
  }

  await mkdir(destDir, { recursive: true });
  const migrated: string[] = [];
  const skipped: string[] = [];

  for (const name of jsonFiles) {
    const raw = await readFile(join(sourceDir, name), "utf8");
    let parsed: V1CorrectionRecord;
    try {
      parsed = JSON.parse(raw) as V1CorrectionRecord;
    } catch {
      skipped.push(name);
      continue;
    }
    if (!isCorrection(parsed)) {
      skipped.push(name);
      continue;
    }
    const destPath = join(destDir, `${parsed.id}.md`);
    if (!input.overwrite) {
      try {
        await access(destPath);
        skipped.push(parsed.id);
        continue;
      } catch {
        // file does not exist; proceed
      }
    }
    await writeFile(destPath, renderLegacyPrinciple(parsed), "utf8");
    migrated.push(parsed.id);
  }

  return { scanned: jsonFiles.length, migrated, skipped, missing_source: false };
}

function isCorrection(value: unknown): value is V1CorrectionRecord {
  if (value === null || typeof value !== "object") return false;
  const v = value as Record<string, unknown>;
  return (
    typeof v.id === "string" &&
    typeof v.rule === "string" &&
    typeof v.source === "string" &&
    typeof v.severity === "string" &&
    (v.severity === "soft" || v.severity === "hard") &&
    typeof v.trigger === "object" &&
    v.trigger !== null
  );
}

export function renderLegacyPrinciple(correction: V1CorrectionRecord): string {
  const keywords = correction.trigger.keywords.length
    ? correction.trigger.keywords.join(", ")
    : "(none recorded)";
  const fileGlobs = correction.trigger.fileGlobs.length
    ? correction.trigger.fileGlobs.join(", ")
    : "(none recorded)";
  const severityNote =
    correction.severity === "hard"
      ? "Originally recorded as a hard correction (gate-style)."
      : "Originally recorded as a soft correction (advisory).";
  return [
    `# ${correction.id}`,
    "",
    `> Legacy correction migrated from .maestro/memory/corrections/${correction.id}.json`,
    `> source: ${correction.source}`,
    `> createdAt: ${correction.createdAt}`,
    "",
    "## Rule",
    "",
    correction.rule.trim().length > 0 ? correction.rule.trim() : "(rule body was empty)",
    "",
    "## Rationale",
    "",
    severityNote,
    "",
    "## Scan Command",
    "",
    `# Triggers recorded against keywords: ${keywords}`,
    `# Triggers recorded against fileGlobs: ${fileGlobs}`,
    "# TODO: convert triggers into a real scan command before activating.",
    "",
    "## Fix Recipe",
    "",
    "Promoted-as-is from v1; edit to document the concrete remediation.",
    "",
  ].join("\n");
}
