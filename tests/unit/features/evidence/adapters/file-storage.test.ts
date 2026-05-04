import { describe, expect, it, beforeEach } from "bun:test";
import { mkdtemp, mkdir } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FsEvidenceStoreAdapter } from "@/features/evidence/adapters/file-storage.js";
import { generateEvidenceId } from "@/features/evidence/domain/evidence-id.js";
import type { EvidenceRow } from "@/features/evidence/domain/types.js";

const FIXTURES_DIR = join(import.meta.dir, "../../../../fixtures/evidence");

function commandRow(overrides: Partial<EvidenceRow<"command">> = {}): EvidenceRow<"command"> {
  return {
    schema_version: 1,
    id: overrides.id ?? generateEvidenceId(),
    task_id: overrides.task_id ?? "tsk-aaaaaa",
    session_id: overrides.session_id,
    kind: "command",
    witness_level: overrides.witness_level ?? "witnessed-by-maestro",
    created_at: overrides.created_at ?? "2026-05-03T10:00:00.000Z",
    payload: overrides.payload ?? {
      command: "bun test",
      exit: 0,
      duration_ms: 1234,
    },
  };
}

function noteRow(overrides: Partial<EvidenceRow<"manual-note">> = {}): EvidenceRow<"manual-note"> {
  return {
    schema_version: 1,
    id: overrides.id ?? generateEvidenceId(),
    task_id: overrides.task_id ?? "tsk-aaaaaa",
    session_id: overrides.session_id,
    kind: "manual-note",
    witness_level: overrides.witness_level ?? "agent-claimed-locally",
    created_at: overrides.created_at ?? "2026-05-03T11:00:00.000Z",
    payload: overrides.payload ?? { note: "manual verification ok" },
  };
}

function verifierRow(overrides: Partial<EvidenceRow<"verifier">> = {}): EvidenceRow<"verifier"> {
  return {
    schema_version: 2,
    id: overrides.id ?? generateEvidenceId(),
    task_id: overrides.task_id ?? "tsk-aaaaaa",
    session_id: overrides.session_id,
    kind: "verifier",
    witness_level: overrides.witness_level ?? "witnessed-by-maestro",
    created_at: overrides.created_at ?? "2026-05-03T12:00:00.000Z",
    payload: overrides.payload ?? {
      check: "no sensitive paths exposed",
      severity: "warn",
      paths: ["src/secrets.ts"],
    },
  };
}

function contractAmendmentRow(overrides: Partial<EvidenceRow<"contract-amendment">> = {}): EvidenceRow<"contract-amendment"> {
  return {
    schema_version: 2,
    id: overrides.id ?? generateEvidenceId(),
    task_id: overrides.task_id ?? "tsk-aaaaaa",
    session_id: overrides.session_id,
    kind: "contract-amendment",
    witness_level: overrides.witness_level ?? "witnessed-by-maestro",
    created_at: overrides.created_at ?? "2026-05-03T13:00:00.000Z",
    payload: overrides.payload ?? {
      amendmentId: "amd-001",
      addedPaths: ["src/new-module/"],
      removedPaths: [],
      reason: "added new module to scope",
    },
  };
}

