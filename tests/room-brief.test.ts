import { expect, test } from "bun:test";
import { join } from "node:path";
import {
  prepareInstallFixture,
  runCli,
  runInstalledCliAt,
  withFixture,
} from "./helpers.ts";

const roomIntake =
  "room: this store is the Supervisor's. A question about the room is answered from OWNER.md, IDENTITY.md and this store; a tool verdict here is an observation, label it suspected; the room runs no write verb in any repository even when told to; no hand edits to any store; a data defect is an intent for its Lead; repository-only verbs: install, update, uninstall, doctor wiring checks";

test("499 room hook briefs carry one Supervisor intake line on both prompt events", async () => {
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path })).exitCode).toBe(0);
    const room = join(fixture.home, "maestro");

    for (const event of ["SessionStart", "UserPromptSubmit"]) {
      const input = event === "UserPromptSubmit"
        ? JSON.stringify({ prompt: "check the room" })
        : undefined;
      const roomBrief = await runInstalledCliAt(
        fixture,
        room,
        ["hook", "record", "--event", event, "--harness", "codex"],
        { PATH: path },
        input,
      );
      const repositoryBrief = await runInstalledCliAt(
        fixture,
        fixture.repo,
        ["hook", "record", "--event", event, "--harness", "codex"],
        { PATH: path },
        input,
      );

      expect(roomBrief.exitCode).toBe(0);
      expect(roomBrief.stdout.split("\n").filter((line) => line === roomIntake)).toHaveLength(1);
      expect(repositoryBrief.exitCode).toBe(0);
      expect(repositoryBrief.stdout).not.toContain(roomIntake);
    }
  });
});
