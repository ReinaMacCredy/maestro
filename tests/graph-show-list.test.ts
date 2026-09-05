import { expect, test } from "bun:test";
import { realpath } from "node:fs/promises";
import { join } from "node:path";
import { runCli, withFixture } from "./helpers.ts";
import { data, failure, homeGraphs, passthroughGraph, repoGraphs, writeGraph } from "./graph-helpers.ts";

test("graph-show-list: show renders a graph from any of the three locations; list shows origin and shadowing across repo, room and shipped (red 12, d81, d100)", async () => {
  await withFixture(async (fixture) => {
    const shippedPath = join(import.meta.dir, "..", "src", "plugins", "graphs", "review-gate.md");
    const shipped = data<{ graph: { nodes: Array<{ kind: string }> }; origin: string; path: string; text: string }>(
      await runCli(fixture, ["graph", "show", "review-gate", "--json"]),
    );
    expect(shipped.origin).toBe("shipped");
    expect(shipped.path).toBe(shippedPath);
    expect(shipped.text).toBe(await Bun.file(shippedPath).text());
    expect(new Set(shipped.graph.nodes.map((node) => node.kind))).toEqual(new Set(["function", "agent", "router", "join", "foreach"]));

    const before = data<{ graphs: Array<{ name: string; origin: string; path: string; shadows: Array<{ origin: string }> }> }>(
      await runCli(fixture, ["graph", "list", "--json"]),
    );
    expect(before.graphs).toEqual([{ name: "review-gate", origin: "shipped", path: shippedPath, shadows: [] }]);

    await writeGraph(homeGraphs(fixture), "review-gate", passthroughGraph.replace("name: passthrough", "name: review-gate"));
    await writeGraph(homeGraphs(fixture), "sweep", passthroughGraph.replace("name: passthrough", "name: sweep"));
    const home = data<{ origin: string; path: string }>(await runCli(fixture, ["graph", "show", "review-gate", "--json"]));
    expect(home.origin).toBe("home");
    expect(home.path).toBe(join(homeGraphs(fixture), "review-gate.md"));

    await writeGraph(repoGraphs(fixture), "review-gate", passthroughGraph.replace("name: passthrough", "name: review-gate"));
    const repo = data<{ origin: string; path: string }>(await runCli(fixture, ["graph", "show", "review-gate", "--json"]));
    expect(repo.origin).toBe("repo");
    expect(await realpath(repo.path)).toBe(await realpath(join(repoGraphs(fixture), "review-gate.md")));

    const listed = data<{ graphs: Array<{ name: string; origin: string; shadows: Array<{ origin: string; path: string }> }> }>(
      await runCli(fixture, ["graph", "list", "--json"]),
    );
    expect(listed.graphs.map((graph) => [graph.name, graph.origin, graph.shadows.map((shadow) => shadow.origin)])).toEqual([
      ["review-gate", "repo", ["home", "shipped"]],
      ["sweep", "home", []],
    ]);
    const text = await runCli(fixture, ["graph", "list"]);
    expect(text.stdout).toContain("review-gate\trepo\t");
    expect(text.stdout).toContain("shadows: home, shipped");
    expect(text.stdout).toContain("sweep\thome\t");
    expect(failure(await runCli(fixture, ["graph", "show", "missing", "--json"])).code).toBe("GRAPH_NOT_FOUND");
  });
}, 30_000);
