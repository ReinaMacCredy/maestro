import { describe, expect, test } from "bun:test";
import { ago, cards, counts, pillState, progress, workRows } from "./model";
import { ASTRA, FIXTURE, FIXTURE_NOW, MAESTRO } from "./fixture";

describe("workRows", () => {
  test("orders parents before children and marks gated parents", () => {
    const rows = workRows(MAESTRO);
    expect(rows.map((r) => r.work.id)).toEqual(["w15", "w21"]);
    expect(rows[0]).toMatchObject({ status: "gated", depth: 0, blockers: [] });
    expect(rows[1]).toMatchObject({ status: "active", depth: 1 });
  });

  test("an open item with an open blocker is gated on that blocker", () => {
    expect(workRows(ASTRA)[2]).toMatchObject({ status: "gated", blockers: ["w3"] });
  });
});

describe("cards", () => {
  test("one card per finding, draft decision and gated item, with a copyable command", () => {
    const list = cards(FIXTURE, FIXTURE_NOW);
    expect(list.map((c) => c.variant)).toEqual(["decision", "gated", "attention"]);
    expect(list.map((c) => c.command)).toEqual([
      "maestro decision lock d10",
      "maestro work list --parent w15",
      "maestro work show w3",
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

  test("ready is an open item with no open blocker and no gate", () => {
    expect(counts([ASTRA], []).ready).toBe(1);
    expect(counts([MAESTRO], []).ready).toBe(0);
  });

  test("progress counts done work the list no longer carries", () => {
    expect(progress(MAESTRO)).toEqual({ done: 5, total: 7, active: 1, pct: 71 });
  });
});

test("ago rounds to minutes, hours, days", () => {
  const now = new Date("2026-08-25T12:00:00Z");
  expect(ago("2026-08-25T11:54:00Z", now)).toBe("6m");
  expect(ago("2026-08-25T09:00:00Z", now)).toBe("3h");
  expect(ago("2026-08-22T12:00:00Z", now)).toBe("3d");
  expect(ago(null, now)).toBe("");
});
