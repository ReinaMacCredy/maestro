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

test("graph-presets: fix-loop and council load, the union of review-gate and fix-loop node and edge kinds is the full set, and both run to a verdict under fed results (rest of red 16, d81, d837)", async () => {
  await withFixture(async (fixture) => {
    await initializeGitRepository(fixture.repo);
    const graphOf = async (name: string) =>
      data<{ graph: { edges: Array<{ max_rounds?: number; when?: unknown }>; nodes: Array<{ kind: string; node: string; writes?: boolean }> } }>(
        await runCli(fixture, ["graph", "show", name, "--json"]),
      ).graph;
    const review = await graphOf("review-gate");
    const fixLoop = await graphOf("fix-loop");
    const kinds = new Set([...review.nodes, ...fixLoop.nodes].map((node) => node.kind));
    expect([...kinds].sort()).toEqual(["agent", "foreach", "function", "human", "join", "router"]);
    const edges = [...review.edges, ...fixLoop.edges];
    expect(edges.some((edge) => edge.when === undefined && edge.max_rounds === undefined)).toBe(true);
    expect(review.edges.some((edge) => edge.when !== undefined)).toBe(true);
    expect(fixLoop.edges.some((edge) => edge.max_rounds !== undefined)).toBe(true);
    expect(fixLoop.nodes.some((node) => node.writes)).toBe(true);
    const listed = data<{ graphs: Array<{ name: string }> }>(await runCli(fixture, ["graph", "list", "--json"])).graphs.map((graph) => graph.name);
    expect(listed).toEqual(["council", "fix-loop", "review-gate"]);

    // fix-loop: one finding in round one, the fixer runs alone, the loop re-runs the check and the lenses, round two is clean and the human confirms.
    const loop = await graphRun(fixture, ["fix-loop", "scope=the off-by-one in src.ts", "check=true"]);
    expect(loop.envelope.nodes.map((node) => node.ref)).toEqual(["review-bugs", "review-regressions"]);
    expect(loop.envelope.state.verify).toEqual({ passed: true });
    expect(loop.envelope.nodes[0]?.prompt).toContain("passed=true");
    const finding = { file: "src.ts", line: 3, summary: "off by one", evidence: "loop bound" };
    expect((await graphResult(fixture, loop.run, "review-bugs", JSON.stringify({ findings: [finding] }))).exitCode).toBe(0);
    expect((await graphResult(fixture, loop.run, "review-regressions", JSON.stringify({ findings: [] }))).exitCode).toBe(0);
    const fixing = await graphNext(fixture, loop.run);
    expect(fixing.nodes.map((node) => [node.ref, node.profile, node.round])).toEqual([["fix", "fixer", 1]]);
    expect(fixing.nodes[0]?.prompt).toContain('"summary": "off by one"');
    expect((await graphResult(fixture, loop.run, "fix", JSON.stringify({ files: ["src.ts"], summary: "bounded the loop", unresolved: [] }), ["--files", "src.ts"])).exitCode).toBe(0);
    const roundTwo = await graphNext(fixture, loop.run);
    expect(roundTwo.nodes.map((node) => [node.ref, node.round])).toEqual([["review-bugs", 2], ["review-regressions", 2]]);
    for (const ref of ["review-bugs", "review-regressions"]) {
      expect((await graphResult(fixture, loop.run, ref, JSON.stringify({ findings: [] }))).exitCode).toBe(0);
    }
    const confirming = await graphNext(fixture, loop.run);
    expect(confirming.nodes.map((node) => [node.ref, node.kind])).toEqual([["confirm", "human"]]);
    expect(confirming.nodes[0]?.prompt).toContain("Round 2");
    expect((await graphResult(fixture, loop.run, "confirm", "approved")).exitCode).toBe(0);
    const loopDone = await graphNext(fixture, loop.run);
    expect(loopDone.done).toBe(true);
    expect(loopDone.verdict).toEqual({ confirm: "approved", escalate: null });
    const loopTrace = data<{ events: Array<{ payload: Record<string, unknown>; type: string }> }>(await runCli(fixture, ["trace", loop.run, "--json"]));
    expect(loopTrace.events.filter((event) => event.type === "graph.loop")).toHaveLength(1);
    expect(loopTrace.events.filter((event) => event.type === "graph.node.done" && event.payload.node === "verify")).toHaveLength(2);

    // council at tier lens: one seat, unanimity, the premise verifier, the Lead's draft, no audit, the Lead's verdict.
    const report = (position: string) => JSON.stringify({ position, claims: [{ id: "C1", type: "FACT", claim: "x", evidence: "y" }], falsifier: "z" });
    const lens = await graphRun(fixture, ["council", "brief=CASE_ID: demo", "tier=lens"]);
    expect(lens.envelope.nodes.map((node) => [node.ref, node.profile])).toEqual([["independent", "independent"]]);
    expect(lens.envelope.nodes[0]?.prompt.startsWith("seat execution mode:")).toBe(true);
    expect((await graphResult(fixture, lens.run, "independent", report("keep"))).exitCode).toBe(0);
    const modeling = await graphNext(fixture, lens.run);
    expect(modeling.nodes.map((node) => [node.ref, node.profile])).toEqual([["model", "classifier"]]);
    expect((await graphResult(fixture, lens.run, "model", JSON.stringify({ unanimous: true, premise: "the store is single-writer", disputed: [] }))).exitCode).toBe(0);
    const premise = await graphNext(fixture, lens.run);
    expect(premise.nodes.map((node) => [node.ref, node.profile])).toEqual([["premise", "verifier"]]);
    expect(premise.nodes[0]?.prompt).toContain("PROPOSITION: the store is single-writer");
    const verification = { proposition: "the store is single-writer", mandate: "disconfirming evidence", observations: "none found", result: "verified" };
    expect((await graphResult(fixture, lens.run, "premise", JSON.stringify(verification))).exitCode).toBe(0);
    const drafting = await graphNext(fixture, lens.run);
    expect(drafting.nodes.map((node) => [node.ref, node.kind])).toEqual([["draft", "human"]]);
    expect((await graphResult(fixture, lens.run, "draft", "keep; dissent: none")).exitCode).toBe(0);
    const deciding = await graphNext(fixture, lens.run);
    expect(deciding.nodes.map((node) => [node.ref, node.kind])).toEqual([["verdict", "human"]]);
    expect((await graphResult(fixture, lens.run, "verdict", "keep (d-demo)")).exitCode).toBe(0);
    const lensDone = await graphNext(fixture, lens.run);
    expect(lensDone.done).toBe(true);
    expect(lensDone.verdict).toBe("keep (d-demo)");
    const lensSkipped = data<{ events: Array<{ payload: { node?: string }; type: string }> }>(await runCli(fixture, ["trace", lens.run, "--json"]))
      .events.filter((event) => event.type === "graph.node.skipped").map((event) => event.payload.node);
    expect(lensSkipped).toEqual(expect.arrayContaining(["challenger", "specialist", "disputed", "auditor"]));

    // council at tier high-risk: three seats sealed, a dispute, one bounded verifier per proposition, cross-examination, draft, mandatory audit, verdict.
    const high = await graphRun(fixture, ["council", "brief=CASE_ID: risky", "tier=high-risk"]);
    expect(high.envelope.nodes.map((node) => node.ref)).toEqual(["independent", "challenger", "specialist"]);
    expect((await graphResult(fixture, high.run, "independent", report("split"))).exitCode).toBe(0);
    expect((await graphResult(fixture, high.run, "challenger", report("merge"))).exitCode).toBe(0);
    expect((await graphNext(fixture, high.run)).nodes.map((node) => node.ref)).toEqual(["specialist"]);
    expect((await graphResult(fixture, high.run, "specialist", report("split"))).exitCode).toBe(0);
    const highModel = await graphNext(fixture, high.run);
    expect(highModel.nodes.map((node) => node.ref)).toEqual(["model"]);
    expect(highModel.nodes[0]?.prompt).toContain('"producer": "challenger"');
    const disputed = [
      { id: "P1", proposition: "merging doubles write latency", mandate: "supporting evidence" },
      { id: "P2", proposition: "the split needs no migration", mandate: "disconfirming evidence" },
    ];
    expect((await graphResult(fixture, high.run, "model", JSON.stringify({ unanimous: false, premise: "the fork", disputed }))).exitCode).toBe(0);
    const verifying = await graphNext(fixture, high.run);
    expect(verifying.nodes.map((node) => [node.ref, node.profile])).toEqual([["verify@P1", "verifier"], ["verify@P2", "verifier"]]);
    expect(verifying.nodes[1]?.prompt).toContain("PROPOSITION (P2): the split needs no migration");
    for (const [ref, result] of [["verify@P1", "falsified"], ["verify@P2", "verified"]] as const) {
      expect((await graphResult(fixture, high.run, ref, JSON.stringify({ ...verification, result }))).exitCode).toBe(0);
    }
    const crossing = await graphNext(fixture, high.run);
    expect(crossing.nodes.map((node) => [node.ref, node.profile])).toEqual([["cross", "challenger"]]);
    expect(crossing.nodes[0]?.prompt).toContain('"instance": "P1"');
    const responses = { responses: [{ id: "P1", response: "CONCEDE", reason: "the measurement holds" }] };
    expect((await graphResult(fixture, high.run, "cross", JSON.stringify(responses))).exitCode).toBe(0);
    const responding = await graphNext(fixture, high.run);
    expect(responding.nodes.map((node) => [node.ref, node.profile])).toEqual([["response", "independent"]]);
    expect((await graphResult(fixture, high.run, "response", JSON.stringify({ responses: [{ id: "P1", response: "MAINTAIN", reason: "latency is not the constraint" }] }))).exitCode).toBe(0);
    const highDraft = await graphNext(fixture, high.run);
    expect(highDraft.nodes.map((node) => node.ref)).toEqual(["draft"]);
    expect(highDraft.nodes[0]?.prompt).toContain("CONCEDE");
    expect((await graphResult(fixture, high.run, "draft", "split; dissent: merge")).exitCode).toBe(0);
    const auditing = await graphNext(fixture, high.run);
    expect(auditing.nodes.map((node) => [node.ref, node.profile])).toEqual([["auditor", "auditor"]]);
    expect(auditing.nodes[0]?.prompt).toContain("split; dissent: merge");
    expect((await graphResult(fixture, high.run, "auditor", JSON.stringify({ result: "CLEAR", findings: [] }))).exitCode).toBe(0);
    const finalHigh = await graphNext(fixture, high.run);
    expect(finalHigh.nodes.map((node) => node.ref)).toEqual(["verdict"]);
    expect(finalHigh.nodes[0]?.prompt).toContain('"result": "CLEAR"');
    expect((await graphResult(fixture, high.run, "verdict", "split (d-risky)")).exitCode).toBe(0);
    const highDone = await graphNext(fixture, high.run);
    expect(highDone.done).toBe(true);
    expect(highDone.verdict).toBe("split (d-risky)");
    const highSkipped = data<{ events: Array<{ payload: { node?: string }; type: string }> }>(await runCli(fixture, ["trace", high.run, "--json"]))
      .events.filter((event) => event.type === "graph.node.skipped").map((event) => event.payload.node);
    expect(highSkipped).toEqual(["premise"]);
  });
}, 60_000);
