import { describe, expect, it } from "bun:test";
import { SUPPORTED_AGENTS, agentConfigPath } from "../../../src/domain/agents.js";

describe("agent config specs", () => {
  it("anchors Droid config under ~/.maestro/AGENTS.md", () => {
    const droid = SUPPORTED_AGENTS.find((agent) => agent.slug === "droid");
    expect(droid).toBeDefined();
    expect(droid?.configDir).toBe(".maestro");
    expect(droid?.configFile).toBe("AGENTS.md");
    expect(agentConfigPath(droid!)).toContain("/.maestro/AGENTS.md");
  });
});
