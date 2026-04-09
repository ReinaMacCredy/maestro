// Mission feature public surface
// Commit 2 scaffolding: exports mission error factories.
// Commit 3 will expand this with types, ports, adapters, usecases, and commands.

export {
  missionNotFound,
  milestoneNotFound,
  featureNotFound,
  assertionNotFound,
  invalidMissionTransition,
  invalidMilestoneTransition,
  invalidFeatureTransition,
  invalidAssertionTransition,
  danglingReference,
  cyclicDependency,
  duplicateMilestoneId,
  milestoneNotSealable,
  checkpointNotFound,
} from "./domain/errors.js";
