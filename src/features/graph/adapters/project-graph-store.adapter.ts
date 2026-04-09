import { join } from "node:path";
import type { ProjectGraph } from "@/domain/memory-types.js";
import { GRAPH_DIR } from "@/domain/defaults.js";
import { ensureDir, readJson, writeJson } from "@/lib/fs.js";
import type { ProjectGraphStorePort } from "../ports/project-graph-store.port.js";

export class FsProjectGraphStoreAdapter implements ProjectGraphStorePort {
  private graphPath(): string {
    return join(GRAPH_DIR, "projects.json");
  }

  async load(): Promise<ProjectGraph> {
    return (await readJson<ProjectGraph>(this.graphPath())) ?? { nodes: [], edges: [] };
  }

  async save(graph: ProjectGraph): Promise<void> {
    await ensureDir(GRAPH_DIR);
    await writeJson(this.graphPath(), graph);
  }
}
