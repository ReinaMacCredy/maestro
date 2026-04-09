import type { ProjectEdge, ProjectNode, GraphRelation } from "../domain/memory-types.js";
import type { ProjectGraphStorePort } from "../ports/project-graph-store.port.js";

export interface LinkOpts {
  readonly targetName: string;
  readonly relation: GraphRelation;
  readonly detail?: string;
  readonly currentPath: string;
  readonly currentName: string;
  readonly targetPath?: string;
}

export interface LinkResult {
  readonly edge: ProjectEdge;
  readonly nodesAdded: number;
}

export async function linkProjects(
  store: ProjectGraphStorePort,
  opts: LinkOpts,
): Promise<LinkResult> {
  const graph = await store.load();
  const nodes = [...graph.nodes];
  const edges = [...graph.edges];
  let nodesAdded = 0;

  // Ensure current project node exists
  if (!nodes.some((n) => n.name === opts.currentName)) {
    nodes.push({ path: opts.currentPath, name: opts.currentName });
    nodesAdded++;
  }

  // Ensure target project node exists
  if (!nodes.some((n) => n.name === opts.targetName)) {
    nodes.push({ path: opts.targetPath ?? opts.targetName, name: opts.targetName });
    nodesAdded++;
  }

  const edge: ProjectEdge = {
    from: opts.currentName,
    to: opts.targetName,
    relation: opts.relation,
    detail: opts.detail,
  };

  // Replace existing edge with same from/to/relation or add new
  const existingIdx = edges.findIndex(
    (e) => e.from === edge.from && e.to === edge.to && e.relation === edge.relation,
  );
  if (existingIdx >= 0) {
    edges[existingIdx] = edge;
  } else {
    edges.push(edge);
  }

  await store.save({ nodes, edges });
  return { edge, nodesAdded };
}
