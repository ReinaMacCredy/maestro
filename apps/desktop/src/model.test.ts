import { describe, expect, test } from "bun:test";
import { ago, cards, counts, pillState, workRows } from "./model";
import { ASTRA, FIXTURE, FIXTURE_NOW, MAESTRO } from "./fixture";

describe("workRows", () => {
  test("orders parents before children and marks gated parents", () => {
    const rows = workRows(MAESTRO);
    expect(rows.map((r) => r.work.id)).toEqual(["w15", "w16", "w17", "w18", "w19", "w20", "w21"]);
    expect(rows[0]).toMatchObject({ status: "gated", depth: 0, blockers: ["w21"] });
    expect(rows[6]).toMatchObject({ status: "active", depth: 1 });
  });
});

describe("cards", () => {
  test("one card per finding, draft decision and gated item, with a copyable command", () => {
    const list = cards(FIXTURE, FIXTURE_NOW);
    expect(list.map((c) => c.variant)).toEqual(["decision", "gated", "attention"]);
    expect(list.map((c) => c.command)).toEqual([
      "maestro decision lock d10",
      "maestro status",
      "maestro trace w3",
    ]);
  });

  test("locked decisions never become cards", () => {
    expect(cards([MAESTRO], FIXTURE_NOW).some((c) => c.title.startsWith("d3"))).toBe(false);
  });

  test("a draft older than 24h is hot", () => {
    const later = new Date(FIXTURE_NOW.getTime() + 25 * 3600000);
    expect(cards([MAESTRO], later)[0]?.hot).toBe(true);
    expect(cards([MAESTRO], FIXTURE_NOW)[0]?.hot).toBe(false);
  });
});

describe("pill", () => {
  test("counts aggregate across repos and attention wins the state", () => {
    const c = counts(FIXTURE, cards(FIXTURE, FIXTURE_NOW));
    expect(c).toEqual({ active: 2, ready: 1, attention: 3 });
    expect(pillState(c)).toBe("attention");
    expect(pillState({ active: 1, ready: 0, attention: 0 })).toBe("working");
    expect(pillState({ active: 0, ready: 0, attention: 0 })).toBe("idle");
  });

  test("ready comes from the ready verb, not from open state", () => {
    expect(counts([ASTRA], []).ready).toBe(1);
    expect(counts([MAESTRO], []).ready).toBe(0);
  });
});

test("ago rounds to minutes, hours, days", () => {
  const now = new Date("2026-08-25T12:00:00Z");
  expect(ago("2026-08-25T11:54:00Z", now)).toBe("6m");
  expect(ago("2026-08-25T09:00:00Z", now)).toBe("3h");
  expect(ago("2026-08-22T12:00:00Z", now)).toBe("3d");
  expect(ago(null, now)).toBe("");
});
