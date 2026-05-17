/**
 * Mission bundle domain types.
 *
 * A mission bundle is a portable, auditable `.tar.gz` that packages everything
 * the conductor and agents produced for a single mission: mission plan,
 * features, assertions, agent prompts + reports, replies, handoffs,
 * principles snapshot, and optional memory + git patch.
 *
 * Phase 1 scope: export + inspect only. Import/replay is phase 2.
 */
import type { MissionStatus } from "@/shared/domain/legacy-mission";

/** Redaction scopes -- content dropped from the bundle when requested. */
export type BundleRedactScope = "prompts" | "replies";

/** Caller-facing options for `maestro bundle export`. */
export interface BundleOptions {
  readonly out?: string;
  readonly base?: string;
  readonly redact: readonly BundleRedactScope[];
}

/** In-memory description of a file to include in the tar archive. */
export interface BundleFile {
  readonly path: string;
  readonly content: string | Buffer;
}

export interface BundleStats {
  readonly features: number;
  readonly milestones: number;
  readonly assertions: number;
  readonly agents: number;
  readonly replies: number;
  readonly handoffs: number;
  readonly checkpoints: number;
  readonly principlesSnapshot: number;
  readonly outcomesSnapshot: number;
}

export interface BundleGitPatchInfo {
  readonly base: string;
  readonly commits: number;
  readonly bytes: number;
}

export interface BundleManifestMission {
  readonly id: string;
  readonly title: string;
  readonly status: MissionStatus;
  readonly createdAt: string;
  readonly completedAt?: string;
}

/** Schema v1 of the bundle manifest. */
export interface BundleManifest {
  readonly schemaVersion: 1;
  readonly bundleId: string;
  readonly createdAt: string;
  readonly createdBy?: string;
  readonly maestroVersion: string;
  readonly mission: BundleManifestMission;
  readonly stats: BundleStats;
  readonly redacted: readonly BundleRedactScope[];
  readonly gitPatch: BundleGitPatchInfo | null;
}

/** Result returned from the export usecase, consumed by the command layer. */
export interface BundleExportResult {
  readonly manifest: BundleManifest;
  readonly outputPath: string;
  readonly bytes: number;
}
