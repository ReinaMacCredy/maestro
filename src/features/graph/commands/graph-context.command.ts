import { basename } from "node:path";
import type { Command } from "commander";
import { output } from "@/lib/output.js";
import { getServices } from "@/services.js";
import { getGraphContext, type GraphContext } from "../usecases/graph-context.usecase.js";

export function registerGraphContextCommand(program: Command): void {
  program
    .command("graph-context")
    .description("Show this project's relationships in the project graph")
    .addHelpText("after", `
Examples:
  maestro graph-context
  maestro graph-context --json
`)
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const services = getServices();
      const isJson = opts.json ?? program.opts().json;
      const currentName = basename(process.cwd());

      const ctx = await getGraphContext(services.projectGraphStore, currentName);

      output(isJson, ctx, formatContext);
    });
}

function formatContext(ctx: GraphContext): string[] {
  const lines: string[] = [`Project Graph (${ctx.totalProjects} projects, ${ctx.totalEdges} links)`];

  if (!ctx.currentProject) {
    lines.push("", "Current project not in graph. Use `maestro graph-link` to add relationships.");
    return lines;
  }

  lines.push(`  Current: ${ctx.currentProject.name} (${ctx.currentProject.path})`);

  if (ctx.relationships.length === 0) {
    lines.push("", "  No relationships defined");
    return lines;
  }

  lines.push("");
  for (const rel of ctx.relationships) {
    const arrow = rel.direction === "outgoing" ? "-->" : "<--";
    lines.push(`  ${arrow} ${rel.edge.relation}: ${rel.project.name}`);
    if (rel.edge.detail) lines.push(`      via: ${rel.edge.detail}`);
  }

  return lines;
}
