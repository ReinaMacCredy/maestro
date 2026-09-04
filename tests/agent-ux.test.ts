import { expect, test } from "bun:test";
import { Database } from "bun:sqlite";
import { mkdir, symlink } from "node:fs/promises";
import { join } from "node:path";
import { idFrom, runCli, withFixture, writeConfig, writePlugin, type Fixture } from "./helpers.ts";

interface JsonRpcResponse {
  id: number;
  result?: Record<string, unknown>;
}

const cli = join(import.meta.dir, "..", "bin", "maestro.ts");

function startMcp(fixture: Fixture) {
  const child = Bun.spawn([process.execPath, cli, "mcp", "serve"], {
    cwd: fixture.repo,
    env: {
      ...process.env,
      HOME: fixture.home,
      MAESTRO_SESSION_ID: "test-session",
      MAESTRO_SESSION_PID: String(process.pid),
    },
    stdin: "pipe",
    stdout: "pipe",
    stderr: "pipe",
  });
  const reader = child.stdout.getReader();
  const decoder = new TextDecoder();
  let buffered = "";

  async function request(id: number, method: string, params: Record<string, unknown> = {}) {
    child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", id, method, params })}\n`);
    while (!buffered.includes("\n")) {
      const chunk = await reader.read();
      if (chunk.done) throw new Error(`MCP server closed before replying to ${method}`);
      buffered += decoder.decode(chunk.value, { stream: true });
    }
    const newline = buffered.indexOf("\n");
    const line = buffered.slice(0, newline);
    buffered = buffered.slice(newline + 1);
    return JSON.parse(line) as JsonRpcResponse;
  }

  async function close(): Promise<void> {
    child.stdin.end();
    expect(await child.exited).toBe(0);
    expect(await new Response(child.stderr).text()).toBe("");
  }

  return { close, request };
}

async function initialize(mcp: ReturnType<typeof startMcp>): Promise<void> {
  await mcp.request(1, "initialize", {
    protocolVersion: "2025-06-18",
    capabilities: {},
    clientInfo: { name: "maestro-tests", version: "1" },
  });
}

async function noPsEnvironment(fixture: Fixture): Promise<Record<string, string>> {
  const path = join(fixture.root, "no-ps-bin");
  await mkdir(path, { recursive: true });
  const git = Bun.which("git");
  if (!git) throw new Error("git is required for the no-ps fixture");
  await symlink(process.execPath, join(path, "bun"));
  await symlink(git, join(path, "git"));
  return {
    PATH: path,
    MAESTRO_SESSION_ID: "",
    MAESTRO_SESSION_PID: "",
    CODEX_SESSION_ID: "",
    CODEX_THREAD_ID: "",
    CLAUDE_CODE_SESSION_ID: "",
    CLAUDE_SESSION_ID: "",
    CURSOR_SESSION_ID: "",
    CODEX_CI: "1",
  };
}

function sessionEnvironment(id: string, pid = ""): Record<string, string> {
  return {
    MAESTRO_SESSION_ID: id,
    MAESTRO_SESSION_PID: pid,
    CODEX_SESSION_ID: "",
    CODEX_THREAD_ID: "",
    CLAUDE_CODE_SESSION_ID: "",
    CLAUDE_SESSION_ID: "",
    CURSOR_SESSION_ID: "",
    CODEX_CI: "1",
  };
}

test("54 sandboxed processes without an identity get fresh sessions and never inherit a lease (d704)", async () => {
  await withFixture(async (fixture) => {
    const id = idFrom(await runCli(fixture, ["work", "add", "sandbox bridge", "--kind", "idea"]));
    const environment = await noPsEnvironment(fixture);

    const hooked = await runCli(
      fixture,
      ["hook", "record", "--event", "SessionStart", "--harness", "codex"],
      environment,
    );
    const started = await runCli(fixture, ["work", "start", id], environment);
    const holder = started.stdout.trim().split(" started by ")[1] ?? "missing session";
    const completed = await runCli(
      fixture,
      ["work", "done", id, "--claim", "test: sandbox bridge", "--proof", "three processes"],
      environment,
    );

    expect(hooked.exitCode).toBe(0);
    expect(started.exitCode).toBe(0);
    // Every identity-less process is its own session. The second cannot finish
    // work the first holds, and the refusal names the holder instead of
    // silently adopting it.
    expect(completed.exitCode).not.toBe(0);
    expect(completed.stderr).toContain("LEASE_HELD");
    expect(completed.stderr).toContain(holder);

    const status = await runCli(fixture, ["status", "--json"], environment);
    const sessions = (JSON.parse(status.stdout) as {
      data: { sessions: Array<{ anchor: string; id: string }> };
    }).data.sessions.filter((session) => session.anchor === "ttl");
    expect(sessions.length).toBeGreaterThan(1);
    expect(new Set(sessions.map((session) => session.id)).size).toBe(sessions.length);

    // Carrying the identity forward is what restores continuity.
    const recovered = await runCli(
      fixture,
      ["work", "done", id, "--claim", "test: sandbox bridge", "--proof", "explicit identity"],
      { ...environment, MAESTRO_SESSION_ID: holder },
    );
    expect(recovered.exitCode).toBe(0);
  });
});

