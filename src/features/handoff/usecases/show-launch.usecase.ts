import type { HandoffLaunchRecord, LaunchStorePort } from "../domain/launch-types.js";
import { MaestroError } from "@/shared/errors.js";

export async function showLaunch(
  store: LaunchStorePort,
  id: string,
): Promise<HandoffLaunchRecord> {
  const record = await store.get(id);
  if (!record) {
    throw new MaestroError(`Handoff packet not found: ${id}`, [
      "Run `maestro handoff list` to see available packets",
    ]);
  }
  return record;
}
