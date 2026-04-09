import type { ProjectGraph } from "../domain/memory-types.js";

export interface ProjectGraphStorePort {
  load(): Promise<ProjectGraph>;
  save(graph: ProjectGraph): Promise<void>;
}
