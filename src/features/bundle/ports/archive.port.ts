/**
 * Archive port.
 *
 * Abstracts tar.gz read/write for mission bundles so the export and inspect
 * usecases can be tested against an in-memory fake. The filesystem adapter
 * shells out to the system `tar` binary.
 */
import type { BundleFile, BundleManifest } from "../domain/bundle-types.js";

export interface ArchivePort {
  /**
   * Write the given files as a gzipped tar archive at `outPath`.
   * Files with identical `path` entries overwrite the earlier one.
   * Returns the size in bytes of the resulting archive.
   */
  writeTarGz(outPath: string, files: readonly BundleFile[]): Promise<number>;

  /**
   * Extract only the manifest.json from a bundle without unpacking the rest.
   * Throws `MaestroError` when the manifest is missing, unreadable, or
   * declares an unsupported `schemaVersion`.
   */
  readManifest(tarPath: string): Promise<BundleManifest>;
}
