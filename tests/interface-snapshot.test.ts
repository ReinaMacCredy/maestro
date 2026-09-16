import { expect, test } from "bun:test";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { renderSurfaces, snapshotDirectory, surfaces } from "./snapshot-surfaces.ts";

// The interface gate for the Hermes-shape cleanup (w732): every worker runs this
// before returning, and a difference is a changed interface, not a changed
// snapshot. MAESTRO_SNAPSHOT_UPDATE=1 rewrites the files, the way
// MAESTRO_GOLDEN_UPDATE=1 rewrites the scenario goldens.
const recording = process.env["MAESTRO_SNAPSHOT_UPDATE"] === "1";

test("671 every interface surface matches its committed snapshot (w732)", async () => {
  const rendered = await renderSurfaces();
  expect(rendered.size).toBe(surfaces.length);

  if (recording) {
    await mkdir(snapshotDirectory, { recursive: true });
    for (const [name, body] of rendered) {
      await writeFile(join(snapshotDirectory, name), body);
    }
    return;
  }

  for (const [name, body] of rendered) {
    const expected = await readFile(join(snapshotDirectory, name), "utf8");
    // The name rides along in the compared value so a failure names the surface
    // instead of printing two anonymous walls of text.
    expect(`${name}\n${body}`).toBe(`${name}\n${expected}`);
  }
}, 300_000);

test("672 the interface snapshot covers every surface it claims (w732)", async () => {
  for (const surface of surfaces) {
    const body = await readFile(join(snapshotDirectory, surface.name), "utf8");
    expect(body.length).toBeGreaterThan(0);
  }
});
