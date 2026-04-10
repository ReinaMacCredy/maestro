import { buildInfraServices, type InfraServices } from "./infra/services.js";
import { buildSessionServices, type SessionServices } from "./features/session/index.js";
import { buildNotesServices, type NotesServices } from "./features/notes/index.js";
import { buildMissionServices, type MissionServices } from "./features/mission/index.js";
import { buildMemoryServices, type MemoryServices } from "./features/memory/index.js";
import { buildHandoffServices, type HandoffServices } from "./features/handoff/index.js";
import { buildRatchetServices, type RatchetServices } from "./features/ratchet/index.js";
import { buildGraphServices, type GraphServices } from "./features/graph/index.js";

export interface Services extends
  InfraServices,
  SessionServices,
  NotesServices,
  MissionServices,
  MemoryServices,
  HandoffServices,
  RatchetServices,
  GraphServices { }

let instance: Services | undefined;

export function initServices(projectDir: string): Services {
  instance = {
    ...buildInfraServices(projectDir),
    ...buildSessionServices(),
    ...buildNotesServices(projectDir),
    ...buildMissionServices(projectDir),
    ...buildMemoryServices(projectDir),
    ...buildHandoffServices(projectDir),
    ...buildRatchetServices(projectDir),
    ...buildGraphServices(),
  };
  return instance;
}

export function getServices(): Services {
  if (!instance) {
    throw new Error("Services not initialized. Call initServices() first.");
  }
  return instance;
}
