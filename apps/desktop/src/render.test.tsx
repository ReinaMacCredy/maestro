import { describe, expect, test } from "bun:test";
import { renderToString } from "react-dom/server";
import { Panel } from "./components/Panel";
import { Pill } from "./components/Pill";
import { FIXTURE, FIXTURE_NOW } from "./fixture";
import { cards, counts, pillState, sessionIndex } from "./model";

const cardList = cards(FIXTURE, FIXTURE_NOW);
const c = counts(FIXTURE, cardList);

describe("Panel", () => {
  const html = renderToString(
    <Panel repos={FIXTURE} cards={cardList} counts={c} sessions={sessionIndex(FIXTURE)} now={FIXTURE_NOW} collapsed={new Set(["synara"])} onToggleRepo={() => {}} />,
  );

  test("renders every card with its command and a copy button, no approve actions", () => {
    expect(html).toContain('data-variant="decision"');
    expect(html).toContain('data-variant="gated"');
    expect(html).toContain('data-variant="attention"');
    expect(html).toContain("maestro decision lock d10");
    expect(html).toContain("maestro trace w3");
    expect((html.match(/class="copy"/g) ?? []).length).toBe(3);
    expect(html).not.toMatch(/Approve|Reject|Chấp nhận|Từ chối/);
  });

  test("renders one task-list per repo with tree rows, holder badge and gated marker", () => {
    expect((html.match(/class="todo"/g) ?? []).length).toBe(3);
    expect(html).toContain('class="todoItem gated "');
    expect(html).toContain('class="todoItem active child"');
    expect(html).toContain("codex 01a03893");
    expect(html).toContain("chờ w21");
    expect(html).toContain("Không có work đang theo dõi.");
    expect(html).toContain('aria-expanded="false" aria-label="Toggle synara"');
  });

  test("shows all-clear when nothing needs the human", () => {
    const quiet = renderToString(
      <Panel repos={[]} cards={[]} counts={{ active: 0, ready: 0, attention: 0 }} sessions={new Map()} now={FIXTURE_NOW} collapsed={new Set()} onToggleRepo={() => {}} />,
    );
    expect(quiet).toContain("Không có gì chờ bạn.");
    expect(quiet).toContain("Chưa có repo nào trong config.");
  });
});

describe("Pill", () => {
  test("shows counts and hides the bang when attention is zero", () => {
    const hot = renderToString(<Pill counts={c} state={pillState(c)} pinned={false} expanded={false} onClick={() => {}} />);
    expect(hot).toContain('data-state="attention"');
    expect(hot).toContain("3</span> !");
    const calm = renderToString(<Pill counts={{ active: 1, ready: 2, attention: 0 }} state="working" pinned={true} expanded={true} onClick={() => {}} />);
    expect(calm).not.toContain("!");
    expect(calm).toContain('class="pin"');
  });
});

test("no auto-approve survives from the aicss source", async () => {
  const glob = new Bun.Glob("src/**/*.{ts,tsx,css}");
  for await (const file of glob.scan(".")) {
    if (file.includes(".test.")) continue;
    expect(await Bun.file(file).text()).not.toMatch(/AUTO_APPROVE|autoApprove/);
  }
});
