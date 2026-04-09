// Worker feature has no stores of its own; the services object is empty.
// This file exists for symmetry with other features' composition pattern.
// Worker is a pure usecase + types + lib cluster consumed by other features
// (mission's feature.command invokes generateWorkerPrompt; install/uninstall/
// update commands invoke manage-agents helpers).

export interface WorkerServices {
  // intentionally empty -- worker has no ports.
}

export function buildWorkerServices(_projectDir: string): WorkerServices {
  return {};
}
