import type { ProjectGraphStorePort } from "./ports/project-graph-store.port.js";
import { FsProjectGraphStoreAdapter } from "./adapters/project-graph-store.adapter.js";

export interface GraphServices {
  readonly projectGraphStore: ProjectGraphStorePort;
}

export function buildGraphServices(): GraphServices {
  return {
    projectGraphStore: new FsProjectGraphStoreAdapter(),
  };
}
