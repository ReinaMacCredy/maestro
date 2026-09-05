import { expect, test } from "bun:test";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { initializeGitRepository, runCli, runTool, withFixture } from "./helpers.ts";
import { data, graphNext, graphResult, graphRun } from "./graph-helpers.ts";

// The fix-loop half of red 16 (function, static fan-out, loop-back with
// max_rounds, human, writing agent) ships with the second close (d100).
test("graph-presets: review-gate loads, covers function, agent, router, join and foreach, and runs to a verdict under fed results on both tiers (red 16 review-gate half, d81, item 10)", async () => {
  await withFixture(async (fixture) => {
    await initializeGitRepository(fixture.repo);
    await Bun.write(join(fixture.repo, "src.ts"), "export const answer = 42;\n");
    expect((await runTool(["git", "add", "src.ts"], fixture.repo)).exitCode).toBe(0);
    expect(
      (await runTool(["git", "-c", "user.name=t", "-c", "user.email=t@example.invalid", "commit", "-q", "-m", "answer"], fixture.repo)).exitCode,
    ).toBe(0);

    const shown = data<{ graph: { edges: unknown[]; nodes: Array<{ kind: string; node: string }> } }>(
      await runCli(fixture, ["graph", "show", "review-gate", "--json"]),
    );
    expect(new Set(shown.graph.nodes.map((node) => node.kind))).toEqual(new Set(["function", "agent", "router", "join", "foreach"]));

    // Tier light on a one-subsystem, no-boundary diff: simplify lens only.
    const light = await graphRun(fixture, ["review-gate", "range=HEAD~1..HEAD"]);
    expect(light.envelope.nodes.map((node) => node.ref)).toEqual(["classify"]);
    expect(light.envelope.nodes[0]?.profile).toBe("classifier");
    expect(light.envelope.nodes[0]?.prompt).toContain("src.ts | 1 +");
    expect(light.envelope.nodes[0]?.prompt).toContain("git diff HEAD~1..HEAD");
    expect(light.envelope.executor).toBe("subagent");
    const classification = { files: ["src.ts"], subsystems: ["src"], touchesTrustBoundary: false, touchesSchemaOrMigration: false, touchesAuthSecretsOrInput: false, summary: "adds a constant" };
    expect((await graphResult(fixture, light.run, "classify", JSON.stringify(classification))).exitCode).toBe(0);
    const lenses = await graphNext(fixture, light.run);
    expect(lenses.nodes.map((node) => [node.ref, node.profile])).toEqual([["review-simplify", "reviewer-simplify"]]);
    expect((await graphResult(fixture, light.run, "review-simplify", JSON.stringify({ findings: [] }))).exitCode).toBe(0);
    const synth = await graphNext(fixture, light.run);
    expect(synth.nodes.map((node) => [node.ref, node.profile])).toEqual([["verdict", "synthesizer"]]);
    expect(synth.nodes[0]?.prompt).toContain('"items": []');
    const lightVerdict = { verdict: "pass", confirmed: [], refuted: [], summary: "nothing to report" };
    expect((await graphResult(fixture, light.run, "verdict", JSON.stringify(lightVerdict))).exitCode).toBe(0);
    const lightDone = await graphNext(fixture, light.run);
    expect(lightDone.done).toBe(true);
    expect(lightDone.verdict).toEqual(lightVerdict);

    // Tier full on a broad diff: correctness, regression, contracts and security fan out; one refuter per deduplicated finding.
    const full = await graphRun(fixture, ["review-gate", "range=HEAD~1..HEAD", "tier=full"]);
    const broad = { ...classification, subsystems: ["src", "kernel"], touchesTrustBoundary: true, touchesAuthSecretsOrInput: true };
    expect((await graphResult(fixture, full.run, "classify", JSON.stringify(broad))).exitCode).toBe(0);
    const fanout = await graphNext(fixture, full.run);
    expect(fanout.nodes.map((node) => node.ref)).toEqual(["review-correctness", "review-regression", "review-contracts", "review-security"]);
    expect(fanout.nodes.map((node) => node.profile)).toEqual(["reviewer-correctness", "reviewer-regression", "reviewer-contracts", "reviewer-security"]);
    const finding = (summary: string, line = 1) => ({ file: "src.ts", line, summary, evidence: "the diff" });
    expect((await graphResult(fixture, full.run, "review-correctness", JSON.stringify({ findings: [finding("off by one")] }))).exitCode).toBe(0);
    expect((await graphResult(fixture, full.run, "review-regression", JSON.stringify({ findings: [finding("same line again")] }))).exitCode).toBe(0);
    expect((await graphResult(fixture, full.run, "review-contracts", JSON.stringify({ findings: [] }))).exitCode).toBe(0);
    expect((await graphResult(fixture, full.run, "review-security", JSON.stringify({ findings: [finding("secret in log", 7)] }))).exitCode).toBe(0);
    const refuters = await graphNext(fixture, full.run);
    expect(refuters.nodes.map((node) => [node.ref, node.profile])).toEqual([["refute@0", "refuter"], ["refute@1", "refuter"]]);
    expect(refuters.nodes[0]?.prompt).toContain("It came from the review-correctness lens.");
    expect(refuters.nodes[0]?.prompt).toContain("Claim: off by one");
    expect(refuters.nodes[1]?.prompt).toContain("line 7");
    expect((await graphResult(fixture, full.run, "refute@0", JSON.stringify({ refuted: true, reason: "handled by the caller" }))).exitCode).toBe(0);
    expect((await graphResult(fixture, full.run, "refute@1", JSON.stringify({ refuted: false, reason: "the log line prints the token" }))).exitCode).toBe(0);
    const finalNode = await graphNext(fixture, full.run);
    expect(finalNode.nodes.map((node) => node.ref)).toEqual(["verdict"]);
    expect(finalNode.nodes[0]?.prompt).toContain('"provenance": [\n        "review-regression"\n      ]');
    expect(finalNode.nodes[0]?.prompt).toContain('"instance": "1"');
    const fullVerdict = {
      verdict: "fail",
      confirmed: [{ file: "src.ts", line: 7, summary: "secret in log", lens: "review-security" }],
      refuted: [{ file: "src.ts", line: 1, summary: "off by one", reason: "handled by the caller" }],
      summary: "one confirmed security finding",
    };
    expect((await graphResult(fixture, full.run, "verdict", JSON.stringify(fullVerdict))).exitCode).toBe(0);
    const fullDone = await graphNext(fixture, full.run);
    expect(fullDone.done).toBe(true);
    expect(fullDone.verdict).toEqual(fullVerdict);
    const trace = data<{ events: Array<{ payload: { node?: string }; type: string }> }>(await runCli(fixture, ["trace", full.run, "--json"]));
    expect(trace.events.filter((event) => event.type === "graph.node.skipped").map((event) => event.payload.node)).toEqual(["review-simplify"]);
    expect(trace.events.filter((event) => event.type === "graph.node.done" && event.payload.node === "diffstat")).toHaveLength(1);

    const packageJson = JSON.parse(await readFile(join(import.meta.dir, "..", "package.json"), "utf8")) as Record<string, unknown>;
    expect("dependencies" in packageJson).toBe(false);
  });
}, 30_000);
