import { describe, it, expect } from "bun:test";
import { mockProjectGraphStore } from "../../../../helpers/mocks.js";
import { getGraphContext } from "@/features/graph";

describe("getGraphContext", () => {
  it("returns empty relationships for unknown project", async () => {
    const store = mockProjectGraphStore();
    const ctx = await getGraphContext(store, "maestro");
    expect(ctx.currentProject).toBeUndefined();
    expect(ctx.relationships.length).toBe(0);
  });

  it("returns outgoing and incoming relationships", async () => {
    const store = mockProjectGraphStore({
      nodes: [
        { path: "/code/maestro", name: "maestro" },
        { path: "/code/web", name: "maestro-web" },
        { path: "/code/types", name: "shared-types" },
      ],
      edges: [
        { from: "maestro", to: "maestro-web", relation: "exposes", detail: "mcp-tools" },
        { from: "shared-types", to: "maestro", relation: "shared-types" },
      ],
    });

    const ctx = await getGraphContext(store, "maestro");
    expect(ctx.currentProject?.name).toBe("maestro");
    expect(ctx.relationships.length).toBe(2);

    const outgoing = ctx.relationships.find((r) => r.direction === "outgoing");
    expect(outgoing?.project.name).toBe("maestro-web");
    expect(outgoing?.edge.relation).toBe("exposes");

    const incoming = ctx.relationships.find((r) => r.direction === "incoming");
    expect(incoming?.project.name).toBe("shared-types");
  });

  it("reports totals", async () => {
    const store = mockProjectGraphStore({
      nodes: [
        { path: "/a", name: "a" },
        { path: "/b", name: "b" },
      ],
      edges: [{ from: "a", to: "b", relation: "consumes" }],
    });

    const ctx = await getGraphContext(store, "a");
    expect(ctx.totalProjects).toBe(2);
    expect(ctx.totalEdges).toBe(1);
  });
});
