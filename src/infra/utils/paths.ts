/**
 * Path utilities for maestroCLI.
 * Pure path construction -- no filesystem I/O.
 * Forked from hive-core/src/utils/paths.ts.
 */

import * as path from 'path';

const MAESTRO_DIR = '.maestro';
const FEATURES_DIR = 'features';
const TASKS_DIR = 'tasks';
const MEMORY_DIR = 'memory';
const PLAN_FILE = 'plan.md';
const COMMENTS_FILE = 'comments.json';
const FEATURE_FILE = 'feature.json';
const STATUS_FILE = 'status.json';
const REPORT_FILE = 'report.md';
const APPROVED_FILE = 'APPROVED';
const SPEC_FILE = 'spec.md';
const HANDOFFS_DIR = 'handoffs';
const VERIFICATION_FILE = 'verification.json';

export function normalizePath(filePath: string): string {
  return filePath.replace(/\\/g, '/');
}

export function getMaestroPath(projectRoot: string): string {
  return path.join(projectRoot, MAESTRO_DIR);
}

export function getFeaturesPath(projectRoot: string): string {
  return path.join(getMaestroPath(projectRoot), FEATURES_DIR);
}

export function getFeaturePath(projectRoot: string, featureName: string): string {
  return path.join(getFeaturesPath(projectRoot), featureName);
}

export function getPlanPath(projectRoot: string, featureName: string): string {
  return path.join(getFeaturePath(projectRoot, featureName), PLAN_FILE);
}

export function getCommentsPath(projectRoot: string, featureName: string): string {
  return path.join(getFeaturePath(projectRoot, featureName), COMMENTS_FILE);
}

export function getFeatureJsonPath(projectRoot: string, featureName: string): string {
  return path.join(getFeaturePath(projectRoot, featureName), FEATURE_FILE);
}

export function getMemoryPath(projectRoot: string, featureName: string): string {
  return path.join(getFeaturePath(projectRoot, featureName), MEMORY_DIR);
}

export function getGlobalMemoryPath(projectRoot: string): string {
  return path.join(getMaestroPath(projectRoot), MEMORY_DIR);
}

export function getTasksPath(projectRoot: string, featureName: string): string {
  return path.join(getFeaturePath(projectRoot, featureName), TASKS_DIR);
}

export function getTaskPath(projectRoot: string, featureName: string, taskFolder: string): string {
  return path.join(getTasksPath(projectRoot, featureName), taskFolder);
}

export function getTaskStatusPath(projectRoot: string, featureName: string, taskFolder: string): string {
  return path.join(getTaskPath(projectRoot, featureName, taskFolder), STATUS_FILE);
}

export function getTaskReportPath(projectRoot: string, featureName: string, taskFolder: string): string {
  return path.join(getTaskPath(projectRoot, featureName, taskFolder), REPORT_FILE);
}

export function getTaskSpecPath(projectRoot: string, featureName: string, taskFolder: string): string {
  return path.join(getTaskPath(projectRoot, featureName, taskFolder), SPEC_FILE);
}

export function getApprovedPath(projectRoot: string, featureName: string): string {
  return path.join(getFeaturePath(projectRoot, featureName), APPROVED_FILE);
}

export function getHandoffsPath(projectRoot: string, featureName: string): string {
  return path.join(getFeaturePath(projectRoot, featureName), HANDOFFS_DIR);
}

export function getHandoffPath(projectRoot: string, featureName: string, beadId: string): string {
  const safeName = beadId.replace(/[^a-z0-9-]/gi, '-');
  return path.join(getHandoffsPath(projectRoot, featureName), `${safeName}.md`);
}

export function getTaskVerificationPath(projectRoot: string, featureName: string, taskFolder: string): string {
  return path.join(getTaskPath(projectRoot, featureName, taskFolder), VERIFICATION_FILE);
}

// ============================================================================
// Doctrine Paths
// ============================================================================

const DOCTRINE_DIR = 'doctrine';

export function getDoctrinePath(projectRoot: string): string {
  return path.join(getMaestroPath(projectRoot), DOCTRINE_DIR);
}

export function getDoctrineItemPath(projectRoot: string, name: string): string {
  return path.join(getDoctrinePath(projectRoot), `${name}.json`);
}

export function getSessionsDir(projectDir: string): string {
  return path.join(projectDir, MAESTRO_DIR, 'sessions');
}

export const EVENTS_FILE = 'events.jsonl';
