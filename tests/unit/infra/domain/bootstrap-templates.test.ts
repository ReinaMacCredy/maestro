import { describe, expect, it } from "bun:test";
import { PROJECT_BOOTSTRAP_TEMPLATES } from "@/infra/domain/bootstrap-templates.js";

// AGENT_INSTRUCTION_BLOCK was deleted when the 5-skill maestro bundle replaced
// `~/.claude/MAESTRO.md` injection. The shipped agent guidance now lives under
// `skills/bundled/` and is covered by `bundled-skill-templates.test.ts`.
// These tests continue to guard the per-project `.maestro/AGENTS.md` bootstrap
// content that `maestro init` writes, which is a separate surface.

describe("PROJECT_BOOTSTRAP_TEMPLATES", () => {
  it("ships a TOC-style bootstrap AGENTS template under the size budget", () => {
    const agentsTemplate = PROJECT_BOOTSTRAP_TEMPLATES.find((template) => template.path === ".maestro/AGENTS.md");
    expect(agentsTemplate).toBeDefined();
    const content = agentsTemplate!.content;
    const lineCount = (content.endsWith("\n") ? content.slice(0, -1) : content).split("\n").length;
    expect(lineCount).toBeLessThanOrEqual(100);
    expect(content).toContain("long-running agent harness");
    expect(content).toContain("docs/harness-positioning.md");
    expect(content).toContain("docs/cli-reference.md");
  });

  it("includes pointer-style task-system entries in the bootstrap AGENTS template", () => {
    const agentsTemplate = PROJECT_BOOTSTRAP_TEMPLATES.find((template) => template.path === ".maestro/AGENTS.md");
    const content = agentsTemplate!.content;
    expect(content).toContain(".maestro/tasks/contracts/");
    expect(content).toContain(".maestro/tasks/contract-templates/");
    expect(content).toContain("maestro intake");
    expect(content).toContain("maestro plan check");
    expect(content).toContain("maestro verdict request");
    expect(content).toContain("maestro recover");
  });

  it("ships the default contract draft template in bootstrap assets", () => {
    const template = PROJECT_BOOTSTRAP_TEMPLATES.find(
      (entry) => entry.path === ".maestro/tasks/contract-templates/default.md",
    );
    expect(template?.content).toContain("intent:");
    expect(template?.content).toContain("filesExpected:");
    expect(template?.content).toContain("doneWhen:");
  });

  it("ships a .maestro/MAESTRO.md read-order compass", () => {
    const template = PROJECT_BOOTSTRAP_TEMPLATES.find(
      (entry) => entry.path === ".maestro/MAESTRO.md",
    );
    expect(template).toBeDefined();
    expect(template?.content).toContain("Read Order");
    expect(template?.content).toContain("AGENTS.md");
    expect(template?.content).toContain("maestro intake");
    expect(template?.content).toContain("Two outputs per task");
  });

  it("points the bootstrap AGENTS template at the root AGENTS.md", () => {
    const template = PROJECT_BOOTSTRAP_TEMPLATES.find(
      (entry) => entry.path === ".maestro/AGENTS.md",
    );
    expect(template?.content).toContain("project root");
    expect(template?.content).toContain("AGENTS.md");
  });
});
