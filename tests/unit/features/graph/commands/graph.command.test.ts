import { afterEach, describe, expect, it } from "bun:test";
import { basename } from "node:path";
import { Command } from "commander";
import type { GraphContext, LinkOpts, LinkResult, ProjectGraphStorePort } from "@/features/graph";
import { registerGraphContextCommand, registerGraphLinkCommand } from "@/features/graph";

const originalConsoleLog = console.log;
const originalConsoleError = console.error;

function captureConsole(): {
  readonly logs: string[];
  readonly errors: string[];
} {
  const logs: string[] = [];
  const errors: string[] = [];
  console.log = (...args: unknown[]) => {
    logs.push(args.map((arg) => String(arg)).join(" "));
  };
  console.error = (...args: unknown[]) => {
    errors.push(args.map((arg) => String(arg)).join(" "));
  };
  return { logs, errors };
}

function graphDeps(overrides: {
  readonly linkProjects?: (store: ProjectGraphStorePort, opts: LinkOpts) => Promise<LinkResult>;
  readonly getGraphContext?: (store: ProjectGraphStorePort, currentName: string) => Promise<GraphContext>;
} = {}) {
  return {
    getServices: () => ({
      projectGraphStore: {
        load: async () => ({ nodes: [], edges: [] }),
        save: async () => undefined,
      },
    }),
    linkProjects: overrides.linkProjects ?? (async () => ({
      edge: { from: "maestro", to: "api", relation: "consumes" },
      nodesAdded: 0,
    })),
    getGraphContext: overrides.getGraphContext ?? (async () => ({
      currentProject: undefined,
      relationships: [],
      totalProjects: 0,
      totalEdges: 0,
    })),
  };
}

afterEach(() => {
  console.log = originalConsoleLog;
  console.error = originalConsoleError;
});

describe("graph commands", () => {
    it("formats successful graph-link output", async () => {
      const captured = captureConsole();
      const deps = graphDeps({
        linkProjects: async (_store, opts) => {
          expect(opts.currentName).toBe(basename(process.cwd()));
          expect(opts.currentPath).toBe(process.cwd());
        expect(opts.targetName).toBe("shared-types");
        expect(opts.relation).toBe("shared-types");
        expect(opts.detail).toBe("contracts");

        return {
          edge: {
            from: basename(process.cwd()),
            to: "shared-types",
            relation: "shared-types",
            detail: "contracts",
          },
            nodesAdded: 2,
          };
        },
      });

      const program = new Command().name("maestro").option("--json", "Output as JSON");
      registerGraphLinkCommand(program, deps);

    await program.parseAsync([
      "node",
      "maestro",
      "graph-link",
      "shared-types",
      "--shared-types",
      "--via",
      "contracts",
    ]);

    expect(captured.logs).toEqual([
      "[ok] Project linked",
      `  ${basename(process.cwd())} --[shared-types]--> shared-types`,
      "  Via: contracts",
      "  (2 new node(s) added)",
    ]);
  });

    it("rejects graph-link calls without a relation flag", async () => {
      const program = new Command().name("maestro").option("--json", "Output as JSON");
      registerGraphLinkCommand(program, graphDeps());

    await expect(
      program.parseAsync(["node", "maestro", "graph-link", "shared-types"]),
    ).rejects.toMatchObject({
      message: "Specify a relation: --consumes, --exposes, or --shared-types",
    });
  });

    it("formats graph-context output for unknown and linked projects", async () => {
      const captured = captureConsole();
      let firstCall = true;
      const deps = graphDeps({
        getGraphContext: async (_store, currentName) => {
          expect(currentName).toBe(basename(process.cwd()));

        if (firstCall) {
          firstCall = false;
          return {
            currentProject: undefined,
            relationships: [],
            totalProjects: 0,
            totalEdges: 0,
          };
        }

        return {
          currentProject: { name: currentName, path: process.cwd() },
            relationships: [
              {
                direction: "outgoing",
                edge: {
                  from: currentName,
                  to: "maestro-web",
                  relation: "exposes",
                  detail: "mcp",
                },
                project: { name: "maestro-web", path: "/code/maestro-web" },
              },
              {
                direction: "incoming",
                edge: {
                  from: "shared-types",
                  to: currentName,
                  relation: "consumes",
                },
                project: { name: "shared-types", path: "/code/shared-types" },
              },
            ],
          totalProjects: 3,
            totalEdges: 2,
          };
        },
      });

      const program = new Command().name("maestro").option("--json", "Output as JSON");
      registerGraphContextCommand(program, deps);

    await program.parseAsync(["node", "maestro", "graph-context"]);
    expect(captured.logs).toEqual([
      "Project Graph (0 projects, 0 links)",
      "",
      "Current project not in graph. Use `maestro graph-link` to add relationships.",
    ]);

    captured.logs.length = 0;

    await program.parseAsync(["node", "maestro", "graph-context"]);
    expect(captured.logs).toEqual([
      "Project Graph (3 projects, 2 links)",
      `  Current: ${basename(process.cwd())} (${process.cwd()})`,
      "",
      "  --> exposes: maestro-web",
      "      via: mcp",
      "  <-- consumes: shared-types",
    ]);
  });
});
