import { expect, setDefaultTimeout, test } from "bun:test";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { prepareInstallFixture, runCli, runCliAt, withFixture, type Fixture } from "./helpers.ts";

setDefaultTimeout(15_000);

async function hub(fixture: Fixture): Promise<string> {
  const room = join(fixture.home, "maestro");
  await mkdir(room, { recursive: true });
  return room;
}

async function writeClaudeFact(
  fixture: Fixture,
  file: string,
  fields: { name: string; description: string; modified: string; supersedes?: string; type?: string },
  body: string,
): Promise<string> {
  const directory = join(fixture.home, ".claude", "projects", "-Users-x", "memory");
  await mkdir(directory, { recursive: true });
  const path = join(directory, file);
  await writeFile(
    path,
    [
      "---",
      `name: ${fields.name}`,
      `description: ${fields.description}`,
      fields.supersedes ? `supersedes: ${fields.supersedes}` : null,
      "metadata:",
      `  type: ${fields.type ?? "feedback"}`,
      `  modified: ${fields.modified}`,
      "---",
      "",
      body,
      "",
    ].filter((line): line is string => line !== null).join("\n"),
  );
  return path;
}

test("569 R1 memory ingest: a buffer fact that supersedes a Hub fact replaces it and the superseded fact can never be promoted again", async () => {
  await withFixture(async (fixture) => {
    const room = await hub(fixture);
    await writeClaudeFact(
      fixture,
      "feedback_prose-over-question-cards.md",
      { name: "prose-over-question-cards", description: "Ask forks in prose, not cards", modified: "2026-06-01T10:00:00.000Z" },
      "Reina asked for prose questions.",
    );
    const first = await runCliAt(fixture, room, ["memory", "ingest", "--json"]);
    expect(first.exitCode).toBe(0);
    const firstData = JSON.parse(first.stdout).data;
    expect(firstData.counts).toEqual({ promoted: 1, updated: 0, skipped: 0, refused: 0 });
    expect(firstData.actions[0]).toMatchObject({ action: "promoted", id: "m1", slug: "prose-over-question-cards" });

    // 2026-06-08: the retraction lands in the buffer and names what it supersedes.
    await writeClaudeFact(
      fixture,
      "feedback_ask-with-cards.md",
      {
        name: "ask-with-cards",
        description: "Ask forks with AskUserQuestion cards, one decision per call",
        modified: "2026-06-08T10:00:00.000Z",
        supersedes: "prose-over-question-cards",
      },
      "Reina retracted the prose rule.",
    );
    const second = await runCliAt(fixture, room, ["memory", "ingest", "--json"]);
    expect(second.exitCode).toBe(0);
    const secondData = JSON.parse(second.stdout).data;
    // The old fact is still active when its turn comes (buffers replay in recorded order),
    // so this pass records it as a duplicate; the retraction lands after it and retires it.
    expect(secondData.counts).toEqual({ promoted: 1, updated: 0, skipped: 1, refused: 0 });
    const retraction = secondData.actions.find((action: { slug: string }) => action.slug === "ask-with-cards");
    expect(retraction).toMatchObject({ action: "promoted", id: "m2", reason: "supersedes m1" });

    const shown = await runCliAt(fixture, room, ["memory", "show", "m1", "--json"]);
    expect(JSON.parse(shown.stdout).data.fact).toMatchObject({ state: "superseded", supersededById: "m2" });
    const active = await runCliAt(fixture, room, ["memory", "list", "--json"]);
    expect(JSON.parse(active.stdout).data.facts.map((fact: { slug: string }) => fact.slug)).toEqual(["ask-with-cards"]);

    // The kill case: the old file still sits in the buffer; a third pass must not resurrect it.
    const third = await runCliAt(fixture, room, ["memory", "ingest", "--json"]);
    const thirdData = JSON.parse(third.stdout).data;
    expect(thirdData.counts).toEqual({ promoted: 0, updated: 0, skipped: 1, refused: 1 });
    const old = thirdData.actions.find((action: { slug: string }) => action.slug === "prose-over-question-cards");
    expect(old).toMatchObject({ action: "refused", id: "m1" });
    expect(old.reason).toContain("superseded by m2");
  });
});

