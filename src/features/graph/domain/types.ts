export type GraphRelation = "exposes" | "consumes" | "shared-types";

export interface ProjectNode {
  readonly path: string;
  readonly name: string;
  readonly role?: string;
}

export interface ProjectEdge {
  readonly from: string;
  readonly to: string;
  readonly relation: GraphRelation;
  readonly detail?: string;
}

export interface ProjectGraph {
  readonly nodes: readonly ProjectNode[];
  readonly edges: readonly ProjectEdge[];
}
