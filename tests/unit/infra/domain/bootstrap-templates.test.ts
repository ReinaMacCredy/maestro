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

  it("documents task prune for bounding local candidates and completed continuations", () => {
    expect(AGENT_INSTRUCTION_BLOCK).toContain("maestro task prune");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("local-only per-machine state");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("--candidates-only");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("--continuations-only");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("--dry-run");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("keep newest 500 per kind");
  });

  it("documents the task contract workflow", () => {
    expect(AGENT_INSTRUCTION_BLOCK).toContain("## Task Contracts");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("maestro task contract new <id>");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("--from default");
    expect(AGENT_INSTRUCTION_BLOCK).toContain(".maestro/tasks/contract-templates/default.md");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("new/edit/lock/discard/amend/criteria");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("maestro task contract lock <id>");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("maestro task contract show <id>");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("maestro task contract verdict <id>");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("maestro task contract list");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("maestro task contract discard <id>");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("maestro task contract amend <id> --reason");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("maestro task contract criteria mark <id> <criterionId> --met");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("maestro task contract criteria add <id>");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("maestro task contract criteria remove <id> <criterionId>");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("--session <id>");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("--strict");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("--no-contract");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("Reopening a completed task reactivates its contract");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("Previously amended contracts reopen as amended");
    expect(AGENT_INSTRUCTION_BLOCK).toContain("active contracts follow the new owner unless policy blocks reclaim");
  });

  it("mirrors contract guidance into the bootstrap AGENTS template", () => {
    const agentsTemplate = PROJECT_BOOTSTRAP_TEMPLATES.find((template) => template.path === ".maestro/AGENTS.md");
    expect(agentsTemplate?.content).toContain(".maestro/tasks/contracts/");
    expect(agentsTemplate?.content).toContain(".maestro/tasks/contract-templates/");
    expect(agentsTemplate?.content).toContain("maestro task contract new <id>");
    expect(agentsTemplate?.content).toContain("maestro task contract lock <id>");
    expect(agentsTemplate?.content).toContain("new/edit/lock/discard/amend/criteria");
    expect(agentsTemplate?.content).toContain("maestro task contract verdict <id>");
    expect(agentsTemplate?.content).toContain("maestro task contract amend <id> --reason");
    expect(agentsTemplate?.content).toContain("maestro task contract criteria mark <id> <criterionId> --met");
    expect(agentsTemplate?.content).toContain("--session <id>");
    expect(agentsTemplate?.content).toContain("--strict");
    expect(agentsTemplate?.content).toContain("stored verdict");
    expect(agentsTemplate?.content).toContain("contracts.overlapPolicy: annotate");
    expect(agentsTemplate?.content).toContain("reactivates its contract");
    expect(agentsTemplate?.content).toContain("Previously amended contracts reopen as amended");
    expect(agentsTemplate?.content).toContain("staleReclaimContractPolicy: block");
  });

  it("mirrors the PR 35 shared task loop guidance into the bootstrap AGENTS template", () => {
    const agentsTemplate = PROJECT_BOOTSTRAP_TEMPLATES.find((template) => template.path === ".maestro/AGENTS.md");
    expect(agentsTemplate?.content).toContain("## Shared Task Loop");
    expect(agentsTemplate?.content).toContain("maestro task ready --json --compact --limit 5");
    expect(agentsTemplate?.content).toContain("maestro task show <id>");
    expect(agentsTemplate?.content).toContain("maestro task claim <id> --contract-required");
    expect(agentsTemplate?.content).toContain("maestro task claim <id> --no-contract");
    expect(agentsTemplate?.content).toContain('--summary "<receipt summary>"');
    expect(agentsTemplate?.content).toContain('--surprise "<gotcha>"');
    expect(agentsTemplate?.content).toContain("--verified-by <name>");
    expect(agentsTemplate?.content).toContain("maestro task similar <id>");
    expect(agentsTemplate?.content).toContain("maestro task mine");
    expect(agentsTemplate?.content).toContain("maestro task stuck [--older-than 4h]");
    expect(agentsTemplate?.content).toContain("maestro task heartbeat <id>");
    expect(agentsTemplate?.content).toContain("maestro task claim <id> [--stale-after 4h]");
    expect(agentsTemplate?.content).toContain("MAESTRO_TASK_SILENT=1");
    expect(agentsTemplate?.content).toContain("maestro task prune --dry-run");
    expect(agentsTemplate?.content).toContain(".maestro/tasks/NOW.md");
  });

  it("ships the default contract draft template in bootstrap assets", () => {
    const template = PROJECT_BOOTSTRAP_TEMPLATES.find(
      (entry) => entry.path === ".maestro/tasks/contract-templates/default.md",
    );
    expect(template?.content).toContain("intent:");
    expect(template?.content).toContain("filesExpected:");
    expect(template?.content).toContain("doneWhen:");
  });

  it("documents the native handoff launcher and launch artifact path", () => {
    expect(AGENT_INSTRUCTION_BLOCK).toContain('maestro handoff "Implement <featureId> for mission <id>"');
    expect(AGENT_INSTRUCTION_BLOCK).toContain(".maestro/launches/<id>/");
  });
});