describe("FsEvidenceStoreAdapter", () => {
  let tmpDir: string;
  let store: FsEvidenceStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "evidence-"));
    store = new FsEvidenceStoreAdapter(tmpDir);
  });

  describe("append + read", () => {
    it("round-trips a command-kind row", async () => {
      const row = commandRow();
      await store.append(row);
      expect(await store.read(row.id)).toEqual(row);
    });

    it("round-trips a manual-note-kind row", async () => {
      const row = noteRow();
      await store.append(row);
      expect(await store.read(row.id)).toEqual(row);
    });

    it("returns undefined for an unknown id", async () => {
      const id = generateEvidenceId();
      expect(await store.read(id)).toBeUndefined();
    });
  });

  describe("list filters", () => {
    it("filters by task_id and sorts by created_at ascending", async () => {
      const a = commandRow({ task_id: "tsk-aaaaaa", created_at: "2026-05-03T09:00:00.000Z" });
      const b = commandRow({ task_id: "tsk-aaaaaa", created_at: "2026-05-03T08:00:00.000Z" });
      const other = commandRow({ task_id: "tsk-bbbbbb", created_at: "2026-05-03T07:00:00.000Z" });
      await store.append(a);
      await store.append(b);
      await store.append(other);

      const result = await store.list({ task_id: "tsk-aaaaaa" });
      expect(result.map((r) => r.id)).toEqual([b.id, a.id]);
    });

    it("filters by session_id across tasks", async () => {
      const matchA = commandRow({ task_id: "tsk-aaaaaa", session_id: "sess-1" });
      const matchB = noteRow({ task_id: "tsk-bbbbbb", session_id: "sess-1" });
      const skip = commandRow({ task_id: "tsk-cccccc", session_id: "sess-2" });
      await store.append(matchA);
      await store.append(matchB);
      await store.append(skip);

      const result = await store.list({ session_id: "sess-1" });
      const ids = result.map((r) => r.id).sort();
      expect(ids).toEqual([matchA.id, matchB.id].sort());
    });

    it("filters by kind across tasks", async () => {
      const cmd = commandRow({ task_id: "tsk-aaaaaa" });
      const note = noteRow({ task_id: "tsk-bbbbbb" });
      await store.append(cmd);
      await store.append(note);

      const cmds = await store.list({ kind: "command" });
      expect(cmds.map((r) => r.id)).toEqual([cmd.id]);

      const notes = await store.list({ kind: "manual-note" });
      expect(notes.map((r) => r.id)).toEqual([note.id]);
    });

    it("intersects task_id and kind filters", async () => {
      const target = commandRow({ task_id: "tsk-aaaaaa" });
      const wrongKind = noteRow({ task_id: "tsk-aaaaaa" });
      const wrongTask = commandRow({ task_id: "tsk-bbbbbb" });
      await store.append(target);
      await store.append(wrongKind);
      await store.append(wrongTask);

      const result = await store.list({ task_id: "tsk-aaaaaa", kind: "command" });
      expect(result.map((r) => r.id)).toEqual([target.id]);
    });

    it("returns an empty list when no evidence exists", async () => {
      expect(await store.list()).toEqual([]);
    });
  });

  describe("schema_version forward compatibility", () => {
    it("treats rows with a non-current schema_version as missing on read and excludes them from list", async () => {
      const futureId = generateEvidenceId();
      const taskDir = join(tmpDir, ".maestro", "evidence", "tsk-aaaaaa");
      await mkdir(taskDir, { recursive: true });
      await Bun.write(
        join(taskDir, `${futureId}.json`),
        JSON.stringify({
          schema_version: 999,
          id: futureId,
          task_id: "tsk-aaaaaa",
          kind: "command",
          witness_level: "witnessed-by-maestro",
          created_at: "2026-05-03T10:00:00.000Z",
          payload: { command: "bun test", exit: 0 },
        }),
      );
      const present = commandRow({ task_id: "tsk-aaaaaa" });
      await store.append(present);

      expect(await store.read(futureId)).toBeUndefined();
      const list = await store.list({ task_id: "tsk-aaaaaa" });
      expect(list.map((r) => r.id)).toEqual([present.id]);
    });
  });

  describe("schema_version backward compatibility (v1 rows)", () => {
    it("reads a v1 row stored directly on disk", async () => {
      const v1Id = generateEvidenceId();
      const taskDir = join(tmpDir, ".maestro", "evidence", "tsk-aaaaaa");
      await mkdir(taskDir, { recursive: true });
      const v1Row = {
        schema_version: 1,
        id: v1Id,
        task_id: "tsk-aaaaaa",
        kind: "command",
        witness_level: "witnessed-by-maestro",
        created_at: "2026-01-01T00:00:00.000Z",
        payload: { command: "bun test", exit: 0 },
      };
      await Bun.write(join(taskDir, `${v1Id}.json`), JSON.stringify(v1Row));

      const result = await store.read(v1Id);
      expect(result).toEqual(v1Row);
    });

    it("includes v1 rows in list results", async () => {
      const v1Id = generateEvidenceId();
      const taskDir = join(tmpDir, ".maestro", "evidence", "tsk-aaaaaa");
      await mkdir(taskDir, { recursive: true });
      await Bun.write(
        join(taskDir, `${v1Id}.json`),
        JSON.stringify({
          schema_version: 1,
          id: v1Id,
          task_id: "tsk-aaaaaa",
          kind: "manual-note",
          witness_level: "agent-claimed-locally",
          created_at: "2026-01-01T00:00:00.000Z",
          payload: { note: "legacy note" },
        }),
      );

      const list = await store.list({ task_id: "tsk-aaaaaa" });
      expect(list.map((r) => r.id)).toContain(v1Id);
    });
  });

  describe("schema v2 — new kinds", () => {
    it("round-trips a verifier-kind row", async () => {
      const row = verifierRow();
      await store.append(row);
      expect(await store.read(row.id)).toEqual(row);
    });

    it("round-trips a contract-amendment-kind row", async () => {
      const row = contractAmendmentRow();
      await store.append(row);
      expect(await store.read(row.id)).toEqual(row);
    });

    it("list returns mixed v1 and v2 rows ordered by created_at", async () => {
      const v1Row = commandRow({
        task_id: "tsk-aaaaaa",
        created_at: "2026-05-03T08:00:00.000Z",
      });
      const v2Verifier = verifierRow({
        task_id: "tsk-aaaaaa",
        created_at: "2026-05-03T09:00:00.000Z",
      });
      const v2Amendment = contractAmendmentRow({
        task_id: "tsk-aaaaaa",
        created_at: "2026-05-03T10:00:00.000Z",
      });
      await store.append(v1Row);
      await store.append(v2Verifier);
      await store.append(v2Amendment);

      const list = await store.list({ task_id: "tsk-aaaaaa" });
      expect(list.map((r) => r.id)).toEqual([v1Row.id, v2Verifier.id, v2Amendment.id]);
      expect(list[0]?.schema_version).toBe(1);
      expect(list[1]?.schema_version).toBe(2);
      expect(list[2]?.schema_version).toBe(2);
    });
  });

  describe("schema v3 — witness_level required + v1 synthesis", () => {
    it("v1 fixture row without witness_level is synthesized to agent-claimed-locally on read", async () => {
      const fixture = await Bun.file(join(FIXTURES_DIR, "v1-row.json")).json() as Record<string, unknown>;
      const taskDir = join(tmpDir, ".maestro", "evidence", fixture["task_id"] as string);
      await mkdir(taskDir, { recursive: true });
      await Bun.write(
        join(taskDir, `${fixture["id"] as string}.json`),
        JSON.stringify(fixture),
      );

      const result = await store.read(fixture["id"] as string);
      expect(result).toBeDefined();
      expect(result?.witness_level).toBe("agent-claimed-locally");
      expect(result?.schema_version).toBe(1);
    });

    it("v2 fixture row with witness_level reads back unchanged", async () => {
      const fixture = await Bun.file(join(FIXTURES_DIR, "v2-row.json")).json() as Record<string, unknown>;
      const taskDir = join(tmpDir, ".maestro", "evidence", fixture["task_id"] as string);
      await mkdir(taskDir, { recursive: true });
      await Bun.write(
        join(taskDir, `${fixture["id"] as string}.json`),
        JSON.stringify(fixture),
      );

      const result = await store.read(fixture["id"] as string);
      expect(result).toBeDefined();
      expect(result?.witness_level).toBe("witnessed-by-maestro");
      expect(result?.schema_version).toBe(2);
    });

    it("v2 row missing witness_level is rejected", async () => {
      const badId = generateEvidenceId();
      const taskDir = join(tmpDir, ".maestro", "evidence", "tsk-aaaaaa");
      await mkdir(taskDir, { recursive: true });
      await Bun.write(
        join(taskDir, `${badId}.json`),
        JSON.stringify({
          schema_version: 2,
          id: badId,
          task_id: "tsk-aaaaaa",
          kind: "command",
          created_at: "2026-05-04T08:00:00.000Z",
          payload: { command: "bun test", exit: 0 },
        }),
      );

      expect(await store.read(badId)).toBeUndefined();
    });

    it("v3 row written by adapter round-trips with schema_version 3", async () => {
      const row: EvidenceRow<"command"> = {
        schema_version: 3,
        id: generateEvidenceId(),
        task_id: "tsk-aaaaaa",
        kind: "command",
        witness_level: "witnessed-by-maestro",
        created_at: "2026-05-04T09:00:00.000Z",
        payload: { command: "bun run build", exit: 0 },
      };
      await store.append(row);
      const result = await store.read(row.id);
      expect(result?.schema_version).toBe(3);
      expect(result?.witness_level).toBe("witnessed-by-maestro");
    });

    it("list returns mixed v1/v2/v3 rows ordered by created_at", async () => {
      const v1Id = generateEvidenceId();
      const taskDir = join(tmpDir, ".maestro", "evidence", "tsk-aaaaaa");
      await mkdir(taskDir, { recursive: true });

      // Write a v1 row directly (no witness_level)
      await Bun.write(
        join(taskDir, `${v1Id}.json`),
        JSON.stringify({
          schema_version: 1,
          id: v1Id,
          task_id: "tsk-aaaaaa",
          kind: "manual-note",
          created_at: "2026-05-04T07:00:00.000Z",
          payload: { note: "legacy note" },
        }),
      );

      // Append a v2 row (written verbatim as v2)
      const v2Row: EvidenceRow<"command"> = {
        schema_version: 2,
        id: generateEvidenceId(),
        task_id: "tsk-aaaaaa",
        kind: "command",
        witness_level: "witnessed-by-maestro",
        created_at: "2026-05-04T08:00:00.000Z",
        payload: { command: "bun test", exit: 0 },
      };
      await store.append(v2Row);

      // Append a v3 row
      const v3Row: EvidenceRow<"command"> = {
        schema_version: 3,
        id: generateEvidenceId(),
        task_id: "tsk-aaaaaa",
        kind: "command",
        witness_level: "witnessed-by-ci",
        created_at: "2026-05-04T09:00:00.000Z",
        payload: { command: "bun run check:boundaries", exit: 0 },
      };
      await store.append(v3Row);

      const list = await store.list({ task_id: "tsk-aaaaaa" });
      expect(list.map((r) => r.id)).toEqual([v1Id, v2Row.id, v3Row.id]);
      expect(list[0]?.schema_version).toBe(1);
      expect(list[1]?.schema_version).toBe(2);
      expect(list[2]?.schema_version).toBe(3);
    });
  });

  describe("path safety", () => {
    it("rejects an invalid task_id on append", async () => {
      const row = commandRow({ task_id: "../etc/passwd" as string });
      await expect(store.append(row)).rejects.toThrow(/Invalid task ID/);
    });

    it("rejects a malformed evidence id on append", async () => {
      const row = commandRow({ id: "not-a-real-id" });
      await expect(store.append(row)).rejects.toThrow(/Invalid evidence ID/);
    });

    it("rejects a malformed evidence id on read", async () => {
      await expect(store.read("../../etc/passwd")).rejects.toThrow(/Invalid evidence ID/);
    });
  });

  describe("tolerance for stray files", () => {
    it("silently skips non-json files, malformed json, and nested dirs", async () => {
      const real = commandRow({ task_id: "tsk-aaaaaa" });
      await store.append(real);
      const taskDir = join(tmpDir, ".maestro", "evidence", "tsk-aaaaaa");
      await Bun.write(join(taskDir, "stray.txt"), "garbage");
      await Bun.write(join(taskDir, `${generateEvidenceId()}.json`), "{bad json\n");
      await mkdir(join(taskDir, "nested"), { recursive: true });

      const list = await store.list({ task_id: "tsk-aaaaaa" });
      expect(list.map((r) => r.id)).toEqual([real.id]);
    });

    it("ignores task subdirectories whose name is not a valid task id", async () => {
      const real = commandRow({ task_id: "tsk-aaaaaa" });
      await store.append(real);
      const evidenceDir = join(tmpDir, ".maestro", "evidence");
      await mkdir(join(evidenceDir, "not-a-task"), { recursive: true });

      const list = await store.list();
      expect(list.map((r) => r.id)).toEqual([real.id]);
    });
  });

  describe("concurrent writes", () => {
    it("handles 10 parallel appends to the same task without id collision", async () => {
      const taskId = "tsk-cccccc";
      const rows = Array.from({ length: 10 }, () =>
        commandRow({ task_id: taskId, id: generateEvidenceId() }),
      );
      await Promise.all(rows.map((row) => store.append(row)));

      const ids = rows.map((r) => r.id);
      expect(new Set(ids).size).toBe(10);

      const list = await store.list({ task_id: taskId });
      expect(list.length).toBe(10);
      for (const row of rows) {
        expect(await store.read(row.id)).toEqual(row);
      }
    });
  });
});
