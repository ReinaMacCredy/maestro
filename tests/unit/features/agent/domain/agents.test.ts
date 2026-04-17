import { describe, expect, it } from "bun:test";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { SUPPORTED_AGENTS, agentConfigPath } from "@/features/agent";

describe("agent config specs", () => {
  it("anchors Droid config under project-local .maestro/AGENTS.md", () => {
    const droid = SUPPORTED_AGENTS.find((agent) => agent.slug === "droid");
    expect(droid).toBeDefined();
    expect(droid?.configDir).toBe(".maestro");
    expect(droid?.configFile).toBe("AGENTS.md");
    expect(droid?.configScope).toBe("project");
    const projectDir = join(tmpdir(), "project");
    expect(agentConfigPath(droid!, projectDir)).toBe(join(projectDir, ".maestro", "AGENTS.md"));
  });
});
