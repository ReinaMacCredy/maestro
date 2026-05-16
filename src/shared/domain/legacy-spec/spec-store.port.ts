import type { Spec } from "./types.js";

/** v1 SpecStorePort — distinct from v2's SpecStorePort (which is typed against ProductSpec). */
export interface LegacySpecStorePort {
  write(spec: Spec): Promise<void>;
  read(missionId: string): Promise<Spec | undefined>;
  list(): Promise<Spec[]>;
}
