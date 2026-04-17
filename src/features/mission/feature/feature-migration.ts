/**
 * Pure schema migrations for feature records.
 *
 * Kept free of I/O so both the storage adapter (transparent on-read
 * upgrade) and the one-shot `migrate-feature-agent-type.ts` script can
 * share the same promotion rules without drifting.
 */

export interface LegacyWorkerTypeMigration<T = unknown> {
  readonly normalized: T;
  readonly migrated: boolean;
}

export function migrateLegacyWorkerType(
  data: unknown,
): LegacyWorkerTypeMigration {
  if (data === null || typeof data !== "object" || Array.isArray(data)) {
    return { normalized: data, migrated: false };
  }

  const record = data as Record<string, unknown>;
  if (!("workerType" in record)) {
    return { normalized: data, migrated: false };
  }

  const { workerType, ...rest } = record;
  if ("agentType" in record && record.agentType !== undefined) {
    return { normalized: rest, migrated: true };
  }

  return { normalized: { ...rest, agentType: workerType }, migrated: true };
}