test("570 memory ingest gates: dry-run writes nothing, a fact with no evidence is refused, a retracted fact is refused with its reason, and a project store refuses the verb", async () => {
  await withFixture(async (fixture) => {
    const room = await hub(fixture);
    await writeClaudeFact(
      fixture,
      "project_thing.md",
      { name: "thing", description: "A thing worth keeping", modified: "2026-07-01T00:00:00.000Z", type: "project" },
      "Body.",
    );
    const notes = join(fixture.home, ".codex", "memories", "extensions", "ad_hoc", "notes");
    await mkdir(notes, { recursive: true });
    await writeFile(join(notes, "20260810T232139+0700-astra-brand-icon.md"), "# Astra brand decision\n\n- Rebrand to Astra.\n");
    await writeFile(join(notes, "20260811T000000+0700-empty.md"), "");

    const dry = await runCliAt(fixture, room, ["memory", "ingest", "--dry-run", "--json"]);
    expect(dry.exitCode).toBe(0);
    const dryData = JSON.parse(dry.stdout).data;
    expect(dryData.dryRun).toBe(true);
    expect(dryData.counts).toEqual({ promoted: 2, updated: 0, skipped: 0, refused: 1 });
    expect(dryData.actions.find((action: { slug: string }) => action.slug === "empty").reason).toContain("no evidence");
    const stillEmpty = await runCliAt(fixture, room, ["memory", "list", "--json"]);
    expect(JSON.parse(stillEmpty.stdout).data.facts).toEqual([]);

    expect((await runCliAt(fixture, room, ["memory", "ingest"])).exitCode).toBe(0);
    const codex = await runCliAt(fixture, room, ["memory", "show", "astra-brand-icon", "--json"]);
    expect(JSON.parse(codex.stdout).data.fact).toMatchObject({ source: "codex-adhoc", kind: "project", description: "Astra brand decision" });

    const retract = await runCliAt(fixture, room, ["memory", "retract", "astra-brand-icon", "--reason", "icon decided"]);
    expect(retract.exitCode).toBe(0);
    const again = await runCliAt(fixture, room, ["memory", "ingest", "--json"]);
    const refused = JSON.parse(again.stdout).data.actions.find((action: { slug: string }) => action.slug === "astra-brand-icon");
    expect(refused.action).toBe("refused");
    expect(refused.reason).toContain("retracted");
    expect(refused.reason).toContain("icon decided");

    const project = await runCli(fixture, ["memory", "ingest", "--dry-run"]);
    expect(project.exitCode).not.toBe(0);
    expect(project.stderr).toContain("NOT_HUB_STORE");
    expect(project.stderr).toContain(room);
  });
});

