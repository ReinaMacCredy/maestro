import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, rm, stat, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { TarArchiveAdapter } from "@/features/bundle/adapters/tar-archive.adapter.js";
import type { BundleManifest } from "@/features/bundle/domain/bundle-types.js";

let tmpDir: string;
let adapter: TarArchiveAdapter;

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-bundle-tar-"));
  adapter = new TarArchiveAdapter();
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

const sampleManifest: BundleManifest = {
  schemaVersion: 1,
  bundleId: "f5f3a1a0-0000-0000-0000-000000000000",
  createdAt: "2026-04-15T00:00:00.000Z",
  maestroVersion: "0.0.0",
  mission: {
    id: "2026-04-15-001",
    title: "Test",
    status: "draft",
    createdAt: "2026-04-14T00:00:00.000Z",
  },
  stats: {
    features: 1,
    milestones: 1,
    assertions: 0,
    agents: 0,
    replies: 0,
    handoffs: 0,
    checkpoints: 0,
    principlesSnapshot: 0,
    outcomesSnapshot: 0,
  },
  redacted: [],
  gitPatch: null,
};

describe("TarArchiveAdapter", () => {
  it("writes a tarball and round-trips the manifest", async () => {
    const outPath = join(tmpDir, "test.mission.tar.gz");
    const files = [
      {
        path: "2026-04-15-001.mission/manifest.json",
        content: JSON.stringify(sampleManifest, null, 2) + "\n",
      },
      {
        path: "2026-04-15-001.mission/mission/mission.json",
        content: '{"id":"2026-04-15-001"}\n',
      },
    ];

    const bytes = await adapter.writeTarGz(outPath, files);
    expect(bytes).toBeGreaterThan(0);

    const info = await stat(outPath);
    expect(info.size).toBe(bytes);

    const manifest = await adapter.readManifest(outPath);
    expect(manifest.bundleId).toBe(sampleManifest.bundleId);
    expect(manifest.schemaVersion).toBe(1);
    expect(manifest.mission.id).toBe("2026-04-15-001");
  });

  it("rejects archives without a manifest entry", async () => {
    const outPath = join(tmpDir, "no-manifest.tar.gz");
    await adapter.writeTarGz(outPath, [
      { path: "something/else.txt", content: "hi" },
    ]);

    await expect(adapter.readManifest(outPath)).rejects.toThrow(/manifest\.json/);
  });

  it("rejects archives with an unknown schemaVersion", async () => {
    const outPath = join(tmpDir, "future.tar.gz");
    const futureManifest = { ...sampleManifest, schemaVersion: 99 };
    await adapter.writeTarGz(outPath, [
      {
        path: "future.mission/manifest.json",
        content: JSON.stringify(futureManifest, null, 2) + "\n",
      },
    ]);

    await expect(adapter.readManifest(outPath)).rejects.toThrow(/schemaVersion/);
  });

  it("reports a helpful error when manifest is not valid JSON", async () => {
    const outPath = join(tmpDir, "broken.tar.gz");
    await adapter.writeTarGz(outPath, [
      { path: "broken.mission/manifest.json", content: "not json" },
    ]);

    await expect(adapter.readManifest(outPath)).rejects.toThrow(/not valid JSON/);
  });

  it("rejects structurally invalid schema-v1 manifests", async () => {
    const outPath = join(tmpDir, "invalid-shape.tar.gz");
    await adapter.writeTarGz(outPath, [
      {
        path: "invalid-shape.mission/manifest.json",
        content: JSON.stringify({ schemaVersion: 1 }) + "\n",
      },
    ]);

    await expect(adapter.readManifest(outPath)).rejects.toThrow(/manifest/i);
  });

  it("rejects bundle files that escape the staging root", async () => {
    const outPath = join(tmpDir, "unsafe-path.tar.gz");

    await expect(adapter.writeTarGz(outPath, [
      {
        path: "../escaped.txt",
        content: "nope",
      },
    ])).rejects.toThrow(/outside the allowed root/i);
  });

  it("rejects manifest entries that could be parsed as tar options", async () => {
    const archiveRoot = join(tmpDir, "unsafe-archive");
    const entryDir = join(archiveRoot, "--checkpoint=1");
    const outPath = join(tmpDir, "option-member.tar.gz");
    await mkdir(entryDir, { recursive: true });
    await writeFile(join(entryDir, "manifest.json"), JSON.stringify(sampleManifest), "utf8");

    const tar = Bun.spawnSync({
      cmd: ["tar", "-czf", outPath, "-C", archiveRoot, "--", "--checkpoint=1/manifest.json"],
      stdout: "pipe",
      stderr: "pipe",
    });
    expect(tar.exitCode).toBe(0);

    await expect(adapter.readManifest(outPath)).rejects.toThrow(/unsafe/i);
  });

  it("surfaces tar errors when given a missing file", async () => {
    const missing = join(tmpDir, "does-not-exist.tar.gz");
    await expect(adapter.readManifest(missing)).rejects.toThrow();
  });

});
