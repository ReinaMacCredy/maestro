import { readdir, readFile, stat } from "node:fs/promises";
import { join } from "node:path";
import type { GitPort } from "../ports/git.port.js";
import type { ConfigPort } from "../ports/config.port.js";
import { listIgnoredProjectConfigKeys } from "@/shared/domain/ui-config.js";
import type { DoctorCheck } from "@/infra/domain/status-types.js";
import { countLegacyHandoffFiles, type CountLegacyHandoffFilesOptions } from "@/features/handoff";

/**
 * Phase 1 strip: CASS and agent-transport checks were removed. The
 * conductor model does not spawn runtime agents or depend on CASS, so these
 * checks no longer map to anything the CLI can fix.
 */
export async function runDoctor(
  git: GitPort,
  config: ConfigPort,
  dir: string,
  options: CountLegacyHandoffFilesOptions = {},
): Promise<DoctorCheck[]> {
  const [
    gitAvailable,
    projectConfig,
    globalConfig,
    configLayers,
    legacyHandoffCount,
    emptyFeatureDirs,
    oversizedRootDocs,
  ] = await Promise.all([
    git.isRepo(dir),
    config.exists("project", dir),
    config.exists("global", dir),
    config.loadLayers(dir),
    countLegacyHandoffFiles(dir, options),
    findEmptyFeatureDirs(dir),
    findOversizedRootDocs(dir),
  ]);

  const doctorChecks: DoctorCheck[] = [
    {
      name: "git",
      status: gitAvailable ? "ok" : "fail",
      message: gitAvailable ? "Git repository detected" : "Not inside a git repository",
      fix: gitAvailable ? undefined : "Run: git init",
    },
    {
      name: "project-config",
      status: projectConfig ? "ok" : "warn",
      message: projectConfig ? "Project config found at .maestro/config.yaml" : "No project config found",
      fix: projectConfig ? undefined : "Run: maestro init",
    },
    {
      name: "global-config",
      status: globalConfig ? "ok" : "warn",
      message: globalConfig ? "Global config found at ~/.maestro/config.yaml" : "No global config found",
      fix: globalConfig ? undefined : "Run: maestro init --global",
    },
  ];

  for (const keyPath of listIgnoredProjectConfigKeys(configLayers.project)) {
    doctorChecks.push({
      name: `ignored-${keyPath.replaceAll(".", "-")}`,
      status: "warn",
      message: `${keyPath} is set in project config but only global config is used`,
      fix: "Remove the project value or set it in ~/.maestro/config.yaml instead",
    });
  }

  if (legacyHandoffCount > 0) {
    doctorChecks.push({
      name: "legacy-handoffs",
      status: "warn",
      message: `Found ${legacyHandoffCount} legacy handoff artifact(s) under .maestro/handoffs/ or .maestro/launches/`,
      fix: "Review or remove the old files manually. Maestro now writes handoff artifacts to ~/.maestro/handoff/",
    });
  }

  for (const featureDir of emptyFeatureDirs) {
    doctorChecks.push({
      name: `empty-feature-${featureDir}`,
      status: "warn",
      message: `src/features/${featureDir}/ has no .ts files; either populate or remove the directory`,
      fix: `Implement the feature under src/features/${featureDir}/, or delete the directory if it was a stub`,
    });
  }

  for (const doc of oversizedRootDocs) {
    doctorChecks.push({
      name: `oversized-root-doc-${doc.name.replaceAll(".", "-")}`,
      status: "warn",
      message: `${doc.name} (${doc.lineCount} lines) is at the repo root; planning docs of this size belong under docs/`,
      fix: `Move ${doc.name} to docs/proposals/ (or delete if executed)`,
    });
  }

  return doctorChecks;
}

/**
 * Finds direct subdirectories of `<dir>/src/features` that contain zero `.ts`
 * files anywhere in their tree. These are stub directories that imply a
 * feature exists when none does.
 */
async function findEmptyFeatureDirs(dir: string): Promise<string[]> {
  const featuresRoot = join(dir, "src", "features");
  let entries: string[];
  try {
    entries = await readdir(featuresRoot);
  } catch {
    return [];
  }

  const empty: string[] = [];
  await Promise.all(
    entries.map(async (entry) => {
      const sub = join(featuresRoot, entry);
      let entryStat;
      try {
        entryStat = await stat(sub);
      } catch {
        return;
      }
      if (!entryStat.isDirectory()) return;
      if (await containsTsFile(sub)) return;
      empty.push(entry);
    }),
  );
  return empty.sort();
}

async function containsTsFile(dir: string): Promise<boolean> {
  let entries;
  try {
    entries = await readdir(dir, { withFileTypes: true });
  } catch {
    return false;
  }
  for (const entry of entries) {
    if (entry.isFile() && (entry.name.endsWith(".ts") || entry.name.endsWith(".tsx"))) {
      return true;
    }
    if (entry.isDirectory()) {
      if (await containsTsFile(join(dir, entry.name))) return true;
    }
  }
  return false;
}

const ROOT_DOC_ALLOWLIST = new Set([
  "README.md",
  "AGENTS.md",
  "CLAUDE.md",
  "CHANGELOG.md",
  "ROADMAP.md",
  "CONTRIBUTING.md",
  "SECURITY.md",
  "LICENSE",
  "LICENSE.md",
]);

const ROOT_DOC_LINE_LIMIT = 500;

/**
 * Returns root-level *.md files exceeding ROOT_DOC_LINE_LIMIT that are not in
 * the allowlist of canonical project files. Catches planning/proposal docs
 * that should live under docs/.
 */
async function findOversizedRootDocs(
  dir: string,
): Promise<{ name: string; lineCount: number }[]> {
  let entries;
  try {
    entries = await readdir(dir, { withFileTypes: true });
  } catch {
    return [];
  }

  const candidates = entries.filter(
    (e) => e.isFile() && e.name.endsWith(".md") && !ROOT_DOC_ALLOWLIST.has(e.name),
  );

  const results = await Promise.all(
    candidates.map(async (entry) => {
      try {
        const text = await readFile(join(dir, entry.name), "utf8");
        const lineCount = text.split("\n").length;
        if (lineCount > ROOT_DOC_LINE_LIMIT) {
          return { name: entry.name, lineCount };
        }
      } catch (err) {
        // Surface read failures (permission errors, transient FS issues)
        // instead of silently dropping the candidate from the doctor report.
        const message = err instanceof Error ? err.message : String(err);
        process.stderr.write(`[doctor] skipped ${entry.name}: ${message}\n`);
      }
      return undefined;
    }),
  );

  return results.filter((r): r is { name: string; lineCount: number } => r !== undefined);
}
