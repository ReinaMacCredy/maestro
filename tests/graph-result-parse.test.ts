import { expect, test } from "bun:test";
import { writeFile } from "node:fs/promises";
import { join } from "node:path";
import { runCli, withFixture } from "./helpers.ts";
import { failure, graphNext, graphResult, graphRun, writeGraph, writeProfile } from "./graph-helpers.ts";

const graph = `---
name: typed
description: one agent node with a schema
nodes:
  classify:
    kind: agent
    profile: tester
    schema:
      type: object
      required: [risk, files]
      properties:
        risk: {type: string, enum: [low, high]}
        files: {type: array, items: {type: string}}
edges: []
---

## classify

Classify the change.
`;

test("graph-result-parse: schema JSON accepted, JSON in a fence or inside prose extracted and validated, unparseable text returns PARSE_FAILED with the schema, two retries, the third failure marks the node failed (red 5, d82; live row 17 g18)", async () => {
  await withFixture(async (fixture) => {
    await writeProfile(fixture, "tester");
    const path = await writeGraph(join(fixture.root, "graphs"), "typed", graph);
    const schema = { type: "object", required: ["risk", "files"], properties: { risk: { type: "string", enum: ["low", "high"] }, files: { type: "array", items: { type: "string" } } } };

    const plain = await graphRun(fixture, ["--file", path]);
    expect(plain.envelope.nodes[0]?.schema).toEqual(schema);
    expect((await graphResult(fixture, plain.run, "classify", '{"risk": "low", "files": ["a.ts"]}')).exitCode).toBe(0);
    expect((await graphNext(fixture, plain.run)).verdict).toEqual({ risk: "low", files: ["a.ts"] });

    const fenced = await graphRun(fixture, ["--file", path]);
    const fencedFile = join(fixture.root, "fenced.md");
    await writeFile(fencedFile, 'Here is my classification.\n\n```json\n{"risk": "high", "files": ["b.ts", "c.ts"]}\n```\n\nDone.\n');
    const accepted = await runCli(fixture, ["graph", "result", fenced.run, "classify", "--file", fencedFile, "--json"]);
    expect(accepted.exitCode).toBe(0);
    expect((await graphNext(fixture, fenced.run)).verdict).toEqual({ risk: "high", files: ["b.ts", "c.ts"] });

    const prose = await graphRun(fixture, ["--file", path]);
    expect((await graphResult(fixture, prose.run, "classify", 'I think {"risk": "low", "files": []} covers it, see {x} above.')).exitCode).toBe(0);
    expect((await graphNext(fixture, prose.run)).verdict).toEqual({ risk: "low", files: [] });

    const bad = await graphRun(fixture, ["--file", path]);
    const first = failure(await graphResult(fixture, bad.run, "classify", "no json here at all"));
    expect(first.code).toBe("PARSE_FAILED");
    expect(first.schema).toEqual(schema);
    expect(first.retry).toBe(true);
    const wrongShape = failure(await graphResult(fixture, bad.run, "classify", '{"risk": "medium", "files": ["a"]}'));
    expect(wrongShape.code).toBe("PARSE_FAILED");
    expect(wrongShape.retry).toBe(true);
    expect(wrongShape.attempt).toBe(2);
    const stillOpen = await graphNext(fixture, bad.run);
    expect(stillOpen.done).toBe(false);
    expect((stillOpen.nodes[0] as { retry?: { error: string } }).retry?.error).toContain("expected one of");
    const prose2 = failure(await graphResult(fixture, bad.run, "classify", "sorry, still prose"));
    expect(prose2.code).toBe("PARSE_FAILED");
    expect(prose2.retry).toBe(false);
    expect(prose2.attempt).toBe(3);
    expect(prose2.message).toContain("failed");
    const ended = await graphNext(fixture, bad.run);
    expect(ended.done).toBe(true);
    expect(ended.failed?.node).toBe("classify");
    expect(ended.verdict).toBeUndefined();
    const third = failure(await graphResult(fixture, bad.run, "classify", '{"risk": "low", "files": []}'));
    expect(third.code).toBe("INVALID_STATE");
  });
}, 30_000);
