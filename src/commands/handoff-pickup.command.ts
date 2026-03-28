import type { Command } from "commander";
import { getServices } from "../services.js";
import { pickupHandoff, listHandoffs } from "../usecases/pickup-handoff.usecase.js";
import { output } from "../lib/output.js";
import type { HandoffEnvelope } from "../domain/types.js";

export function registerHandoffPickupCommand(program: Command): void {
  program
    .command("handoff-pickup")
    .description("Read the latest (or specified) handoff payload")
    .addHelpText("after", `
Examples:
  maestro handoff-pickup --json
  maestro handoff-pickup --markdown
  maestro handoff-pickup --list
  maestro handoff-pickup --id 2026-03-28-001
`)
    .option("--id <handoff-id>", "Pick up a specific handoff by ID")
    .option("--list", "List available handoffs without picking one up")
    .option("--markdown", "Output as readable markdown")
    .option("--json", "Output as JSON (default)")
    .action(async (opts) => {
      const services = getServices();
      const isJson = opts.json ?? (!opts.markdown && program.opts().json !== false);

      if (opts.list) {
        const all = await listHandoffs(services.handoffStore);
        output(isJson ?? true, all, (list) =>
          list.length === 0
            ? ["No handoffs found"]
            : [
                `${list.length} handoff(s):`,
                ...list.map(
                  (e) =>
                    `  ${e.handoff.id}  [${e.status}]  ${e.handoff.message}`,
                ),
              ],
        );
        return;
      }

      const envelope = await pickupHandoff(services.handoffStore, {
        id: opts.id,
        agent: "unknown",
      });

      if (opts.markdown) {
        console.log(formatMarkdown(envelope));
      } else {
        output(true, envelope, () => []);
      }
    });
}

function formatMarkdown(envelope: HandoffEnvelope): string {
  const h = envelope.handoff;
  const lines: string[] = [
    `# Handoff Briefing`,
    "",
    `**ID:** ${h.id}`,
    `**From:** ${h.session.agent}`,
    `**Branch:** ${h.git.branch}`,
    `**CASS indexed:** ${h.session.cassIndexed}`,
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

  if (h.session.cassIndexed) {
    lines.push(
      "",
      "## Session History",
      "",
      `Full conversation available via CASS:`,
      `  maestro handoff-dig "<your query>" --id ${h.id}`,
    );
  }

  return lines.join("\n");
}
