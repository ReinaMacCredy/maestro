import { expect, test } from "bun:test";
import { mkdir } from "node:fs/promises";
import { join } from "node:path";
import { fakeHerdrCommands, installFakeHerdr } from "./fake-herdr.ts";
import { runCliAt, withFixture, type Fixture } from "./helpers.ts";
import { coreTriggerRules } from "../src/plugins/team-observer.ts";

function envelope(value: string): Record<string, any> {
  return JSON.parse(value) as Record<string, any>;
}

async function openTeam(
  fixture: Fixture,
  room: string,
  teamId: string,
  env: Record<string, string>,
): Promise<void> {
  const opened = await runCliAt(
    fixture,
    room,
    [
      "team",
      "open",
      teamId,
      "--repo",
      fixture.repo,
      "--operation",
      `open-${teamId}-1`,
      "--wait-ms",
      "0",
      "--json",
    ],
    env,
  );
  expect(opened.exitCode).toBe(0);
}

test("trigger catalog fixes five semantic rules and their declared thresholds", () => {
  const semantic = coreTriggerRules.filter((rule) => rule.id.startsWith("semantic."));
  expect(semantic.map((rule) => [rule.id, rule.minimumOccurrences])).toEqual([
    ["semantic.failure-third", 3],
    ["semantic.status-contradiction", 1],
    ["semantic.role-boundary", 1],
    ["semantic.stop-silence", 1],
    ["semantic.self-correction", 2],
  ]);
});

test("semantic trigger thresholds, evidence bounds, and dedupe produce one Observer packet", async () => {
  await withFixture(async (fixture) => {
    const room = join(fixture.root, "room");
    await mkdir(room);
    const fake = await installFakeHerdr(fixture);
    await openTeam(fixture, room, "theta", fake.env);
    const beforeThreshold = await fakeHerdrCommands(fake);

    const underThreshold = await runCliAt(
      fixture,
      room,
      [
        "team",
        "review",
        "trigger",
        "theta",
        "--operation",
        "trigger-theta-under",
        "--rule",
        "semantic.failure-third",
        "--actor",
        "lead-repo",
        "--evidence",
        "same failure",
        "--occurrences",
        "2",
        "--json",
      ],
      fake.env,
    );
    expect(underThreshold.exitCode).toBe(1);
    expect(envelope(underThreshold.stderr).error.code).toBe("TRIGGER_THRESHOLD_NOT_MET");
    expect(await fakeHerdrCommands(fake)).toEqual(beforeThreshold);

    const longExcerpt = "x".repeat(9_000);
    const fired = await runCliAt(
      fixture,
      room,
      [
        "team",
        "review",
        "trigger",
        "theta",
        "--operation",
        "trigger-theta-fired",
        "--rule",
        "semantic.failure-third",
        "--actor",
        "lead-repo",
        "--evidence",
        "same failure",
        "--excerpt",
        longExcerpt,
        "--occurrences",
        "3",
        "--work",
        "w42",
        "--json",
      ],
      fake.env,
    );
    expect(fired.exitCode).toBe(0);
    const first = envelope(fired.stdout).data.packet;
    expect(first).toMatchObject({
      actor: "lead-repo",
      generation: 1,
      ruleId: "semantic.failure-third",
      status: "DELIVERED",
      teamId: "theta",
      truncated: true,
      workRef: "w42",
    });
    expect(first.excerpt.length).toBe(8_192);
    expect(typeof first.capability).toBe("string");

    const beforeDedupe = await fakeHerdrCommands(fake);
    const deduped = await runCliAt(
      fixture,
      room,
      [
        "team",
        "review",
        "trigger",
        "theta",
        "--operation",
        "trigger-theta-deduped",
        "--rule",
        "semantic.failure-third",
        "--actor",
        "lead-repo",
        "--evidence",
        "same failure",
        "--excerpt",
        longExcerpt,
        "--occurrences",
        "3",
        "--work",
        "w42",
        "--json",
      ],
      fake.env,
    );
    expect(deduped.exitCode).toBe(0);
    expect(envelope(deduped.stdout).data).toMatchObject({ deduped: true });
    expect(envelope(deduped.stdout).data.packet.id).toBe(first.id);
    expect(await fakeHerdrCommands(fake)).toEqual(beforeDedupe);
  });
});

