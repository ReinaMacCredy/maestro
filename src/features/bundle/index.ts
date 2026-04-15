export type {
  BundleExportResult,
  BundleFile,
  BundleGitPatchInfo,
  BundleManifest,
  BundleManifestMission,
  BundleMemoryStats,
  BundleOptions,
  BundleRedactScope,
  BundleStats,
} from "./domain/bundle-types.js";

export type { ArchivePort } from "./ports/archive.port.js";
export { TarArchiveAdapter } from "./adapters/tar-archive.adapter.js";

export {
  collectBundleSources,
  type BundleSources,
  type CollectBundleSourcesDeps,
  type CollectBundleSourcesInput,
} from "./usecases/collect-bundle-sources.usecase.js";

export {
  exportBundle,
  type ExportBundleDeps,
  type ExportBundleInput,
} from "./usecases/export-bundle.usecase.js";

export {
  inspectBundle,
  type InspectBundleDeps,
} from "./usecases/inspect-bundle.usecase.js";

export { registerBundleCommand } from "./commands/bundle.command.js";
export { buildBundleServices } from "./services.js";
export type { BundleServices } from "./services.js";
