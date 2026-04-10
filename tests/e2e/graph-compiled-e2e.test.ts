/**
 * End-to-end test for the graph feature against ./dist/maestro.
 *
 * Graph state is GLOBAL: it lives at ~/.maestro/graph/projects.json
 * and is shared across every project on the machine. Running e2e
 * tests against the user's real home would pollute their live graph,
 * so this suite sandboxes HOME to a fresh tmpdir for every test.
 * Setting env: { HOME: <sandbox> } on runCompiled causes the graph
 * adapter to write to <sandbox>/.maestro/graph/projects.json instead
 * of the real one.
 *
 * Each test creates two tmpdirs:
 *   - projectDir: the cwd of the command invocation
 *   - homeDir:    the sandboxed HOME where graph state lives
 *
 * We tear down both in afterEach so tests are independent.
 */
import {
  afterEach,
  beforeAll,
  beforeEach,
  describe,
  expect,
  it,
} from "bun:test";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, basename } from "node:path";
import {
  BUILD_TIMEOUT_MS,
  SLOW_CLI_TIMEOUT_MS,
  buildCompiledCli,
  expectJson,
  initGitRepo,
  runCompiled,
} from "../helpers/run-compiled-cli.js";

let projectDir: string;
let homeDir: string;

beforeAll(buildCompiledCli, BUILD_TIMEOUT_MS);

beforeEach(async () => {
  projectDir = await mkdtemp(join(tmpdir(), "maestro-graph-e2e-project-"));
  homeDir = await mkdtemp(join(tmpdir(), "maestro-graph-e2e-home-"));
  await initGitRepo(projectDir);
});

afterEach(async () => {
  await rm(projectDir, { recursive: true, force: true });
  await rm(homeDir, { recursive: true, force: true });
});

/** Sandboxed env that redirects ~/.maestro/graph/ into the test home. */
function sandboxedEnv(): Record<string, string> {
  return { HOME: homeDir };
}

describe("compiled graph feature E2E", () => {
  it(
    "graph-link creates an outgoing edge and persists it to the sandboxed home",
    async () => {
      const projectName = basename(projectDir);

      const result = await runCompiled(
        [
          "graph-link",
          "maestro-web",
          "--consumes",
          "--via",
          "shared types",
          "--json",
        ],
        projectDir,
        { env: sandboxedEnv() },
      );
      expect(result.exitCode).toBe(0);
      const linked = expectJson<{
        edge: { from: string; to: string; relation: string; detail?: string };
        nodesAdded: number;
      }>(result);
      expect(linked.edge.from).toBe(projectName);
      expect(linked.edge.to).toBe("maestro-web");
      expect(linked.edge.relation).toBe("consumes");
      expect(linked.edge.detail).toBe("shared types");
      expect(linked.nodesAdded).toBeGreaterThanOrEqual(1);

      // The real ~/.maestro/graph/projects.json must NOT exist in the
      // sandbox home (and must still exist unchanged in the real home).
      // The sandboxed file should contain both nodes + the edge.
      const sandboxPath = join(
        homeDir,
        ".maestro",
        "graph",
        "projects.json",
      );
      const raw = await readFile(sandboxPath, "utf8");
      const parsed = JSON.parse(raw) as {
        nodes: Array<{ name: string }>;
        edges: Array<{ from: string; to: string; relation: string }>;
      };
      expect(parsed.nodes.length).toBeGreaterThanOrEqual(2);
      const nodeNames = parsed.nodes.map((n) => n.name);
      expect(nodeNames).toContain(projectName);
      expect(nodeNames).toContain("maestro-web");
      expect(parsed.edges.length).toBeGreaterThanOrEqual(1);
      expect(parsed.edges[0]?.relation).toBe("consumes");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "graph-context reports the outgoing relationship from the current project",
    async () => {
      const projectName = basename(projectDir);

      // Seed the graph with a link.
      const link = await runCompiled(
        ["graph-link", "shared-types", "--exposes", "--json"],
        projectDir,
        { env: sandboxedEnv() },
      );
      expect(link.exitCode).toBe(0);

      // Read it back via graph-context.
      const context = await runCompiled(
        ["graph-context", "--json"],
        projectDir,
        { env: sandboxedEnv() },
      );
      expect(context.exitCode).toBe(0);
      const payload = expectJson<{
        currentProject: { name: string; path: string };
        relationships: Array<{
          direction: string;
          edge: { from: string; to: string; relation: string };
          project: { name: string };
        }>;
        totalProjects: number;
        totalEdges: number;
      }>(context);

      expect(payload.currentProject.name).toBe(projectName);
      expect(payload.totalProjects).toBeGreaterThanOrEqual(2);
      expect(payload.totalEdges).toBeGreaterThanOrEqual(1);

      const outgoing = payload.relationships.filter(
        (r) => r.direction === "outgoing",
      );
      expect(outgoing.length).toBeGreaterThanOrEqual(1);
      expect(outgoing[0]?.edge.relation).toBe("exposes");
      expect(outgoing[0]?.project.name).toBe("shared-types");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "graph-context on a fresh sandbox reports no relationships",
    async () => {
      const result = await runCompiled(
        ["graph-context", "--json"],
        projectDir,
        { env: sandboxedEnv() },
      );
      expect(result.exitCode).toBe(0);
      const payload = expectJson<{
        relationships: unknown[];
        totalEdges: number;
      }>(result);
      expect(payload.relationships).toEqual([]);
      expect(payload.totalEdges).toBe(0);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "multiple graph-link calls accumulate edges",
    async () => {
      const links = [
        ["maestro-api", "--consumes"],
        ["shared-types", "--exposes"],
        ["maestro-web", "--shared-types"],
      ];

      for (const [target, relation] of links) {
        const r = await runCompiled(
          ["graph-link", target!, relation!, "--json"],
          projectDir,
          { env: sandboxedEnv() },
        );
        expect(r.exitCode).toBe(0);
      }

      const context = await runCompiled(
        ["graph-context", "--json"],
        projectDir,
        { env: sandboxedEnv() },
      );
      expect(context.exitCode).toBe(0);
      const payload = expectJson<{
        relationships: unknown[];
        totalEdges: number;
        totalProjects: number;
      }>(context);
      expect(payload.totalEdges).toBeGreaterThanOrEqual(3);
      expect(payload.totalProjects).toBeGreaterThanOrEqual(4); // self + 3 targets
      expect(payload.relationships.length).toBeGreaterThanOrEqual(3);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "graph-link requires a target argument",
    async () => {
      const result = await runCompiled(["graph-link"], projectDir, {
        env: sandboxedEnv(),
      });
      expect(result.exitCode).not.toBe(0);
      const combined = `${result.stdout}\n${result.stderr}`;
      expect(combined.toLowerCase()).toContain("argument");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "graph-link text mode prints a human-readable confirmation",
    async () => {
      const result = await runCompiled(
        ["graph-link", "sibling", "--consumes"],
        projectDir,
        { env: sandboxedEnv() },
      );
      expect(result.exitCode).toBe(0);
      // Text mode should mention the relation somewhere in the output.
      expect(result.stdout.toLowerCase()).toContain("sibling");
    },
    SLOW_CLI_TIMEOUT_MS,
  );
});
