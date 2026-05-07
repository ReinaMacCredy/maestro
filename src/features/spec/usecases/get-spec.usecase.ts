import type { Spec } from "../domain/types.js";
import type { SpecStorePort } from "../ports/storage.js";

export async function getSpec(
  store: SpecStorePort,
  missionId: string,
): Promise<Spec | undefined> {
  return store.read(missionId);
}