test("only a live packet capability raises REVIEW_HOLD and Supervisor rationale clears it", async () => {
  await withFixture(async (fixture) => {
    const room = join(fixture.root, "room");
    await mkdir(room);
    const fake = await installFakeHerdr(fixture);
    await openTeam(fixture, room, "iota", fake.env);
    const triggered = await runCliAt(
      fixture,
      room,
      [
        "team",
        "review",
        "trigger",
        "iota",
        "--operation",
        "trigger-iota-1",
        "--rule",
        "semantic.status-contradiction",
        "--actor",
        "lead-repo",
        "--evidence",
        "claim says done while work is active",
        "--occurrences",
        "1",
        "--json",
      ],
      fake.env,
    );
    const packet = envelope(triggered.stdout).data.packet;

    const denied = await runCliAt(
      fixture,
      room,
      [
        "team",
        "review",
        "raise",
        "iota",
        "--operation",
        "raise-iota-denied",
        "--packet",
        packet.id,
        "--capability",
        "wrong",
        "--finding",
        "contradiction",
        "--json",
      ],
      fake.env,
    );
    expect(denied.exitCode).toBe(1);
    expect(envelope(denied.stderr).error.code).toBe("OBSERVER_CAPABILITY_REJECTED");

    const raised = await runCliAt(
      fixture,
      room,
      [
        "team",
        "review",
        "raise",
        "iota",
        "--operation",
        "raise-iota-1",
        "--packet",
        packet.id,
        "--capability",
        packet.capability,
        "--finding",
        "contradiction",
        "--json",
      ],
      fake.env,
    );
    expect(raised.exitCode).toBe(0);
    expect(envelope(raised.stdout).data.team).toMatchObject({
      health: "READY",
      review: "REVIEW_REQUIRED",
      verdict: "REVIEW_HOLD",
    });

    const wrongSupervisor = await runCliAt(
      fixture,
      room,
      [
        "team",
        "review",
        "clear",
        "iota",
        "--operation",
        "clear-iota-denied",
        "--requested-by",
        "lead-repo",
        "--rationale",
        "not authorized",
        "--json",
      ],
      fake.env,
    );
    expect(wrongSupervisor.exitCode).toBe(1);
    expect(envelope(wrongSupervisor.stderr).error.code).toBe("TEAM_AUTHORITY_REQUIRED");

    const cleared = await runCliAt(
      fixture,
      room,
      [
        "team",
        "review",
        "clear",
        "iota",
        "--operation",
        "clear-iota-1",
        "--requested-by",
        "supervisor-iota",
        "--rationale",
        "status evidence reconciled",
        "--json",
      ],
      fake.env,
    );
    expect(cleared.exitCode).toBe(0);
    expect(envelope(cleared.stdout).data.team).toMatchObject({
      health: "READY",
      review: "CLEAR",
      verdict: "OPERABLE",
    });
  });
});

