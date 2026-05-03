import { loadOwners } from "./usecases/load-owners.usecase.js";
import type { Owners } from "./domain/owners-types.js";

export interface PolicyServices {
  readonly loadOwners: () => Promise<Owners>;
}

export function buildPolicyServices(baseDir: string): PolicyServices {
  return {
    loadOwners: () => loadOwners(baseDir),
  };
}
