import { expect, test } from "bun:test";
import { join } from "node:path";
import { withFixture } from "./helpers.ts";
import { graphResult, graphRun, writeGraph, writeProfile } from "./graph-helpers.ts";

// Live row 17 defect 5 (lab run w1, items w34 and w35): the Lead's work add
// carried only the prompt, whose "matching the schema" named a schema the
// Peer never saw. d838: the brief carries the schema block.
const graph = `---
name: briefed
description: one typed and one untyped agent node
nodes:
  typed:
    kind: agent
    profile: tester
    schema:
      type: object
      required: [findings, summary]
      properties:
        summary: {type: string}
        findings:
          type: array
          items:
            type: object
            required: [file, line, evidence]
            properties:
              file: {type: string}
              line: {type: integer}
              evidence: {type: string}
  plain: {kind: agent, profile: tester}
  ask: {kind: human}
edges:
  - {from: typed, to: ask}
  - {from: plain, to: ask}
---

## typed

Report findings for the diff. Answer with one JSON object matching the schema.

## plain

Say hello.

## ask

Approve?
`;

// Live row 17 (lab g18): every sonnet node failed the schema once even with
// the schema block in the brief. The brief now leads with a plain sentence
// naming the required keys, top level and inside array items.
test("graph-brief: every agent node in the envelope carries a brief that is the prompt plus a key sentence and the schema block when a schema is declared, and the prompt alone otherwise (live row 17 defect 5, d838 and its successor)", async () => {
  await withFixture(async (fixture) => {
    await writeProfile(fixture, "tester");
    const path = await writeGraph(join(fixture.root, "graphs"), "briefed", graph);
    const { run, envelope } = await graphRun(fixture, ["--file", path]);
    const typed = envelope.nodes.find((node) => node.ref === "typed") as { brief?: string; prompt: string; schema?: unknown } | undefined;
    const plain = envelope.nodes.find((node) => node.ref === "plain") as { brief?: string; prompt: string } | undefined;
    expect(typed?.prompt).toBe("Report findings for the diff. Answer with one JSON object matching the schema.");
    const sentence = "Return one JSON object with exactly these keys: findings, summary; nested findings objects need file, line, evidence; no prose before or after.";
    expect(typed?.brief).toBe(
      `${typed?.prompt}\n\n${sentence}\n\nAnswer with one JSON object matching this schema:\n\n\`\`\`json\n${JSON.stringify(typed?.schema, null, 2)}\n\`\`\``,
    );
    expect(plain?.brief).toBe("Say hello.");
    expect((await graphResult(fixture, run, "typed", '{"findings": [], "summary": "none"}')).exitCode).toBe(0);
    expect((await graphResult(fixture, run, "plain", "hello")).exitCode).toBe(0);
    const human = (await graphRun(fixture, ["--file", path])).envelope;
    void human;
  });
}, 30_000);
