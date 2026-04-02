import { describe, expect, it } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { YamlConfigAdapter } from "../../../src/adapters/config.adapter.js";
import { applyConfigEdit, previewConfigEdit } from "../../../src/usecases/config-edit.usecase.js";

describe("config-edit usecase", () => {
  it("previews and applies scalar edits to project config", async () => {
    const dir = await mkdtemp(join(tmpdir(), "config-edit-"));
    const adapter = new YamlConfigAdapter();

    const preview = await previewConfigEdit(
      adapter,
      dir,
      "project",
      "execution.stopOnFailure",
      "off",
    );

    expect(preview.content).toContain("stopOnFailure: false");

    await applyConfigEdit(
      adapter,
      dir,
      "project",
      "execution.stopOnFailure",
      "off",
    );

    const layers = await adapter.loadLayers(dir);
    expect(layers.project?.execution?.stopOnFailure).toBe(false);
  });
});
