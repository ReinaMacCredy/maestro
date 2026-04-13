import type { ProjectEdge, ProjectNode } from "../domain/types.js";
import type { ProjectGraphStorePort } from "../ports/project-graph-store.port.js";

export interface ProjectRelationship {
  readonly project: ProjectNode;
  readonly direction: "outgoing" | "incoming";
  readonly edge: ProjectEdge;
}

export interface GraphContext {
  readonly currentProject?: ProjectNode;
  readonly relationships: readonly ProjectRelationship[];
  readonly totalProjects: number;
  readonly totalEdges: number;
}

export async function getGraphContext(
  store: ProjectGraphStorePort,
  currentName: string,
): Promise<GraphContext> {
  const graph = await store.load();
  const currentProject = graph.nodes.find((n) => n.name === currentName);

  const relationships: ProjectRelationship[] = [];

  for (const edge of graph.edges) {
    if (edge.from === currentName) {
      const target = graph.nodes.find((n) => n.name === edge.to);
      if (target) {
        relationships.push({ project: target, direction: "outgoing", edge });
      }
    } else if (edge.to === currentName) {
      const source = graph.nodes.find((n) => n.name === edge.from);
      if (source) {
        relationships.push({ project: source, direction: "incoming", edge });
      }
    }
  }

  return {
    currentProject,
    relationships,
    totalProjects: graph.nodes.length,
    totalEdges: graph.edges.length,
  };
}
