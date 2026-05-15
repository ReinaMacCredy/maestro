/**
 * Export a mission as a `.mission.tar.gz` bundle.
 *
 * Aggregates every artifact that belongs to the mission, optionally
 * computes a git patch, assembles the schema-v1 manifest, and delegates
 * writing to the archive port.
 */
import { randomUUID } from "node:crypto";
import { resolve } from "node:path";
import { MaestroError } from "@/shared/errors.js";
import { VERSION } from "@/shared/version.js";
import { execArgv } from "@/shared/lib/shell.js";
import {
  MISSION_ID_PATTERN,
} from "@/features/mission/index.js";
import { assertSafeSegment } from "@/shared/lib/path-safety.js";
import type { ArchivePort } from "../ports/archive.port.js";
import type {
  BundleExportResult,
  BundleFile,
  BundleGitPatchInfo,
  BundleManifest,
  BundleOptions,
} from "../domain/bundle-types.js";
import {
  collectBundleSources,
  type CollectBundleSourcesDeps,
} from "./collect-bundle-sources.usecase.js";

export interface ExportBundleInput {
  readonly missionId: string;
  readonly projectDir: string;
  readonly options: BundleOptions;
}

export interface ExportBundleDeps extends CollectBundleSourcesDeps {
  readonly archive: ArchivePort;
}

export async function exportBundle(
  deps: ExportBundleDeps,
  input: ExportBundleInput,
): Promise<BundleExportResult> {
  const { missionId, projectDir, options } = input;
  assertSafeSegment(
    missionId,
    "mission ID",
    MISSION_ID_PATTERN,
    "YYYY-MM-DD-NNN",
  );

  const sources = await collectBundleSources(deps, {
    missionId,
    projectDir,
    options,
  });

  const files: BundleFile[] = [...sources.files];

  let gitPatch: BundleGitPatchInfo | null = null;
  if (options.base) {
    const patchInfo = await computeGitPatch(projectDir, options.base);
    if (patchInfo) {
      files.push({
        path: `${missionId}.mission/diff.patch`,
        content: patchInfo.content,
      });
      gitPatch = {
        base: options.base,
        commits: patchInfo.commits,
        bytes: patchInfo.bytes,
      };
    }
  }

  const manifest: BundleManifest = {
    schemaVersion: 1,
    bundleId: randomUUID(),
    createdAt: new Date().toISOString(),
    maestroVersion: VERSION,
    mission: {
      id: sources.mission.id,
      title: sources.mission.title,
      status: sources.mission.status,
      createdAt: sources.mission.createdAt,
      ...(sources.mission.completedAt !== undefined && {
        completedAt: sources.mission.completedAt,
      }),
    },
    stats: sources.stats,
    redacted: options.redact,
    gitPatch,
  };

  files.unshift({
    path: `${missionId}.mission/manifest.json`,
    content: JSON.stringify(manifest, null, 2) + "\n",
  });

  const outputPath = resolve(options.out ?? defaultOutputName(missionId));
  const bytes = await deps.archive.writeTarGz(outputPath, files);

  return { manifest, outputPath, bytes };
}

interface GitPatchPayload {
  readonly content: string;
  readonly commits: number;
  readonly bytes: number;
}

async function computeGitPatch(
  projectDir: string,
  base: string,
): Promise<GitPatchPayload | undefined> {
  const range = `${base}..HEAD`;
  const patch = await execArgv(
    ["git", "-C", projectDir, "format-patch", range, "--stdout"],
  );
  if (patch.exitCode !== 0) {
    throw new MaestroError(`git format-patch failed: ${patch.stderr}`, [
      `Range: ${range}`,
      "Verify --base points to a valid ref or omit it to skip the patch",
    ]);
  }
  if (!patch.stdout) return undefined;

  const commits = await execArgv(
    ["git", "-C", projectDir, "rev-list", "--count", range],
  );
  const commitCount = commits.exitCode === 0
    ? Number.parseInt(commits.stdout.trim(), 10) || 0
    : 0;

  const content = patch.stdout.endsWith("\n") ? patch.stdout : `${patch.stdout}\n`;
  return {
    content,
    commits: commitCount,
    bytes: Buffer.byteLength(content, "utf8"),
  };
}

function defaultOutputName(missionId: string): string {
  const now = new Date();
  const pad = (n: number): string => String(n).padStart(2, "0");
  const stamp = `${now.getFullYear()}${pad(now.getMonth() + 1)}${pad(now.getDate())}-${pad(now.getHours())}${pad(now.getMinutes())}${pad(now.getSeconds())}`;
  return `${missionId}-${stamp}.mission.tar.gz`;
}
