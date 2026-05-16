import { MaestroError } from "@/shared/errors.js";

// ============================
// Mission Error Factories
// ============================

export function missionNotFound(id: string): MaestroError {
  return new MaestroError(`Mission ${id} not found`, [
    "List missions: maestro mission list",
    `Check that mission ID '${id}' is correct`,
  ]);
}

export function milestoneNotFound(missionId: string, milestoneId: string): MaestroError {
  return new MaestroError(`Milestone '${milestoneId}' not found in mission ${missionId}`, [
    `List milestones: maestro milestone list --mission ${missionId}`,
    `Check that milestone ID '${milestoneId}' exists in mission '${missionId}'`,
  ]);
}

export function featureNotFound(id: string, missionId?: string): MaestroError {
  const hints = missionId
    ? [`List features: maestro feature list --mission ${missionId}`]
    : ["List features: maestro feature list"];
  return new MaestroError(`Feature ${id} not found`, [
    ...hints,
    `Check that feature ID '${id}' is correct`,
  ]);
}

export function assertionNotFound(id: string, missionId?: string): MaestroError {
  const hints = missionId
    ? [`Show assertions: maestro validate show --mission ${missionId}`]
    : [];
  return new MaestroError(`Assertion ${id} not found`, [
    ...hints,
    `Check that assertion ID '${id}' is correct`,
  ]);
}

export function invalidMissionTransition(
  from: string,
  to: string,
  validNext: readonly string[],
): MaestroError {
  const hint = validNext.length > 0
    ? `Valid transitions from ${from}: ${validNext.join(", ")}`
    : `${from} is a terminal state - no transitions allowed`;
  return new MaestroError(
    `Invalid mission transition: ${from} -> ${to}`,
    [hint, "Use 'maestro mission show' to view current state"],
  );
}

export function invalidMilestoneTransition(
  from: string,
  to: string,
  validNext: readonly string[],
): MaestroError {
  const hint = validNext.length > 0
    ? `Valid transitions from ${from}: ${validNext.join(", ")}`
    : `${from} is a terminal state - no transitions allowed`;
  return new MaestroError(
    `Invalid milestone transition: ${from} -> ${to}`,
    [hint, "Use 'maestro milestone status' to view current state"],
  );
}

export function invalidFeatureTransition(
  from: string,
  to: string,
  validNext: readonly string[],
): MaestroError {
  const hint = validNext.length > 0
    ? `Valid transitions from ${from}: ${validNext.join(", ")}`
    : `${from} is a terminal state - no transitions allowed`;
  return new MaestroError(
    `Invalid feature transition: ${from} -> ${to}`,
    [hint, "Use 'maestro feature list' to view current state"],
  );
}

export function invalidAssertionTransition(
  from: string,
  to: string,
  validNext: readonly string[],
): MaestroError {
  const hint = validNext.length > 0
    ? `Valid transitions from ${from}: ${validNext.join(", ")}`
    : `${from} is a terminal state - no transitions allowed`;
  return new MaestroError(
    `Invalid assertion transition: ${from} -> ${to}`,
    [hint, "Use 'maestro validate show' to view current state"],
  );
}

export function danglingReference(
  entityType: string,
  entityId: string,
  refType: string,
  refId: string,
): MaestroError {
  return new MaestroError(
    `Dangling reference: ${entityType} '${entityId}' references non-existent ${refType} '${refId}'`,
    [
      `Check that ${refType} '${refId}' exists before referencing it`,
      `Verify the ${refType} ID is spelled correctly`,
    ],
  );
}

export function cyclicDependency(cycle: readonly string[]): MaestroError {
  return new MaestroError(
    `Cyclic dependency detected: ${cycle.join(" -> ")}`,
    [
      `Review the 'dependsOn' arrays for features in this cycle`,
      `Remove circular references to fix the dependency graph`,
    ],
  );
}

export function duplicateMilestoneId(milestoneId: string): MaestroError {
  return new MaestroError(
    `Duplicate milestone ID: '${milestoneId}'`,
    [
      "Milestone IDs must be unique within a mission",
      `Rename one of the milestones with ID '${milestoneId}'`,
    ],
  );
}

export function milestoneNotSealable(
  milestoneId: string,
  blockingAssertions: readonly string[],
): MaestroError {
  return new MaestroError(
    `Milestone '${milestoneId}' cannot be sealed: ${blockingAssertions.length} assertion(s) not in terminal state`,
    [
      `Blocking assertions: ${blockingAssertions.join(", ")}`,
      "All assertions must be 'passed' or 'waived' to seal",
      `Update assertions with: maestro validate update --mission <id> --assertion <id>`,
    ],
  );
}

export function checkpointNotFound(id: string, missionId?: string): MaestroError {
  const hints = missionId
    ? [`List checkpoints: maestro checkpoint list --mission ${missionId}`]
    : [];
  return new MaestroError(`Checkpoint ${id} not found`, hints);
}
