/**
 * Factory for constructing DoctrineItem objects with sensible defaults.
 * Centralizes timestamp handling, zero-initialized effectiveness, and schema versioning.
 */

import type { DoctrineItem, DoctrineConditions, DoctrineStatus } from '../../doctrine/port.ts';
import { CURRENT_SCHEMA_VERSION } from '../../doctrine/adapter.ts';

export interface BuildDoctrineItemOpts {
  name: string;
  rule: string;
  rationale: string;
  tags?: string[];
  conditionTags?: string[];
  conditionFilePatterns?: string[];
  sourceFeatures?: string[];
  sourceMemories?: string[];
  status?: DoctrineStatus;
  /** When updating, pass the existing item to preserve effectiveness and createdAt. */
  existing?: DoctrineItem;
}

const ZERO_EFFECTIVENESS = Object.freeze({
  injectionCount: 0,
  associatedSuccessRate: 0,
  overrideCount: 0,
});

export function buildDoctrineItem(opts: BuildDoctrineItemOpts): DoctrineItem {
  const now = new Date().toISOString();

  const conditions: DoctrineConditions = {
    tags: opts.conditionTags,
    filePatterns: opts.conditionFilePatterns,
  };

  return {
    name: opts.name,
    rule: opts.rule,
    rationale: opts.rationale,
    conditions,
    tags: opts.tags ?? [],
    source: {
      features: opts.sourceFeatures ?? [],
      memories: opts.sourceMemories ?? [],
    },
    effectiveness: opts.existing?.effectiveness ?? { ...ZERO_EFFECTIVENESS },
    status: opts.status ?? 'active',
    createdAt: opts.existing?.createdAt ?? now,
    updatedAt: now,
    schemaVersion: CURRENT_SCHEMA_VERSION,
  };
}