test("55 TTL liveness follows last_seen while PID liveness follows the PID", async () => {
  await withFixture(async (fixture) => {
    const noPs = await noPsEnvironment(fixture);
    const staleId = idFrom(await runCli(fixture, ["work", "add", "stale ttl", "--kind", "idea"]));
    const freshId = idFrom(await runCli(fixture, ["work", "add", "fresh ttl", "--kind", "idea"]));
    const pidId = idFrom(await runCli(fixture, ["work", "add", "dead pid", "--kind", "idea"]));

    for (const [sessionId, workId] of [
      ["stale-ttl", staleId],
      ["fresh-ttl", freshId],
    ] as const) {
      const environment = { ...noPs, ...sessionEnvironment(sessionId) };
      expect(
        (
          await runCli(
            fixture,
            ["hook", "record", "--event", "SessionStart", "--harness", "codex"],
            environment,
          )
        ).exitCode,
      ).toBe(0);
      expect((await runCli(fixture, ["work", "start", workId], environment)).exitCode).toBe(0);
    }

    const sharedDeadPid = "99999999";
    const exclusiveDeadPid = "99999998";
    const pidEnvironment = sessionEnvironment("dead-pid", exclusiveDeadPid);
    expect(
      (
        await runCli(
          fixture,
          ["hook", "record", "--event", "SessionStart", "--harness", "codex"],
          pidEnvironment,
        )
      ).exitCode,
    ).toBe(0);
    expect((await runCli(fixture, ["work", "start", pidId], pidEnvironment)).exitCode).toBe(0);

    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"));
    database.query("UPDATE sessions SET pid = ?, last_seen = ? WHERE id = ?")
      .run(Number(sharedDeadPid), new Date(Date.now() - 61 * 60 * 1000).toISOString(), "stale-ttl");
    database.query("UPDATE sessions SET pid = ? WHERE id = ?")
      .run(Number(sharedDeadPid), "fresh-ttl");
    database.close();

    const observer = { ...noPs, ...sessionEnvironment("observer") };
    const ready = await runCli(fixture, ["ready", "--json"], observer);
    expect(ready.exitCode).toBe(0);
    const readyData = JSON.parse(ready.stdout).data as {
      gated: unknown[];
      works: Array<{ id: string }>;
    };
    const status = await runCli(fixture, ["status", "--all", "--json"], observer);
    const sessions = (JSON.parse(status.stdout) as {
      data: { sessions: Array<{ id: string; live: boolean }> };
    }).data.sessions;

    expect(readyData.works.map((work) => work.id)).toContain(staleId);
    expect(readyData.works.map((work) => work.id)).not.toContain(freshId);
    expect(readyData.works.map((work) => work.id)).toContain(pidId);
    expect(readyData.gated).toEqual([]);
    expect(sessions.find((session) => session.id === "stale-ttl")?.live).toBeFalse();
    expect(sessions.find((session) => session.id === "fresh-ttl")?.live).toBeTrue();
    expect(sessions.find((session) => session.id === "dead-pid")?.live).toBeFalse();
  });
});

