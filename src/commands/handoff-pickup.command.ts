import type { Command } from "commander";
import { getServices } from "../services.js";
import { pickupHandoff } from "../usecases/pickup-handoff.usecase.js";
import { output } from "../lib/output.js";
import type { HandoffEnvelope } from "../domain/types.js";

export function registerHandoffPickupCommand(program: Command): void {
  program
    .command("handoff-pickup")
    .description("Read the latest (or specified) handoff payload")
    .addHelpText("after", `
Examples:
  maestro handoff-pickup --json                    # view latest pending (safe, repeatable)
  maestro handoff-pickup --claim --agent codex     # consume and mark as picked-up
  maestro handoff-pickup --claim --json            # consume (anonymous)
  maestro handoff-pickup --markdown                # view as readable briefing
  maestro handoff-pickup --id 2026-03-28-001       # view a specific handoff
`)
    .option("--id <handoff-id>", "View a specific handoff by ID")
    .option("--claim", "Mark the handoff as picked-up (default: peek only)")
    .option("--agent <name>", "Agent name for attribution when claiming")
    .option("--markdown", "Output as readable markdown")
    .option("--json", "Output as JSON (default)")
    .action(async (opts) => {
      const services = getServices();

      const peek = !opts.claim;
      const envelope = await pickupHandoff(services.handoffStore, {
        id: opts.id,
        agent: opts.agent ?? "unknown",
        peek,
      });

      if (opts.markdown) {
        console.log(formatMarkdown(envelope));
      } else {
        const isJson = opts.json ?? program.opts().json;
        output(isJson ?? true, envelope, (e) => formatText(e));
      }
    });
}

function formatText(envelope: HandoffEnvelope): string[] {
  const h = envelope.handoff;
  return [
    `${h.id}  [${envelope.status}]`,
    `  ${h.message}`,
    `  From: ${h.session.agent}  Branch: ${h.git.branch}`,
    "",
    "Sitrep:",
    h.sitrep,
    "",
    "Quickstart:",
    h.quickstart,
  ];
}

function formatMarkdown(envelope: HandoffEnvelope): string {
  const h = envelope.handoff;
  const lines: string[] = [
    `# Handoff Briefing`,
    "",
    `**ID:** ${h.id}`,
    `**From:** ${h.session.agent}`,
    `**Branch:** ${h.git.branch}`,
    `**CASS session:** ${h.session.sourcePath ? "available" : "none"}`,
    "",
    "## Sitrep",
    "",
    h.sitrep,
    "",
    "## Quickstart",
    "",
    h.quickstart,
    "",
    "## Git State",
    "",
    `- Branch: ${h.git.branch}`,
    `- Working tree clean: ${h.git.workingTreeClean}`,
    `- Diff: ${h.git.diffStat}`,
  ];

  if (h.git.changedFiles.length > 0) {
    lines.push("", "### Changed Files", "");
    for (const f of h.git.changedFiles) {
      lines.push(`- ${f}`);
    }
  }

  if (h.git.recentCommits.length > 0) {
    lines.push("", "### Recent Commits", "");
    for (const c of h.git.recentCommits) {
      lines.push(`- ${c}`);
    }
  }

  if (h.plan) {
    lines.push(
      "",
      "## Plan",
      "",
      `Completed: ${h.plan.completed} / ${h.plan.completed + h.plan.remaining}`,
    );
    for (const t of h.plan.tasks) {
      const marker = t.status === "done" ? "[x]" : t.status === "blocked" ? "[~]" : "[ ]";
      lines.push(`- ${marker} ${t.description}`);
    }
  }

  if (h.session.sourcePath) {
    lines.push(
      "",
      "## Session History",
      "",
      `Full conversation available via CASS (indexes on first search):`,
      `  maestro handoff-dig "<your query>" --id ${h.id}`,
    );
  }

  return lines.join("\n");
}
