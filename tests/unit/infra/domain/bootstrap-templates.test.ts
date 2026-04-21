import { describe, expect, it } from "bun:test";
import {
  AGENT_INSTRUCTION_BLOCK,
  PROJECT_BOOTSTRAP_TEMPLATES,
} from "@/infra/domain/bootstrap-templates.js";

describe("AGENT_INSTRUCTION_BLOCK", () => {
  it("documents the shared task coordination workflow", () => {
    expect(AGENT_INSTRUCTION_BLOCK).toContain("maestro task ready --json --compact --limit 5");
    expect(AGENT_INSTRUCTION_BLOCK).not.toContain("maestro task ready --json --limit 5");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("maestro task claim <id>");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("maestro task unclaim <id>");
  });

  it("advertises the in_progress shortcut on task create", () => {
    expect(AGENT_INSTRUCTION_BLOCK).toContain("--status pending|in_progress");
    expect(AGENT_INSTRUCTION_BLOCK).toContain(
      "add --status in_progress to start immediately",
    );
  });

  it("teaches the task plan batch pattern with name-slot references", () => {
    expect(AGENT_INSTRUCTION_BLOCK).toContain("Plan a batch of tasks upfront");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("maestro task plan --file");
    expect(AGENT_INSTRUCTION_BLOCK).toContain('"blockedBy": ["first"]');
    expect(AGENT_INSTRUCTION_BLOCK).toContain("--start <name>");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("batchId");
  });

  it("keeps the two non-obvious rules agents can't derive from --help", () => {
    expect(AGENT_INSTRUCTION_BLOCK).toContain("blockedBy");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("persisted verbatim");
  });

  it("points completion at update --status completed with a reason", () => {
    expect(AGENT_INSTRUCTION_BLOCK).toContain(
      "maestro task update <id> --status completed --reason",
    );
  });

  it("documents the task contract workflow", () => {
    expect(AGENT_INSTRUCTION_BLOCK).toContain("## Task Contracts");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("maestro task contract new <id>");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("maestro task contract lock <id>");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("maestro task contract show <id>");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("maestro task contract list");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("maestro task contract discard <id>");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("maestro task contract amend <id> --reason");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("maestro task contract criteria mark <id> <criterionId> --met");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("maestro task contract criteria add <id>");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("maestro task contract criteria remove <id> <criterionId>");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("--strict");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("--no-contract");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("Reopening a completed task relocks its contract");
  });

  it("mirrors contract guidance into the bootstrap AGENTS template", () => {
    const agentsTemplate = PROJECT_BOOTSTRAP_TEMPLATES.find((template) => template.path === ".maestro/AGENTS.md");
    expect(agentsTemplate?.content).toContain(".maestro/tasks/contracts/");
    expect(agentsTemplate?.content).toContain("maestro task contract new <id>");
    expect(agentsTemplate?.content).toContain("maestro task contract lock <id>");
    expect(agentsTemplate?.content).toContain("maestro task contract amend <id> --reason");
    expect(agentsTemplate?.content).toContain("maestro task contract criteria mark <id> <criterionId> --met");
    expect(agentsTemplate?.content).toContain("--strict");
    expect(agentsTemplate?.content).toContain("stored verdict");
    expect(agentsTemplate?.content).toContain("relocks its contract");
  });

  it("documents the native handoff launcher and launch artifact path", () => {
    expect(AGENT_INSTRUCTION_BLOCK).toContain('maestro handoff "Implement <featureId> for mission <id>"');
    expect(AGENT_INSTRUCTION_BLOCK).toContain(".maestro/launches/<id>/");
  });
});