test("56 work done takes an unheld lease and names the holder that lost it (w548/d720)", async () => {
  await withFixture(async (fixture) => {
    const missingId = idFrom(await runCli(fixture, ["work", "add", "missing lease", "--kind", "idea"]));
    const missing = await runCli(fixture, ["work", "done", missingId, "--evidence", "none"]);

    const expiredId = idFrom(await runCli(fixture, ["work", "add", "expired lease", "--kind", "idea"]));
    const expiredHolder = sessionEnvironment("expired-holder", "99999999");
    expect((await runCli(fixture, ["work", "start", expiredId], expiredHolder)).exitCode).toBe(0);
    const expired = await runCli(
      fixture,
      ["work", "done", expiredId, "--evidence", "retry"],
      sessionEnvironment("replacement-holder", String(process.pid)),
    );

    expect(missing.exitCode).toBe(0);
    expect(missing.stdout).toContain(`${missingId} done`);
    // The expiry reason the old error carried survives on the success path.
    expect(expired.exitCode).toBe(0);
    expect(expired.stdout).toContain(`${expiredId} done`);
    expect(expired.stdout).toContain("expired-holder");
    expect(expired.stdout.toLowerCase()).toContain("pid");
  });
});

test("57 maestro_find tokenizes, ranks, caps, and hints after zero hits", async () => {
  await withFixture(async (fixture) => {
    await writePlugin(
      fixture,
      "repo",
      "find-load",
      `
export default {
  name: "find-load",
  inject: ["recipe"],
  apply(ctx) {
    for (let index = 0; index < 12; index += 1) {
      const suffix = String(index).padStart(2, "0");
      ctx.effect(() => ctx.cli.register(
        \`task \${suffix}\`,
        () => "ok",
        { description: "Track a task through a loaded verb." },
      ));
    }
    for (let index = 0; index < 7; index += 1) {
      const suffix = String(index).padStart(2, "0");
      ctx.effect(() => ctx.recipe.register({
        name: \`task-recipe-\${suffix}\`,
        description: "Track a task through a loaded recipe.",
        body: "# Loaded",
      }));
    }
  },
};
`,
    );
    await writeConfig(fixture, [{ name: "find-load" }]);
    const mcp = startMcp(fixture);
    await initialize(mcp);

    const matched = await mcp.request(2, "tools/call", {
      name: "maestro_find",
      arguments: { query: "track a task" },
    });
    const missed = await mcp.request(3, "tools/call", {
      name: "maestro_find",
      arguments: { query: "xylophonicquasar" },
    });
    await mcp.close();
    const result = matched.result?.structuredContent as {
      recipes: Array<{ name: string }>;
      verbs: Array<{ name: string }>;
    };
    const empty = missed.result?.structuredContent as {
      hint?: string;
      recipes: unknown[];
      verbs: unknown[];
    };

    expect(result.verbs).toHaveLength(10);
    expect(result.recipes).toHaveLength(5);
    expect(result.verbs[0]?.name).toBe("task 00");
    expect(result.recipes[0]?.name).toBe("task-recipe-00");
    expect(empty.verbs).toEqual([]);
    expect(empty.recipes).toEqual([]);
    expect(empty.hint).toContain("maestro help");
    expect(empty.hint).toContain("single-word");
  });
});

test("58 nested help equals flag help and renders required positionals", async () => {
  await withFixture(async (fixture) => {
    const nested = await runCli(fixture, ["help", "work", "add"]);
    const flagged = await runCli(fixture, ["work", "add", "--help"]);
    const done = await runCli(fixture, ["work", "done", "--help"]);

    expect(nested.exitCode).toBe(0);
    expect(nested.stdout).toBe(flagged.stdout);
    expect(nested.stdout).toContain("work add <title>");
    expect(done.stdout).toContain("work done <id>");
  });
});

