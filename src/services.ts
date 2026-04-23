import { buildInfraServices, type InfraServices } from "./infra/services.js";
import { buildSessionServices, type SessionServices } from "./features/session/services.js";
import { buildNotesServices, type NotesServices } from "./features/notes/services.js";
import { buildMissionServices, type MissionServices } from "./features/mission/services.js";
import { buildMemoryServices, type MemoryServices } from "./features/memory/services.js";
import { buildHandoffServices, type HandoffServices } from "./features/handoff/services.js";
import { buildRatchetServices, type RatchetServices } from "./features/ratchet/services.js";
import { buildGraphServices, type GraphServices } from "./features/graph/services.js";
import { buildTaskServices, type TaskServices } from "./features/task/services.js";
import { buildReplyServices, type ReplyServices } from "./features/reply/services.js";
import { buildBundleServices, type BundleServices } from "./features/bundle/services.js";

export interface Services extends
  InfraServices,
  SessionServices,
  NotesServices,
  MissionServices,
  MemoryServices,
  HandoffServices,
  RatchetServices,
  GraphServices,
  TaskServices,
  ReplyServices,
  BundleServices { }

let instance: Services | undefined;

export function initServices(projectDir: string): Services {
  instance = {
    ...buildInfraServices(projectDir),
    ...buildSessionServices(),
    ...buildNotesServices(projectDir),
    ...buildMissionServices(projectDir),
    ...buildMemoryServices(projectDir),
    ...buildHandoffServices(),
    ...buildRatchetServices(projectDir),
    ...buildGraphServices(),
    ...buildTaskServices(projectDir),
    ...buildReplyServices(projectDir),
    ...buildBundleServices(),
  };
  return instance;
}

export function getServices(): Services {
  if (!instance) {
    throw new Error("Services not initialized. Call initServices() first.");
  }
  return instance;
}
