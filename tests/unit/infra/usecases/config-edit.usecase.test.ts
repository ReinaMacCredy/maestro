import { describe, expect, it } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { YamlConfigAdapter } from "@/infra/adapters/config.adapter.js";
import { applyConfigEdit, previewConfigEdit } from "@/infra/usecases/config-edit.usecase.js";

describe("config-edit usecase", () => {
  it("previews and applies scalar edits to project config", async () => {
    const dir = await mkdtemp(join(tmpdir(), "config-edit-"));
    const adapter = new YamlConfigAdapter();

    const preview = await previewConfigEdit(
      adapter,
      dir,
      "project",
      "workers.codex.enabled",
      "off",
    );

    expect(preview.content).toContain("enabled: false");

    await applyConfigEdit(
      adapter,
      dir,
      "project",
      "workers.codex.enabled",
      "off",
    );

    const layers = await adapter.loadLayers(dir);
    const project = layers.project as unknown as { workers?: { codex?: { enabled?: boolean } } };
    expect(project?.workers?.codex?.enabled).toBe(false);
  });
});