test("59 native search bounds entities, collapses logs, and keeps JSON summaries dense", async () => {
  await withFixture(async (fixture) => {
    for (let index = 0; index < 25; index += 1) {
      expect(
        (await runCli(fixture, ["work", "add", `nativebound ${index}`, "--kind", "idea"])).exitCode,
      ).toBe(0);
    }

    const result = await runCli(fixture, ["search", "nativebound"]);
    const lines = result.stdout.trim().split("\n");
    const hits = lines.filter((line) => /^w\d+ \(idea, open\): nativebound/.test(line));
    const limited = await runCli(fixture, ["search", "nativebound", "--limit", "7"]);
    const limitedLines = limited.stdout.trim().split("\n");
    const limitedHits = limitedLines.filter((line) =>
      /^w\d+ \(idea, open\): nativebound/.test(line)
    );
    const dense = await runCli(fixture, ["search", "nativebound", "--limit", "7", "--json"]);
    const envelope = JSON.parse(dense.stdout) as {
      data: { matches: Array<Record<string, unknown>> };
    };
    const invalid = await runCli(fixture, ["search", "nativebound", "--limit", "0"]);

    expect(result.exitCode).toBe(0);
    expect(hits).toHaveLength(5);
    expect(lines.at(-1)).toBe("20 more; raise --limit to see them");
    expect(result.stdout).not.toContain("work.add");
    expect(result.stdout).not.toContain('{"title"');
    for (const hit of hits) expect(hit.split(" — ").at(-1)?.length).toBeLessThanOrEqual(200);
    expect(limited.exitCode).toBe(0);
    expect(limitedHits).toHaveLength(7);
    expect(limitedLines.at(-1)).toBe("18 more; raise --limit to see them");
    expect(dense.exitCode).toBe(0);
    expect(envelope.data.matches).toHaveLength(7);
    for (const match of envelope.data.matches) {
      expect(match).not.toHaveProperty("payload");
      expect(match).not.toHaveProperty("text");
      expect(match).not.toHaveProperty("snippet");
    }
    expect(invalid.exitCode).toBe(1);
    expect(invalid.stderr).toContain("INVALID_OPTION");
  });
});