test("571 R3 memory render: deterministic for a fixed store, differs from the store only by formatting, and a hand edit is detected on the next render", async () => {
  await withFixture(async (fixture) => {
    const room = await hub(fixture);
    await writeClaudeFact(
      fixture,
      "feedback_cards.md",
      { name: "ask-with-cards", description: "Ask forks with cards", modified: "2026-06-08T10:00:00.000Z" },
      "Body A.",
    );
    await writeClaudeFact(
      fixture,
      "project_hub.md",
      { name: "hub-store", description: "The Hub store is the memory source of truth", modified: "2026-09-04T00:00:00.000Z", type: "project" },
      "Body B.",
    );
    expect((await runCliAt(fixture, room, ["memory", "ingest"])).exitCode).toBe(0);
    const out = join(room, "MEMORY.md");

    const first = await runCliAt(fixture, room, ["memory", "render", "--json"]);
    expect(first.exitCode).toBe(0);
    const firstText = await readFile(out, "utf8");
    const second = await runCliAt(fixture, room, ["memory", "render", "--json"]);
    expect(second.exitCode).toBe(0);
    expect(await readFile(out, "utf8")).toBe(firstText);
    expect(JSON.parse(second.stdout).data.hash).toBe(JSON.parse(first.stdout).data.hash);

    // Every active fact appears once, superseded facts never, and nothing else carries content.
    expect(firstText).toContain("- ask-with-cards: Ask forks with cards (claude-auto 2026-06-08)");
    expect(firstText).toContain("- hub-store: The Hub store is the memory source of truth (claude-auto 2026-09-04)");
    expect(firstText.indexOf("## Feedback")).toBeLessThan(firstText.indexOf("## Project"));
    expect(firstText.startsWith("<!-- rendered by maestro memory render; content-hash: ")).toBe(true);
    const bullets = firstText.split("\n").filter((line) => line.startsWith("- "));
    expect(bullets).toHaveLength(2);
    expect((await runCliAt(fixture, room, ["memory", "render", "--check"])).exitCode).toBe(0);

    await appendFile(out, "- sneaky hand-written rule\n");
    const drifted = await runCliAt(fixture, room, ["memory", "render"]);
    expect(drifted.exitCode).not.toBe(0);
    expect(drifted.stderr).toContain("MEMORY_INDEX_DRIFT");
    const check = await runCliAt(fixture, room, ["memory", "render", "--check"]);
    expect(check.stderr).toContain("MEMORY_INDEX_DRIFT");
    expect((await readFile(out, "utf8")).endsWith("- sneaky hand-written rule\n")).toBe(true);

    const forced = await runCliAt(fixture, room, ["memory", "render", "--force"]);
    expect(forced.exitCode).toBe(0);
    expect(await readFile(out, "utf8")).toBe(firstText);

    await runCliAt(fixture, room, ["memory", "retract", "hub-store", "--reason", "test"]);
    const stale = await runCliAt(fixture, room, ["memory", "render", "--check"]);
    expect(stale.exitCode).not.toBe(0);
    expect(stale.stderr).toContain("MEMORY_INDEX_STALE");
  });
});

test("572 R4 cross-store search: one maestro search in a project returns a project term and a Hub decision, labelled by store", async () => {
  await withFixture(async (fixture) => {
    const room = await hub(fixture);
    const drafted = await runCliAt(fixture, room, [
      "decision", "draft", "Seat leases expire after one silent hour", "--rationale", "hub-side rule",
    ]);
    expect(drafted.exitCode).toBe(0);
    const term = await runCli(fixture, ["term", "add", "Seat", "One SLP role occupying one Herdr pane; a Seat holds at most one lease", "--json"]);
    expect(term.exitCode).toBe(0);
    expect(JSON.parse(term.stdout).data.term).toMatchObject({ id: "t1", name: "Seat" });

    const search = await runCli(fixture, ["search", "Seat", "--json"]);
    expect(search.exitCode).toBe(0);
    const matches = JSON.parse(search.stdout).data.matches as Array<Record<string, string>>;
    expect(matches).toContainEqual(expect.objectContaining({ id: "t1", kind: "term", store: "project" }));
    expect(matches).toContainEqual(expect.objectContaining({ id: "d1", kind: "decision", store: "hub" }));

    const text = await runCli(fixture, ["search", "Seat"]);
    expect(text.stdout).toContain("t1 (term, Seat)");
    expect(text.stdout).toContain("[hub] d1 (decision, draft)");

    const local = await runCli(fixture, ["search", "Seat", "--json", "--local"]);
    expect(JSON.parse(local.stdout).data.matches.map((match: { id: string }) => match.id)).toEqual(["t1"]);

    // From the Hub itself the room is not searched twice.
    const fromHub = await runCliAt(fixture, room, ["search", "Seat", "--json"]);
    const hubIds = JSON.parse(fromHub.stdout).data.matches.map((match: { id: string; store: string }) => `${match.store}:${match.id}`);
    expect(hubIds).toEqual(["project:d1"]);
  });
});