test("health preserves review and escalation uses separate review and health receipts", async () => {
  await withFixture(async (fixture) => {
    const room = join(fixture.root, "room");
    await mkdir(room);
    const fake = await installFakeHerdr(fixture);
    await openTeam(fixture, room, "kappa", fake.env);
    const triggered = await runCliAt(
      fixture,
      room,
      [
        "team",
        "review",
        "trigger",
        "kappa",
        "--operation",
        "trigger-kappa-1",
        "--rule",
        "semantic.role-boundary",
        "--actor",
        "lead-repo",
        "--evidence",
        "Lead answered an owner-only question",
        "--json",
      ],
      fake.env,
    );
    const packet = envelope(triggered.stdout).data.packet;
    await runCliAt(
      fixture,
      room,
      [
        "team",
        "review",
        "raise",
        "kappa",
        "--operation",
        "raise-kappa-1",
        "--packet",
        packet.id,
        "--capability",
        packet.capability,
        "--finding",
        "authority boundary crossed",
        "--json",
      ],
      fake.env,
    );

    const health = await runCliAt(
      fixture,
      room,
      [
        "team",
        "health",
        "kappa",
        "--operation",
        "health-kappa-review",
        "--json",
      ],
      fake.env,
    );
    expect(health.exitCode).toBe(0);
    expect(envelope(health.stdout).data.team).toMatchObject({
      health: "READY",
      review: "REVIEW_REQUIRED",
      verdict: "REVIEW_HOLD",
    });

    const escalated = await runCliAt(
      fixture,
      room,
      [
        "team",
        "review",
        "escalate",
        "kappa",
        "--operation",
        "escalate-kappa-1",
        "--requested-by",
        "owner",
        "--owner-intervention",
        "--override-reason",
        "owner requires the authority violation to drain the team",
        "--override-evidence",
        "room decision d-kappa-drain",
        "--rationale",
        "authority violation requires runtime drain",
        "--json",
      ],
      fake.env,
    );
    expect(escalated.exitCode).toBe(0);
    const result = envelope(escalated.stdout).data;
    expect(result.team).toMatchObject({
      health: "DEGRADED",
      review: "CLEAR",
      verdict: "DRAINING",
    });
    expect(result.receipts.map((receipt: { kind: string }) => receipt.kind)).toEqual([
      "team.review.escalate.review",
      "team.review.escalate.health",
    ]);
    for (const receipt of result.receipts) {
      expect(receipt).toMatchObject({
        overrideEvidence: {
          basis: "owner-intervention",
          declared: "room decision d-kappa-drain",
          missing: [],
        },
        overrideReason: "owner requires the authority violation to drain the team",
        requestedBy: "owner",
      });
    }
  });
});

test("Supervisor spot-check is one-shot, generation-bound, and installs no repeated delivery", async () => {
  await withFixture(async (fixture) => {
    const room = join(fixture.root, "room");
    await mkdir(room);
    const fake = await installFakeHerdr(fixture);
    await openTeam(fixture, room, "lambda", fake.env);

    const denied = await runCliAt(
      fixture,
      room,
      [
        "team",
        "review",
        "spot-check",
        "lambda",
        "--operation",
        "spot-lambda-denied",
        "--requested-by",
        "lead-repo",
        "--question",
        "Does the current claim match w7?",
        "--window",
        "turn 14",
        "--stop",
        "one verdict",
        "--json",
      ],
      fake.env,
    );
    expect(denied.exitCode).toBe(1);
    expect(envelope(denied.stderr).error.code).toBe("TEAM_AUTHORITY_REQUIRED");

    const before = await fakeHerdrCommands(fake);
    const fired = await runCliAt(
      fixture,
      room,
      [
        "team",
        "review",
        "spot-check",
        "lambda",
        "--operation",
        "spot-lambda-1",
        "--requested-by",
        "supervisor-lambda",
        "--question",
        "Does the current claim match w7?",
        "--window",
        "turn 14",
        "--stop",
        "one verdict",
        "--json",
      ],
      fake.env,
    );
    expect(fired.exitCode).toBe(0);
    expect(envelope(fired.stdout).data.packet).toMatchObject({
      generation: 1,
      ruleId: "supervisor.spot-check",
      status: "DELIVERED",
    });
    const delivered = (await fakeHerdrCommands(fake)).slice(before.length)
      .filter((command) => command.slice(0, 2).join(" ") === "agent prompt");
    expect(delivered).toHaveLength(1);

    const beforeDedupe = await fakeHerdrCommands(fake);
    const deduped = await runCliAt(
      fixture,
      room,
      [
        "team",
        "review",
        "spot-check",
        "lambda",
        "--operation",
        "spot-lambda-2",
        "--requested-by",
        "supervisor-lambda",
        "--question",
        "Does the current claim match w7?",
        "--window",
        "turn 14",
        "--stop",
        "one verdict",
        "--json",
      ],
      fake.env,
    );
    expect(envelope(deduped.stdout).data.deduped).toBe(true);
    expect(await fakeHerdrCommands(fake)).toEqual(beforeDedupe);
  });
});