test("60 every shipped recipe command example carries the maestro prefix", async () => {
  await withFixture(async (fixture) => {
    const help = await runCli(fixture, ["help"]);
    expect(help.exitCode).toBe(0);
    const roots = new Set(
      [...help.stdout.matchAll(/^  (\S+)\s{2,}\S/gm)].map((match) => match[1] ?? ""),
    );
    expect(roots.size).toBeGreaterThan(0);
    const listed = await runCli(fixture, ["recipe", "list"]);
    const recipeNames = listed.stdout.trim().split("\n").map((line) => line.split("\t", 1)[0] ?? "");

    for (const name of recipeNames) {
      const shown = await runCli(fixture, ["recipe", "show", name]);
      const inline = [...shown.stdout.matchAll(/`([^`\n]+)`/g)].map((match) => match[1] ?? "");
      const commands = inline.filter((example) => {
        const trimmed = example.trim();
        return roots.has(trimmed.split(/\s+/, 1)[0] ?? "") && !trimmed.startsWith("maestro ");
      });
      expect(commands, name).toEqual([]);
    }
  });
});

test("636 --json is honored by every verb, decision draft and lock included", async () => {
  await withFixture(async (fixture) => {
    const draft = await runCli(fixture, ["decision", "draft", "use bun", "--json"]);
    expect(draft.exitCode).toBe(0);
    const drafted = JSON.parse(draft.stdout) as {
      data: { decision: { id: string; state: string } };
      ok: boolean;
    };
    expect(drafted.ok).toBe(true);
    expect(drafted.data.decision.state).toBe("draft");

    const lock = await runCli(fixture, ["decision", "lock", drafted.data.decision.id, "--json"]);
    expect(lock.exitCode).toBe(0);
    const locked = JSON.parse(lock.stdout) as { data: { decision: { state: string } } };
    expect(locked.data.decision.state).toBe("locked");

    const help = await runCli(fixture, ["help", "decision", "lock"]);
    expect(help.stdout).toContain("--json");

    const work = idFrom(await runCli(fixture, ["work", "add", "cancel me", "--kind", "idea"]));
    const cancel = await runCli(fixture, ["work", "cancel", work, "--reason", "no", "--json"]);
    expect(cancel.exitCode).toBe(0);
    expect((JSON.parse(cancel.stdout) as { ok: boolean }).ok).toBe(true);
  });
});

test("637 work block and unblock edit blockers after creation with work add's checks", async () => {
  await withFixture(async (fixture) => {
    const first = idFrom(await runCli(fixture, ["work", "add", "first", "--kind", "idea"]));
    const second = idFrom(await runCli(fixture, ["work", "add", "second", "--kind", "idea"]));

    const missing = await runCli(fixture, ["work", "block", second, "--by", "w99"]);
    expect(missing.exitCode).not.toBe(0);
    expect((JSON.parse(missing.stderr) as { error: { code: string } }).error.code).toBe("NOT_FOUND");
    const self = await runCli(fixture, ["work", "block", second, "--by", second]);
    expect(self.exitCode).not.toBe(0);
    expect((JSON.parse(self.stderr) as { error: { code: string } }).error.code).toBe(
      "INVALID_ARGUMENT",
    );

    const blocked = await runCli(fixture, ["work", "block", second, "--by", first, "--json"]);
    expect(blocked.exitCode).toBe(0);
    expect(
      (JSON.parse(blocked.stdout) as { data: { blockers: Array<{ id: string }> } }).data.blockers
        .map((blocker) => blocker.id),
    ).toEqual([first]);
    const again = await runCli(fixture, ["work", "block", second, "--by", first]);
    expect(again.exitCode).not.toBe(0);
    expect((JSON.parse(again.stderr) as { error: { code: string } }).error.code).toBe(
      "INVALID_STATE",
    );
    const cycle = await runCli(fixture, ["work", "block", first, "--by", second]);
    expect(cycle.exitCode).not.toBe(0);
    expect((JSON.parse(cycle.stderr) as { error: { code: string } }).error.code).toBe(
      "INVALID_ARGUMENT",
    );

    const start = await runCli(fixture, ["work", "start", second]);
    expect(start.exitCode).not.toBe(0);
    const startError = (JSON.parse(start.stderr) as { error: { blockers: string[]; code: string } })
      .error;
    expect(startError.code).toBe("BLOCKED");
    expect(startError.blockers).toEqual([first]);
    expect((await runCli(fixture, ["work", "show", second])).stdout).toContain(
      `blocker: ${first} [open] first`,
    );
    const trace = await runCli(fixture, ["trace", second]);
    expect(trace.stdout).toContain("work.block");

    const notBlocked = await runCli(fixture, ["work", "unblock", first, "--by", second]);
    expect(notBlocked.exitCode).not.toBe(0);
    expect((JSON.parse(notBlocked.stderr) as { error: { code: string } }).error.code).toBe(
      "NOT_FOUND",
    );
    const unblocked = await runCli(fixture, ["work", "unblock", second, "--by", first]);
    expect(unblocked.exitCode).toBe(0);
    expect(unblocked.stdout).toContain(`${second} unblocked; no blockers left`);
    expect((await runCli(fixture, ["work", "start", second])).exitCode).toBe(0);

    expect((await runCli(fixture, ["work", "done", second, "--evidence", "ok"])).exitCode).toBe(0);
    const closed = await runCli(fixture, ["work", "block", second, "--by", first]);
    expect(closed.exitCode).not.toBe(0);
    expect((JSON.parse(closed.stderr) as { error: { code: string } }).error.code).toBe(
      "INVALID_STATE",
    );
  });
});

test("638 ready hides blocked work by default and --all lists it with its gate", async () => {
  await withFixture(async (fixture) => {
    const parent = idFrom(
      await runCli(fixture, ["work", "add", "parent feature", "--kind", "feature"]),
    );
    const tests = idFrom(
      await runCli(fixture, ["work", "add", "red tests", "--kind", "task", "--parent", parent]),
    );
    const implementation = idFrom(
      await runCli(fixture, [
        "work", "add", "implementation", "--kind", "implement", "--parent", parent,
        "--blocked-by", tests,
      ]),
    );

    const ready = await runCli(fixture, ["ready"]);
    expect(ready.exitCode).toBe(0);
    expect(ready.stdout).toContain(`${tests} red tests`);
    expect(ready.stdout).toContain(`${parent} parent feature [gated by policy-breakdown`);
    expect(ready.stdout).not.toContain(`${implementation} implementation`);
    expect(ready.stdout).toContain("1 blocked by open work hidden; --all to list");

    const all = await runCli(fixture, ["ready", "--all"]);
    expect(all.exitCode).toBe(0);
    expect(all.stdout).toContain(
      `${implementation} implementation [gated by work-blockers: ${implementation} is blocked by unresolved work: ${tests} [open]`,
    );
    expect(all.stdout).not.toContain("hidden");

    const json = JSON.parse((await runCli(fixture, ["ready", "--json"])).stdout) as {
      data: { gated: Array<{ id: string; origin: string }> };
    };
    expect(json.data.gated.find((work) => work.id === implementation)?.origin).toBe(
      "work-blockers",
    );

    expect((await runCli(fixture, ["work", "start", tests])).exitCode).toBe(0);
    expect((await runCli(fixture, ["work", "done", tests, "--evidence", "red"])).exitCode).toBe(0);
    const after = await runCli(fixture, ["ready"]);
    expect(after.stdout).toContain(`${implementation} implementation`);
    expect(after.stdout).not.toContain("hidden");
  });
});
