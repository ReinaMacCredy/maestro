// room d134 (w737, d891-d895): the words a Peer reads carry no role name and
// no SLP. The pack is a second marker block inside the one pinned SLP.md
// (d892), so these tests read the shipped resource and the rendered seat, and
// never a hand-written copy of either.
import { expect, test } from "bun:test";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { parseSeatFile, seatLaunchPath, type SeatName } from "../src/plugins/slp-seats.ts";
import { prepareInstallFixture, runCli, withFixture, type Fixture } from "./helpers.ts";

const shippedRoot = join(import.meta.dir, "..", "src", "plugins", "resources");

function block(pack: string, marker: string): string {
  return (new RegExp(`<!-- slp:${marker}:begin -->([\\s\\S]*?)<!-- slp:${marker}:end -->`)
    .exec(pack)?.[1] ?? "").trim();
}

async function renderedBody(fixture: Fixture, name: SeatName): Promise<string> {
  const text = await readFile(seatLaunchPath(fixture.home, "claude", name), "utf8");
  return text.slice(text.indexOf("\n---\n") + "\n---\n".length);
}

// The seat names and protocol words room d134 removes from the Peer's eyes.
// "peer" itself is excluded: it survives in exactly one place, the pre-filled
// role=peer of the acknowledgement line (d891), which is asserted separately.
const seatWords = [
  "Team Supervisor",
  "team-supervisor",
  "Hub Supervisor",
  "hub-supervisor",
  "the Lead",
  "a Lead",
  "your Lead",
  "Role: Peer",
  "SLP",
  "seat",
  "topology",
  "generation",
  "Peer",
];

test("peer-pack-render: install renders maestro-peer from the SLP.md peer block and maestro-lead from the default block (d892, red 1)", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);

    const pack = await readFile(join(fixture.home, "maestro", "SLP.md"), "utf8");
    const sharedDefault = block(pack, "shared");
    const sharedPeer = block(pack, "shared:peer");
    expect(sharedPeer).not.toBe("");
    expect(sharedPeer).not.toBe(sharedDefault);

    const peerBody = parseSeatFile("peer.md", await readFile(join(shippedRoot, "slp", "peer.md"), "utf8"), "shipped").body;
    const leadBody = parseSeatFile("lead.md", await readFile(join(shippedRoot, "slp", "lead.md"), "utf8"), "shipped").body;
    expect(await renderedBody(fixture, "peer")).toBe(`\n${sharedPeer}\n\n${peerBody}\n`);
    expect(await renderedBody(fixture, "lead")).toBe(`\n${sharedDefault}\n\n${leadBody}\n`);
  });
}, 30_000);

test("peer-pack-words: nothing a Peer reads names a seat, a rank or the protocol, and the one surviving protocol line is the acknowledgement (d891, red 2)", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);

    const mandate = await renderedBody(fixture, "peer");
    const acknowledgement =
      "`SLP_ROLE_READY team=<team-id> generation=<n> role=peer challenge=<challenge>`";
    expect(mandate).toContain(acknowledgement);
    // Everything else is measured with the handshake lines removed, because
    // the handshake is the one place the protocol shows (d891). Dropping whole
    // lines, not one literal, so a rewrap of that paragraph cannot silently
    // widen what this lint lets through.
    const handshake = /SLP_ROLE_READY|slp team <team-id>|reply <challenge>|role=peer/;
    const withoutHandshake = mandate.split("\n").filter((line) => !handshake.test(line)).join("\n");
    for (const word of seatWords) expect({ word, mandate: withoutHandshake }).toEqual({ word, mandate: expect.not.stringContaining(word) });
  });
}, 30_000);

test("peer-pack-fallback: a Hub pack with no peer block renders the Peer from that pack's own default block and install says which block to copy (d892, red 3)", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);

    const packPath = join(fixture.home, "maestro", "SLP.md");
    const pack = await readFile(packPath, "utf8");
    const stripped = pack.replace(
      /<!-- slp:shared:peer:begin -->[\s\S]*?<!-- slp:shared:peer:end -->\n/,
      "",
    );
    expect(stripped).not.toBe(pack);
    await Bun.write(packPath, stripped);

    const again = await runCli(fixture, ["install"], { PATH: path });
    expect(again.exitCode).toBe(0);
    const report = `${again.stdout}${again.stderr}`;
    expect(report).toContain("slp:shared:peer");
    expect(report).toContain(packPath);

    // The fallback is this pack's own default block, never the shipped peer
    // block: the bytes a seat reads must come from the pack it pinned.
    expect(await renderedBody(fixture, "peer")).toContain(block(stripped, "shared"));
    expect(await renderedBody(fixture, "peer")).not.toContain(
      block(await readFile(join(shippedRoot, "SLP.md"), "utf8"), "shared:peer"),
    );
  });
}, 45_000);

// The brief's render proof: the first 30 lines a Peer will read, produced by
// the code path maestro install uses, so no live Peer is needed to see them.
test("peer-pack-first-lines: the opening of the rendered Peer mandate is the working agreement and the acknowledgement (w737 render proof)", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    const first30 = (await renderedBody(fixture, "peer")).split("\n").slice(0, 31).join("\n");
    process.stderr.write(`\n--- first 30 lines a Peer reads ---\n${first30}\n--- end ---\n`);
    expect(first30).toContain("## Working agreement");
    expect(first30).toContain("SLP_ROLE_READY team=<team-id> generation=<n> role=peer challenge=<challenge>");
    expect(first30).not.toContain("You belong to one supervised team generation");
  });
}, 30_000);