test("573 term add redefines in place, term show resolves name or id, and terms survive a search index rebuild", async () => {
  await withFixture(async (fixture) => {
    expect((await runCli(fixture, ["term", "add", "Lane", "A dispatch shape"])).exitCode).toBe(0);
    const redefined = await runCli(fixture, ["term", "add", "Lane", "A dispatch shape: scout, decision, delivery, challenge, shadow", "--json"]);
    expect(JSON.parse(redefined.stdout).data).toMatchObject({ updated: true, term: { id: "t1" } });
    const byName = await runCli(fixture, ["term", "show", "Lane"]);
    expect(byName.stdout).toContain("t1 Lane: A dispatch shape: scout");
    const list = await runCli(fixture, ["term", "list", "--json"]);
    expect(JSON.parse(list.stdout).data.terms).toHaveLength(1);
    const spaced = await runCli(fixture, ["term", "add", "two words", "x"]);
    expect(spaced.exitCode).not.toBe(0);
    expect(spaced.stderr).toContain("INVALID_ARGUMENT");

    // A fresh process re-indexes terms on apply, so a rebuilt search index still finds them.
    const { Database } = await import("bun:sqlite");
    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"));
    database.run("DELETE FROM search_index_state");
    database.close();
    const found = await runCli(fixture, ["search", "shadow", "--local", "--json"]);
    expect(JSON.parse(found.stdout).data.matches).toContainEqual(expect.objectContaining({ id: "t1", kind: "term" }));
  });
});

test("578 superseded and retracted memory facts leave the search index at once, in the room and through a project's Hub read", async () => {
  await withFixture(async (fixture) => {
    const room = await hub(fixture);
    await writeClaudeFact(
      fixture,
      "feedback_prose.md",
      { name: "prose-over-cards", description: "Ask forks in prose zebra", modified: "2026-06-01T10:00:00.000Z" },
      "Reina asked for prose questions.",
    );
    expect((await runCliAt(fixture, room, ["memory", "ingest"])).exitCode).toBe(0);
    const ids = (result: { stdout: string }) =>
      (JSON.parse(result.stdout).data.matches as Array<{ id: string }>).map((match) => match.id);
    expect(ids(await runCliAt(fixture, room, ["search", "zebra", "--json"]))).toEqual(["m1"]);
    expect(ids(await runCli(fixture, ["search", "zebra", "--json"]))).toEqual(["m1"]);

    await writeClaudeFact(
      fixture,
      "feedback_cards.md",
      { name: "ask-with-cards", description: "Ask forks with cards zebra", modified: "2026-06-08T10:00:00.000Z", supersedes: "prose-over-cards" },
      "Reina retracted the prose rule.",
    );
    expect((await runCliAt(fixture, room, ["memory", "ingest"])).exitCode).toBe(0);
    // The project's Hub read runs read-only, so it sees only what ingest de-indexed itself.
    expect(ids(await runCli(fixture, ["search", "zebra", "--json"]))).toEqual(["m2"]);
    const text = await runCliAt(fixture, room, ["search", "zebra"]);
    expect(text.stdout).toContain("m2");
    expect(text.stdout).not.toContain("m1");

    expect((await runCliAt(fixture, room, ["memory", "retract", "m2", "--reason", "settled"])).exitCode).toBe(0);
    expect(ids(await runCli(fixture, ["search", "zebra", "--json"]))).toEqual([]);
    expect(ids(await runCliAt(fixture, room, ["search", "zebra", "--json"]))).toEqual([]);
  });
});

test("579 search fails closed when the Hub room cannot be read, --local still answers, and a fresh install searches cleanly", async () => {
  await withFixture(async (fixture) => {
    const room = await hub(fixture);
    expect((await runCliAt(fixture, room, ["decision", "draft", "Seat leases expire", "--rationale", "hub"])).exitCode).toBe(0);
    expect((await runCli(fixture, ["term", "add", "Seat", "One role in one pane"])).exitCode).toBe(0);
    expect(JSON.parse((await runCli(fixture, ["search", "Seat", "--json"])).stdout).data.matches).toHaveLength(2);

    await writeFile(join(room, ".maestro", "maestro.db"), "not a sqlite database");
    const broken = await runCli(fixture, ["search", "Seat", "--json"]);
    expect(broken.exitCode).toBe(1);
    expect(broken.stderr).toContain("HUB_UNAVAILABLE");
    expect(broken.stderr).toContain("--local");
    const local = await runCli(fixture, ["search", "Seat", "--json", "--local"]);
    expect(local.exitCode).toBe(0);
    expect(JSON.parse(local.stdout).data.matches.map((match: { id: string }) => match.id)).toEqual(["t1"]);
  });
  await withFixture(async (fixture) => {
    const { path } = await prepareInstallFixture(fixture);
    expect((await runCli(fixture, ["install"], { PATH: path, SHELL: "/bin/zsh" })).exitCode).toBe(0);
    const searched = await runCli(fixture, ["search", "anything", "--json"]);
    expect(searched.exitCode).toBe(0);
    expect(JSON.parse(searched.stdout).data.matches).toEqual([]);
  });
});

