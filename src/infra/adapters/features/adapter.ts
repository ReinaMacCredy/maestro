/**
 * Filesystem-based feature adapter for maestroCLI.
 * Forked from hive-core/src/services/featureService.ts.
 * Adapted: stripped getTasks() -- task listing goes through TaskPort.
 */

import {
  getFeaturePath,
  getFeatureJsonPath,
  getMemoryPath,
  getTasksPath,
  getPlanPath,
  getCommentsPath,
} from '../../utils/paths.ts';
import { ensureDir, readJson, writeJsonAtomic, fileExists } from '../../utils/fs-io.ts';
import { acquireLockSync } from '../../utils/locking.ts';
import type { FeatureJson, FeatureStatusType, CommentsJson } from '../../../domain/types.ts';
import { listFeatures } from './detection.ts';
import { MaestroError } from '../../../domain/errors.ts';
import { validateName } from '../../utils/validate-name.ts';
import type { FeaturePort } from '../../../domain/ports/feature.ts';

export class FsFeatureAdapter implements FeaturePort {
  constructor(private projectRoot: string) {}

  create(name: string, ticket?: string): FeatureJson {
    const validation = validateName(name, 'Feature name');
    if (!validation.ok) {
      throw new MaestroError(validation.error);
    }

    const featurePath = getFeaturePath(this.projectRoot, validation.name);

    if (fileExists(featurePath)) {
      throw new Error(`Feature '${validation.name}' already exists`);
    }

    // Detect case-insensitive filesystem collisions (macOS HFS+/APFS)
    const existing = this.list();
    const collision = existing.find(f => f.toLowerCase() === name.toLowerCase());
    if (collision) {
      throw new MaestroError(
        `Feature '${name}' conflicts with existing feature '${collision}' on case-insensitive filesystem`,
        [`Rename to avoid collision or use the existing feature: ${collision}`],
      );
    }

    ensureDir(featurePath);
    ensureDir(getMemoryPath(this.projectRoot, name));
    ensureDir(getTasksPath(this.projectRoot, name));

    const feature: FeatureJson = {
      name,
      status: 'planning',
      ticket,
      createdAt: new Date().toISOString(),
    };

    writeJsonAtomic(getFeatureJsonPath(this.projectRoot, name), feature);

    return feature;
  }

  get(name: string): FeatureJson | null {
    return readJson<FeatureJson>(getFeatureJsonPath(this.projectRoot, name));
  }

  list(): string[] {
    return listFeatures(this.projectRoot);
  }

  /** Get feature or throw. Rejects completed features. */
  requireActive(name: string): FeatureJson {
    const feature = this.get(name);
    if (!feature) throw new MaestroError(`Feature '${name}' not found`);
    if (feature.status === 'completed') {
      throw new MaestroError(`Feature '${name}' is completed`, ['Completed features cannot be modified']);
    }
    return feature;
  }

  getActive(preloadedList?: string[]): FeatureJson | null {
    const features = preloadedList ?? this.list();
    for (const name of features) {
      const feature = this.get(name);
      if (feature && feature.status !== 'completed') {
        return feature;
      }
    }
    return null;
  }

  updateStatus(name: string, status: FeatureStatusType): FeatureJson {
    const jsonPath = getFeatureJsonPath(this.projectRoot, name);
    const release = acquireLockSync(jsonPath);
    try {
      const feature = readJson<FeatureJson>(jsonPath);
      if (!feature) throw new Error(`Feature '${name}' not found`);

      feature.status = status;

      if (status === 'approved' && !feature.approvedAt) {
        feature.approvedAt = new Date().toISOString();
      }
      if (status === 'handed-off' && !feature.handedOffAt) {
        feature.handedOffAt = new Date().toISOString();
      }
      if (status === 'review-pending' && !feature.reviewPendingAt) {
        feature.reviewPendingAt = new Date().toISOString();
      }
      if (status === 'completed' && !feature.completedAt) {
        feature.completedAt = new Date().toISOString();
      }

      writeJsonAtomic(jsonPath, feature);
      return feature;
    } finally {
      release();
    }
  }

  /**
   * Get feature info without task list.
   * Task listing is now the responsibility of TaskPort.
   */
  getInfo(name: string): { name: string; status: FeatureStatusType; hasPlan: boolean; commentCount: number } | null {
    const feature = this.get(name);
    if (!feature) return null;

    const hasPlan = fileExists(getPlanPath(this.projectRoot, name));
    const comments = readJson<CommentsJson>(getCommentsPath(this.projectRoot, name));
    const commentCount = comments?.threads?.length || 0;

    return {
      name: feature.name,
      status: feature.status,
      hasPlan,
      commentCount,
    };
  }

  complete(name: string): FeatureJson {
    const feature = this.get(name);
    if (!feature) throw new Error(`Feature '${name}' not found`);

    if (feature.status === 'completed') {
      return feature; // idempotent -- already completed
    }

    return this.updateStatus(name, 'completed');
  }

  setSession(name: string, sessionId: string): void {
    const jsonPath = getFeatureJsonPath(this.projectRoot, name);
    const release = acquireLockSync(jsonPath);
    try {
      const feature = readJson<FeatureJson>(jsonPath);
      if (!feature) throw new Error(`Feature '${name}' not found`);

      feature.sessionId = sessionId;
      writeJsonAtomic(jsonPath, feature);
    } finally {
      release();
    }
  }

  getSession(name: string): string | undefined {
    const feature = this.get(name);
    return feature?.sessionId;
  }
}
