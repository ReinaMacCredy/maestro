import type { Spec } from "../domain/types.js";

export interface SpecStorePort {
  write(spec: Spec): Promise<void>;
  read(missionId: string): Promise<Spec | undefined>;
  list(): Promise<Spec[]>;
}
