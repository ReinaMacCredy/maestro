import type { ArchivePort } from "./ports/archive.port.js";
import { TarArchiveAdapter } from "./adapters/tar-archive.adapter.js";

export interface BundleServices {
  readonly archive: ArchivePort;
}

export function buildBundleServices(): BundleServices {
  return {
    archive: new TarArchiveAdapter(),
  };
}
