import { beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, mkdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FsEvidenceStoreAdapter } from "@/features/evidence";
import { scanDocGardening } from "@/features/gc";

interface Fixtures {
  tmpDir: string;
  evidenceStore: FsEvidenceStoreAdapter;
}

let f: Fixtures;

beforeEach(async () => {
  const tmpDir = await mkdtemp(join(tmpdir(), "doc-gc-"));
  await mkdir(join(tmpDir, "docs"), { recursive: true });
  await mkdir(join(tmpDir, "src", "features", "real"), { recursive: true });
  await writeFile(join(tmpDir, "src", "features", "real", "index.ts"), "export {};");
  f = { tmpDir, evidenceStore: new FsEvidenceStoreAdapter(tmpDir) };
});

describe("scanDocGardening", () => {
  it("flags references to missing files", async () => {
    await writeFile(
      join(f.tmpDir, "docs", "broken.md"),
      "See `src/features/missing/index.ts` for details.\n",
    );
    const result = await scanDocGardening({}, { projectRoot: f.tmpDir });
    expect(result.staleReferences.length).toBe(1);
    expect(result.staleReferences[0]!.reference).toBe("src/features/missing/index.ts");
    expect(result.staleReferences[0]!.kind).toBe("missing-file");
  });

  it("ignores references to existing files", async () => {
    await writeFile(
      join(f.tmpDir, "docs", "ok.md"),
      "See `src/features/real/index.ts` — that exists.\n",
    );
    const result = await scanDocGardening({}, { projectRoot: f.tmpDir });
    expect(result.staleReferences.length).toBe(0);
  });

  it("skips http(s):// links", async () => {
    await writeFile(
      join(f.tmpDir, "docs", "links.md"),
      "Visit [docs](https://example.com/missing).\n",
    );
    const result = await scanDocGardening({}, { projectRoot: f.tmpDir });
    expect(result.staleReferences.length).toBe(0);
  });

  it("flags broken markdown links", async () => {
    await writeFile(
      join(f.tmpDir, "docs", "ml.md"),
      "Read [the guide](docs/missing-guide.md).\n",
    );
    const result = await scanDocGardening({}, { projectRoot: f.tmpDir });
    const refs = result.staleReferences.map((r) => r.reference);
    expect(refs).toContain("docs/missing-guide.md");
  });

  it("records doc-gardening evidence when taskId is provided", async () => {
    await writeFile(
      join(f.tmpDir, "docs", "foo.md"),
      "Refer to `src/features/missing/x.ts`.\n",
    );
    const result = await scanDocGardening(
      { evidenceStore: f.evidenceStore },
      { projectRoot: f.tmpDir, taskId: "tsk-abcdef", recordEvidence: true },
    );
    expect(result.evidenceId).toMatch(/^evd-/);
    const rows = await f.evidenceStore.list({ task_id: "tsk-abcdef", kind: "doc-gardening" });
    expect(rows.length).toBe(1);
    expect(rows[0]!.witness_level).toBe("witnessed-by-maestro");
  });

  it("does not record evidence when taskId is omitted", async () => {
    await writeFile(
      join(f.tmpDir, "docs", "foo.md"),
      "See `src/missing/x.ts`.\n",
    );
    const result = await scanDocGardening({ evidenceStore: f.evidenceStore }, { projectRoot: f.tmpDir });
    expect(result.evidenceId).toBeUndefined();
  });

  it("counts scanned files", async () => {
    await writeFile(join(f.tmpDir, "AGENTS.md"), "no refs here\n");
    await writeFile(join(f.tmpDir, "README.md"), "also clean\n");
    const result = await scanDocGardening({}, { projectRoot: f.tmpDir });
    expect(result.scannedFiles).toBeGreaterThanOrEqual(2);
  });
});
