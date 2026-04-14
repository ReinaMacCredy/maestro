import { afterEach, describe, expect, it, mock } from "bun:test";
import { basename } from "node:path";
import { Command } from "commander";

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

async function loadGraphCommands(overrides: {
  readonly linkProjects?: (store: unknown, opts: Record<string, unknown>) => Promise<unknown>;
  readonly getGraphContext?: (store: unknown, currentName: string) => Promise<unknown>;
}) {
  mock.module("@/services.js", () => ({
    getServices: () => ({ projectGraphStore: { mocked: true } }),
  }));
  mock.module("@/features/graph/usecases/graph-link.usecase.js", () => ({
    linkProjects: overrides.linkProjects ?? (async () => ({
      edge: { from: "maestro", to: "api", relation: "consumes" },
      nodesAdded: 0,
    })),
  }));
  mock.module("@/features/graph/usecases/graph-context.usecase.js", () => ({
    getGraphContext: overrides.getGraphContext ?? (async () => ({
      currentProject: undefined,
      relationships: [],
      totalProjects: 0,
      totalEdges: 0,
    })),
  }));

  const nonce = `${Date.now()}-${Math.random()}`;
  return {
    ...(await import(`@/features/graph/commands/graph-link.command.ts?test=${nonce}`)),
    ...(await import(`@/features/graph/commands/graph-context.command.ts?test=${nonce}`)),
  };
}

afterEach(() => {
  console.log = originalConsoleLog;
  console.error = originalConsoleError;
  mock.restore();
});

describe("graph commands", () => {
  it("formats successful graph-link output", async () => {
    const captured = captureConsole();
    const { registerGraphLinkCommand } = await loadGraphCommands({
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
    registerGraphLinkCommand(program);

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
    const { registerGraphLinkCommand } = await loadGraphCommands({});
    const program = new Command().name("maestro").option("--json", "Output as JSON");
    registerGraphLinkCommand(program);

    await expect(
      program.parseAsync(["node", "maestro", "graph-link", "shared-types"]),
    ).rejects.toMatchObject({
      message: "Specify a relation: --consumes, --exposes, or --shared-types",
    });
  });

  it("formats graph-context output for unknown and linked projects", async () => {
    const captured = captureConsole();
    let firstCall = true;
    const { registerGraphContextCommand } = await loadGraphCommands({
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
              edge: { relation: "exposes", detail: "mcp" },
              project: { name: "maestro-web", path: "/code/maestro-web" },
            },
            {
              direction: "incoming",
              edge: { relation: "consumes" },
              project: { name: "shared-types", path: "/code/shared-types" },
            },
          ],
          totalProjects: 3,
          totalEdges: 2,
        };
      },
    });

    const program = new Command().name("maestro").option("--json", "Output as JSON");
    registerGraphContextCommand(program);

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
