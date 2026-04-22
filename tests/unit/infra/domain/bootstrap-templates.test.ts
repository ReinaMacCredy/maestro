import { describe, expect, it } from "bun:test";
import { PROJECT_BOOTSTRAP_TEMPLATES } from "@/infra/domain/bootstrap-templates.js";

// AGENT_INSTRUCTION_BLOCK was deleted when the 5-skill maestro bundle replaced
// `~/.claude/MAESTRO.md` injection. The shipped agent guidance now lives under
// `skills/bundled/` and is covered by `bundled-skill-templates.test.ts`.
// These tests continue to guard the per-project `.maestro/AGENTS.md` bootstrap
// content that `maestro init` writes, which is a separate surface.

describe("PROJECT_BOOTSTRAP_TEMPLATES", () => {
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
});
