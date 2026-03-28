import { describe, expect, it, beforeEach, afterEach } from "bun:test";
import { mkdtemp, rm, mkdir, writeFile, symlink, realpath } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir, homedir } from "node:os";
import { FsHandoffStoreAdapter } from "../../src/adapters/handoff-store.adapter.js";
import { ClaudeSessionDetectAdapter } from "../../src/adapters/session-detect.adapter.js";
import { createHandoff } from "../../src/usecases/create-handoff.usecase.js";
import { digHandoff } from "../../src/usecases/dig-handoff.usecase.js";
import { mockGit, mockCass } from "../helpers/mocks.js";
import type { HandoffSession } from "../../src/domain/types.js";

let tmpDir: string;
let store: FsHandoffStoreAdapter;

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-sourcepath-"));
  await mkdir(join(tmpDir, ".maestro", "handoffs"), { recursive: true });
  store = new FsHandoffStoreAdapter(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("sourcePath encoding", () => {
  const adapter = new ClaudeSessionDetectAdapter();

  it("encodes project path with leading dash matching Claude Code format", async () => {
    // Claude Code encodes /Users/foo/Code/bar as -Users-foo-Code-bar (leading dash)
    const cwd = process.cwd();
    const session = await adapter.detect(cwd);

    if (session) {
      // The encoded project dir should start with a dash
      const projectsDir = join(homedir(), ".claude", "projects");
      const relPath = session.sourcePath.replace(projectsDir + "/", "");
      const projectDir = relPath.split("/")[0]!;
      expect(projectDir.startsWith("-")).toBe(true);
    }
  });

  it("sourcePath ends with .jsonl extension", async () => {
    const cwd = process.cwd();
    const session = await adapter.detect(cwd);

    if (session) {
      expect(session.sourcePath.endsWith(".jsonl")).toBe(true);
    }
  });

  it("sourcePath resolves to a real file on disk", async () => {
    const cwd = process.cwd();
    const session = await adapter.detect(cwd);

    if (session) {
      const exists = await Bun.file(session.sourcePath).exists();
      expect(exists).toBe(true);
    }
  });

  it("sourcePath contains the session ID", async () => {
    const cwd = process.cwd();
    const session = await adapter.detect(cwd);

    if (session) {
      expect(session.sourcePath).toContain(session.sessionId);
    }
  });

  it("sourcePath encodes slashes in CWD as dashes", async () => {
    const cwd = process.cwd();
    const session = await adapter.detect(cwd);

    if (session) {
      const projectsDir = join(homedir(), ".claude", "projects");
      const relPath = session.sourcePath.replace(projectsDir + "/", "");
      const projectDir = relPath.split("/")[0]!;

      // Verify the project dir contains no slashes (they should be dashes)
      expect(projectDir.includes("/")).toBe(false);

      // Verify the CWD components are present as dash-separated segments
      const cwdParts = cwd.split("/").filter(Boolean);
      for (const part of cwdParts) {
        expect(projectDir).toContain(part);
      }
    }
  });
});

describe("sourcePath in handoff lifecycle", () => {
  it("created handoff has sourcePath pointing to real session file", async () => {
    const adapter = new ClaudeSessionDetectAdapter();
    const session = await adapter.detect(process.cwd());

    if (!session) return; // Skip in environments without a Claude session

    const mockSession = {
      detect: async () => session,
      resolve: async () => session,
    };

    const handoff = await createHandoff(
      mockGit(),
      mockSession,
      { sessionDetection: { enabled: true, agents: ["claude-code"] } },
      store,
      { plan: false, sitrep: "test", quickstart: "test", session: "test-session-abc", dir: tmpDir },
    );

    const exists = await Bun.file(handoff.session.sourcePath).exists();
    expect(exists).toBe(true);
  });

  it("stored handoff preserves sourcePath through create -> get roundtrip", async () => {
    const adapter = new ClaudeSessionDetectAdapter();
    const session = await adapter.detect(process.cwd());

    if (!session) return;

    const mockSession = {
      detect: async () => session,
      resolve: async () => session,
    };

    const handoff = await createHandoff(
      mockGit(),
      mockSession,
      { sessionDetection: { enabled: true, agents: ["claude-code"] } },
      store,
      { plan: false, sitrep: "test", quickstart: "test", session: "test-session-abc", dir: tmpDir },
    );

    const envelope = await store.get(handoff.id);
    expect(envelope?.handoff.session.sourcePath).toBe(handoff.session.sourcePath);
    expect(envelope?.handoff.session.sourcePath.endsWith(".jsonl")).toBe(true);
  });
});

describe("dig handoff sentinel behavior", () => {
  function makeSessionForDir(dir: string): HandoffSession {
    return {
      agent: "claude-code",
      sessionId: "test-session-abc",
      sourcePath: join(dir, "session.jsonl"),
    };
  }

  it("does not write sentinel when sourcePath file does not exist", async () => {
    const session = makeSessionForDir("/tmp/nonexistent-dir-12345");
    const handoff = await createHandoff(
      mockGit(),
      { detect: async () => session, resolve: async () => session },
      { sessionDetection: { enabled: true, agents: ["claude-code"] } },
      store,
      { plan: false, sitrep: "test", quickstart: "test", session: "test-session-abc", dir: tmpDir },
    );

    const cass = mockCass();
    await digHandoff(store, cass, "test query", {
      id: handoff.id,
      dir: tmpDir,
    });

    const sentinelPath = join(tmpDir, ".maestro", "handoffs", handoff.id, ".cass-indexed");
    const sentinelExists = await Bun.file(sentinelPath).exists();
    expect(sentinelExists).toBe(false);
  });

  it("writes sentinel when sourcePath file exists", async () => {
    // Create a fake session file
    const sessionFile = join(tmpDir, "session.jsonl");
    await writeFile(sessionFile, '{"test": true}\n');

    const session: HandoffSession = {
      agent: "claude-code",
      sessionId: "test-session-abc",
      sourcePath: sessionFile,
    };

    const handoff = await createHandoff(
      mockGit(),
      { detect: async () => session, resolve: async () => session },
      { sessionDetection: { enabled: true, agents: ["claude-code"] } },
      store,
      { plan: false, sitrep: "test", quickstart: "test", session: "test-session-abc", dir: tmpDir },
    );

    const cass = mockCass();
    await digHandoff(store, cass, "test query", {
      id: handoff.id,
      dir: tmpDir,
    });

    const sentinelPath = join(tmpDir, ".maestro", "handoffs", handoff.id, ".cass-indexed");
    const sentinelExists = await Bun.file(sentinelPath).exists();
    expect(sentinelExists).toBe(true);
  });

  it("skips re-indexing when sentinel already exists", async () => {
    const sessionFile = join(tmpDir, "session.jsonl");
    await writeFile(sessionFile, '{"test": true}\n');

    const session: HandoffSession = {
      agent: "claude-code",
      sessionId: "test-session-abc",
      sourcePath: sessionFile,
    };

    const handoff = await createHandoff(
      mockGit(),
      { detect: async () => session, resolve: async () => session },
      { sessionDetection: { enabled: true, agents: ["claude-code"] } },
      store,
      { plan: false, sitrep: "test", quickstart: "test", session: "test-session-abc", dir: tmpDir },
    );

    let indexCount = 0;
    const cass = mockCass({
      indexOnce: async () => { indexCount++; },
    });

    // First dig -- should index
    await digHandoff(store, cass, "query1", { id: handoff.id, dir: tmpDir });
    expect(indexCount).toBe(1);

    // Second dig -- sentinel exists, should skip
    await digHandoff(store, cass, "query2", { id: handoff.id, dir: tmpDir });
    expect(indexCount).toBe(1);
  });

  it("does not call cass.indexOnce when sourcePath is missing", async () => {
    const session = makeSessionForDir("/tmp/nonexistent-dir-99999");
    const handoff = await createHandoff(
      mockGit(),
      { detect: async () => session, resolve: async () => session },
      { sessionDetection: { enabled: true, agents: ["claude-code"] } },
      store,
      { plan: false, sitrep: "test", quickstart: "test", session: "test-session-abc", dir: tmpDir },
    );

    let indexCalled = false;
    const cass = mockCass({
      indexOnce: async () => { indexCalled = true; },
    });

    await digHandoff(store, cass, "test", { id: handoff.id, dir: tmpDir });
    expect(indexCalled).toBe(false);
  });

  it("still searches even when sourcePath is missing (uses existing index)", async () => {
    const session = makeSessionForDir("/tmp/nonexistent-dir-99999");
    const handoff = await createHandoff(
      mockGit(),
      { detect: async () => session, resolve: async () => session },
      { sessionDetection: { enabled: true, agents: ["claude-code"] } },
      store,
      { plan: false, sitrep: "test", quickstart: "test", session: "test-session-abc", dir: tmpDir },
    );

    let searchCalled = false;
    const cass = mockCass({
      search: async (q) => {
        searchCalled = true;
        return { query: q, count: 0, totalMatches: 0, hits: [] };
      },
    });

    await digHandoff(store, cass, "test", { id: handoff.id, dir: tmpDir });
    expect(searchCalled).toBe(true);
  });

  it("does not write sentinel when indexOnce throws (failed indexing)", async () => {
    const sessionFile = join(tmpDir, "session.jsonl");
    await writeFile(sessionFile, '{"test": true}\n');

    const session: HandoffSession = {
      agent: "claude-code",
      sessionId: "test-session-fail",
      sourcePath: sessionFile,
    };

    const handoff = await createHandoff(
      mockGit(),
      { detect: async () => session, resolve: async () => session },
      { sessionDetection: { enabled: true, agents: ["claude-code"] } },
      store,
      { plan: false, sitrep: "test", quickstart: "test", session: "test-session-abc", dir: tmpDir },
    );

    const cass = mockCass({
      indexOnce: async () => { throw new Error("CASS indexing failed (exit 1)"); },
    });

    await digHandoff(store, cass, "test", { id: handoff.id, dir: tmpDir });

    const sentinelPath = join(tmpDir, ".maestro", "handoffs", handoff.id, ".cass-indexed");
    const sentinelExists = await Bun.file(sentinelPath).exists();
    expect(sentinelExists).toBe(false);
  });

  it("allows re-indexing after failed attempt (no stale sentinel)", async () => {
    const sessionFile = join(tmpDir, "session.jsonl");
    await writeFile(sessionFile, '{"test": true}\n');

    const session: HandoffSession = {
      agent: "claude-code",
      sessionId: "test-session-retry",
      sourcePath: sessionFile,
    };

    const handoff = await createHandoff(
      mockGit(),
      { detect: async () => session, resolve: async () => session },
      { sessionDetection: { enabled: true, agents: ["claude-code"] } },
      store,
      { plan: false, sitrep: "test", quickstart: "test", session: "test-session-abc", dir: tmpDir },
    );

    let indexCount = 0;

    // First attempt: indexOnce fails
    const failCass = mockCass({
      indexOnce: async () => { indexCount++; throw new Error("fail"); },
    });
    await digHandoff(store, failCass, "test", { id: handoff.id, dir: tmpDir });
    expect(indexCount).toBe(1);

    // Second attempt: indexOnce succeeds -- should re-index since no sentinel
    const okCass = mockCass({
      indexOnce: async () => { indexCount++; },
    });
    await digHandoff(store, okCass, "test", { id: handoff.id, dir: tmpDir });
    expect(indexCount).toBe(2);

    // Third attempt: sentinel now exists, should skip
    await digHandoff(store, okCass, "test", { id: handoff.id, dir: tmpDir });
    expect(indexCount).toBe(2);
  });
});

describe("sourcePath symlink resolution", () => {
  it("resolves symlinked CWD to real path for sourcePath encoding", async () => {
    // /tmp on macOS is a symlink to /private/tmp
    const realTmp = await realpath(tmpdir());
    const adapter = new ClaudeSessionDetectAdapter();

    // Detect from the symlinked path
    const session = await adapter.detect(process.cwd());

    if (session) {
      // sourcePath should use the resolved (real) path, not the symlink
      const resolvedCwd = await realpath(process.cwd());
      const encodedResolved = resolvedCwd.replace(/\//g, "-");
      expect(session.sourcePath).toContain(encodedResolved);
    }
  });
});

describe("corrupted file resilience", () => {
  it("handoff list survives a corrupted envelope.json", async () => {
    // Create a valid handoff first
    const session: HandoffSession = {
      agent: "claude-code",
      sessionId: "test-session",
      sourcePath: "/tmp/test",
    };

    const handoff = await createHandoff(
      mockGit(),
      { detect: async () => session, resolve: async () => session },
      { sessionDetection: { enabled: true, agents: ["claude-code"] } },
      store,
      { plan: false, sitrep: "valid handoff", quickstart: "test", session: "test-session", dir: tmpDir },
    );

    // Create a corrupted handoff directory
    const corruptDir = join(tmpDir, ".maestro", "handoffs", "2099-01-01-001");
    await mkdir(corruptDir, { recursive: true });
    await writeFile(join(corruptDir, "envelope.json"), "NOT VALID JSON{{{");
    await writeFile(join(corruptDir, "handoff.json"), "NOT VALID JSON{{{");

    // list() should return the valid handoff and skip the corrupted one
    const all = await store.list();
    expect(all.length).toBe(1);
    expect(all[0]!.handoff.id).toBe(handoff.id);
  });

  it("handoff list survives a schema-invalid envelope", async () => {
    const session: HandoffSession = {
      agent: "claude-code",
      sessionId: "test-session",
      sourcePath: "/tmp/test",
    };

    const handoff = await createHandoff(
      mockGit(),
      { detect: async () => session, resolve: async () => session },
      { sessionDetection: { enabled: true, agents: ["claude-code"] } },
      store,
      { plan: false, sitrep: "valid handoff", quickstart: "test", session: "test-session", dir: tmpDir },
    );

    // Create a valid-JSON but schema-invalid envelope
    const badDir = join(tmpDir, ".maestro", "handoffs", "2099-01-01-002");
    await mkdir(badDir, { recursive: true });
    await writeFile(join(badDir, "envelope.json"), JSON.stringify({ wrong: "schema" }));

    const all = await store.list();
    expect(all.length).toBe(1);
    expect(all[0]!.handoff.id).toBe(handoff.id);
  });
});
