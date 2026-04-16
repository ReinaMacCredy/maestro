import { describe, expect, it } from "bun:test";
import { BUILT_IN_SKILL_TEMPLATES } from "@/infra/domain/built-in-skill-templates.js";

describe("BUILT_IN_SKILL_TEMPLATES", () => {
  it("keeps maestro:conduct aligned with current task coordination commands", () => {
    const conduct = BUILT_IN_SKILL_TEMPLATES.find((template) => template.name === "maestro:conduct");
    const skill = conduct?.files.find((file) => file.path === "SKILL.md")?.content ?? "";

    expect(skill).toContain("maestro task create \"Write unit tests for validation\" --labels conduct --blocked-by <prevId>");
    expect(skill).toContain("maestro task claim <taskId> --session <worker-id>");
    expect(skill).toContain("maestro task update <taskId> --status in_progress");
    expect(skill).toContain("maestro task update <taskId> --status completed --reason \"implemented: <summary>\"");
    expect(skill).not.toContain("maestro task update <taskId> --claim");
    expect(skill).not.toContain("maestro task close <taskId>");
    expect(skill).not.toContain("--depends-on");
  });
});
