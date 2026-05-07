import { beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FsContractVersionStoreAdapter } from "@/features/task/adapters/fs-contract-version-store.adapter.js";
import type { Contract } from "@/features/task/domain/contract/contract-types.js";
import { CONTRACT_SCHEMA_VERSION } from "@/features/task/domain/contract/contract-types.js";
import { MaestroError } from "@/shared/errors.js";

function makeContract(overrides: Partial<Contract> = {}): Contract {
  return {
    schemaVersion: CONTRACT_SCHEMA_VERSION,
    id: "c-a1b2c3",
    taskId: "tsk-a1b2c3",
    repoRoot: "/repo",
    status: "draft",
    createdAt: "2026-04-21T00:00:00.000Z",
    intent: "Implement versioned contracts",
    scope: {
      filesExpected: ["src/features/task/**"],
      filesForbidden: [],
    },
    doneWhen: [
      { id: "dw-a1b2c3", text: "versioned store exists", kind: "manual" as const },
    ],
    amendments: [],
    createdBy: "session:codex:1",
    configSnapshot: {
      strict: false,
      overlapPolicy: "fail" as const,
      rebaseFallback: "best-effort" as const,
      staleReclaimContractPolicy: "inherit" as const,
    },
    ...overrides,
  };
}

describe("FsContractVersionStoreAdapter", () => {
  let tmpDir: string;
  let store: FsContractVersionStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "contract-version-store-"));
    store = new FsContractVersionStoreAdapter(tmpDir);
  });

  it("round-trips write + readVersion + readCurrent + history", async () => {
    const contract = makeContract();
    await store.write("tsk-a1b2c3", 1, contract);

    const byVersion = await store.readVersion("tsk-a1b2c3", 1);
    expect(byVersion?.id).toBe("c-a1b2c3");

    const current = await store.readCurrent("tsk-a1b2c3");
    expect(current?.id).toBe("c-a1b2c3");

    const hist = await store.history("tsk-a1b2c3");
    expect(hist).toHaveLength(1);
    expect(hist[0]?.id).toBe("c-a1b2c3");
  });

  it("history returns versions in ascending order", async () => {
    const v1 = makeContract({ status: "draft" });
    const v2 = makeContract({ status: "locked" });
    const v3 = makeContract({ status: "amended" });

    await store.write("tsk-a1b2c3", 1, v1);
    await store.write("tsk-a1b2c3", 2, v2);
    await store.write("tsk-a1b2c3", 3, v3);

    const hist = await store.history("tsk-a1b2c3");
    expect(hist).toHaveLength(3);
    expect(hist[0]?.status).toBe("draft");
    expect(hist[1]?.status).toBe("locked");
    expect(hist[2]?.status).toBe("amended");
  });

  it("readCurrent returns the highest version", async () => {
    await store.write("tsk-a1b2c3", 1, makeContract({ status: "draft" }));
    await store.write("tsk-a1b2c3", 2, makeContract({ status: "locked" }));
    await store.write("tsk-a1b2c3", 3, makeContract({ status: "amended" }));

    const current = await store.readCurrent("tsk-a1b2c3");
    expect(current?.status).toBe("amended");
  });

  it("v1, v2, v3 are all readable independently", async () => {
    const v1 = makeContract({ status: "draft" });
    const v2 = makeContract({ status: "locked" });
    const v3 = makeContract({ status: "amended" });

    await store.write("tsk-a1b2c3", 1, v1);
    await store.write("tsk-a1b2c3", 2, v2);
    await store.write("tsk-a1b2c3", 3, v3);

    expect((await store.readVersion("tsk-a1b2c3", 1))?.status).toBe("draft");
    expect((await store.readVersion("tsk-a1b2c3", 2))?.status).toBe("locked");
    expect((await store.readVersion("tsk-a1b2c3", 3))?.status).toBe("amended");
  });

  it("readCurrent returns undefined when no contract exists", async () => {
    expect(await store.readCurrent("tsk-a1b2c3")).toBeUndefined();
  });

  it("readVersion returns undefined when that version does not exist", async () => {
    await store.write("tsk-a1b2c3", 1, makeContract());
    expect(await store.readVersion("tsk-a1b2c3", 99)).toBeUndefined();
  });

  it("files are written under .maestro/contracts/<task-id>/v<N>.json", async () => {
    const contract = makeContract();
    await store.write("tsk-a1b2c3", 1, contract);

    const path = join(tmpDir, ".maestro", "contracts", "tsk-a1b2c3", "v1.json");
    const exists = await Bun.file(path).exists();
    expect(exists).toBe(true);
  });

  it("rejects path-traversal task ids on write", async () => {
    await expect(
      store.write("../../etc/passwd", 1, makeContract()),
    ).rejects.toThrow(MaestroError);
  });

  it("rejects path-traversal task ids on readVersion", async () => {
    await expect(
      store.readVersion("../../etc/passwd", 1),
    ).rejects.toThrow(MaestroError);
  });

  it("rejects path-traversal task ids on history", async () => {
    await expect(
      store.history("../../etc/passwd"),
    ).rejects.toThrow(MaestroError);
  });

  it("rejects path-traversal task ids on readCurrent", async () => {
    await expect(
      store.readCurrent("../../etc/passwd"),
    ).rejects.toThrow(MaestroError);
  });
});
