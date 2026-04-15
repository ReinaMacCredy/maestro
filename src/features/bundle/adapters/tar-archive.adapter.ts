/**
 * Filesystem adapter for mission bundle archives.
 *
 * Uses the system `tar` binary via `execOrThrow` to stream a staging
 * directory into a gzipped tarball. Staging is cleaned up in both success
 * and failure paths.
 */
import { dirname, join, resolve } from "node:path";
import { mkdtemp, rm, stat } from "node:fs/promises";
import { tmpdir } from "node:os";
import { MaestroError } from "@/shared/errors.js";
import { ensureDir, writeText } from "@/shared/lib/fs.js";
import { execArgv, execOrThrow } from "@/shared/lib/shell.js";
import type { ArchivePort } from "../ports/archive.port.js";
import type { BundleFile, BundleManifest } from "../domain/bundle-types.js";

const SUPPORTED_SCHEMA_VERSIONS: readonly number[] = [1];

export class TarArchiveAdapter implements ArchivePort {
  async writeTarGz(outPath: string, files: readonly BundleFile[]): Promise<number> {
    const absoluteOut = resolve(outPath);
    await ensureDir(dirname(absoluteOut));
    const staging = await mkdtemp(join(tmpdir(), "maestro-bundle-"));
    try {
      for (const file of files) {
        const target = join(staging, file.path);
        await ensureDir(dirname(target));
        if (typeof file.content === "string") {
          await writeText(target, file.content);
        } else {
          await Bun.write(target, file.content);
        }
      }
      await execOrThrow(
        ["tar", "-czf", absoluteOut, "-C", staging, "."],
        "bundle.tar",
      );
    } finally {
      await rm(staging, { recursive: true, force: true });
    }

    const info = await stat(absoluteOut);
    return info.size;
  }

  async readManifest(tarPath: string): Promise<BundleManifest> {
    const absolute = resolve(tarPath);
    const listing = await execOrThrow(
      ["tar", "-tzf", absolute],
      "bundle.list",
    );
    const manifestEntry = listing.stdout
      .split("\n")
      .map((line) => line.trim())
      .find((line) => line.endsWith("/manifest.json") || line === "manifest.json");
    if (!manifestEntry) {
      throw new MaestroError(`Bundle is missing manifest.json: ${tarPath}`, [
        "The archive does not contain a manifest entry at the expected path",
        "Try re-exporting the mission with `maestro bundle export <missionId>`",
      ]);
    }

    const extract = await execArgv([
      "tar",
      "-xzf",
      absolute,
      "-O",
      manifestEntry,
    ]);
    if (extract.exitCode !== 0) {
      throw new MaestroError(`Failed to read bundle manifest: ${extract.stderr}`, [
        `Archive: ${tarPath}`,
        `Manifest entry: ${manifestEntry}`,
      ]);
    }

    let parsed: unknown;
    try {
      parsed = JSON.parse(extract.stdout);
    } catch (err) {
      throw new MaestroError(`Bundle manifest is not valid JSON: ${(err as Error).message}`, [
        `Archive: ${tarPath}`,
      ]);
    }

    return assertManifest(parsed);
  }
}

function assertManifest(value: unknown): BundleManifest {
  if (!value || typeof value !== "object") {
    throw new MaestroError("Bundle manifest is not a JSON object", []);
  }
  const manifest = value as BundleManifest;
  if (!SUPPORTED_SCHEMA_VERSIONS.includes(manifest.schemaVersion)) {
    throw new MaestroError(
      `Unsupported bundle schemaVersion: ${manifest.schemaVersion}`,
      [
        `Supported schema versions: ${SUPPORTED_SCHEMA_VERSIONS.join(", ")}`,
        "Upgrade maestro or re-export the bundle with a supported version",
      ],
    );
  }
  return manifest;
}
