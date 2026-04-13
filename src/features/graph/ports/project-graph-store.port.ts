import type { ProjectGraph } from "../domain/types.js";

export interface ProjectGraphStorePort {
  load(): Promise<ProjectGraph>;
  save(graph: ProjectGraph): Promise<void>;
}