test("580 --limit bounds the combined project-plus-Hub list, project hits first, with one remainder count", async () => {
  await withFixture(async (fixture) => {
    const room = await hub(fixture);
    for (const text of ["Seat lease one", "Seat lease two", "Seat lease three"]) {
      expect((await runCliAt(fixture, room, ["decision", "draft", text, "--rationale", "hub"])).exitCode).toBe(0);
    }
    for (const name of ["Seat", "SeatLease", "SeatPane"]) {
      expect((await runCli(fixture, ["term", "add", name, "A Seat thing"])).exitCode).toBe(0);
    }
    const limited = await runCli(fixture, ["search", "Seat", "--limit", "4", "--json"]);
    expect(limited.exitCode).toBe(0);
    const matches = JSON.parse(limited.stdout).data.matches as Array<{ id: string; store: string }>;
    expect(matches).toHaveLength(4);
    expect(matches.map((match) => match.store)).toEqual(["project", "project", "project", "hub"]);
    const text = await runCli(fixture, ["search", "Seat", "--limit", "4"]);
    expect(text.stdout).toContain("2 more; raise --limit to see them");
    expect(text.stdout.split("\n").filter((line) => line.startsWith("[hub]"))).toHaveLength(1);
    const all = await runCli(fixture, ["search", "Seat", "--limit", "10", "--json"]);
    expect(JSON.parse(all.stdout).data.matches).toHaveLength(6);
  });
});

test("581 a term name shaped like a generated term id is refused and the id row it would have shadowed is untouched", async () => {
  await withFixture(async (fixture) => {
    expect((await runCli(fixture, ["term", "add", "Lane", "A dispatch shape"])).exitCode).toBe(0);
    const hijack = await runCli(fixture, ["term", "add", "t1", "Not a lane"]);
    expect(hijack.exitCode).not.toBe(0);
    expect(hijack.stderr).toContain("INVALID_ARGUMENT");
    expect(hijack.stderr).toContain("t<number>");
    const byId = await runCli(fixture, ["term", "show", "t1", "--json"]);
    expect(JSON.parse(byId.stdout).data.term).toMatchObject({ id: "t1", name: "Lane", definition: "A dispatch shape" });
    expect(JSON.parse((await runCli(fixture, ["term", "list", "--json"])).stdout).data.terms).toHaveLength(1);
  });
});

test("582 a buffer fact whose slug takes the generated fact id shape is refused with a reason and memory show still resolves the id row", async () => {
  await withFixture(async (fixture) => {
    const room = await hub(fixture);
    await writeClaudeFact(
      fixture,
      "feedback_prose-over-question-cards.md",
      { name: "prose-over-question-cards", description: "Ask forks in prose, not cards", modified: "2026-06-01T10:00:00.000Z" },
      "Reina asked for prose questions.",
    );
    const first = await runCliAt(fixture, room, ["memory", "ingest", "--json"]);
    expect(JSON.parse(first.stdout).data.actions[0]).toMatchObject({ action: "promoted", id: "m1" });

    await writeClaudeFact(
      fixture,
      "feedback_m1.md",
      { name: "m1", description: "A slug that would shadow m1", modified: "2026-06-02T10:00:00.000Z" },
      "Body.",
    );
    const second = await runCliAt(fixture, room, ["memory", "ingest", "--json"]);
    expect(second.exitCode).toBe(0);
    const secondData = JSON.parse(second.stdout).data;
    expect(secondData.counts).toEqual({ promoted: 0, updated: 0, skipped: 1, refused: 1 });
    const refused = secondData.actions.find((action: { slug: string }) => action.slug === "m1");
    expect(refused).toMatchObject({ action: "refused", id: null });
    expect(refused.reason).toContain("m<number>");

    const shown = await runCliAt(fixture, room, ["memory", "show", "m1", "--json"]);
    expect(JSON.parse(shown.stdout).data.fact).toMatchObject({ id: "m1", slug: "prose-over-question-cards" });
  });
});
