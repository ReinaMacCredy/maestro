/**
 * Inspect a mission bundle without extracting it.
 * Delegates to the archive port's streaming manifest reader so callers can
 * peek at the manifest of even large bundles cheaply.
 */
import type { ArchivePort } from "../ports/archive.port.js";
import type { BundleManifest } from "../domain/bundle-types.js";

export interface InspectBundleDeps {
  readonly archive: ArchivePort;
}

export async function inspectBundle(
  deps: InspectBundleDeps,
  path: string,
): Promise<BundleManifest> {
  return deps.archive.readManifest(path);
}
