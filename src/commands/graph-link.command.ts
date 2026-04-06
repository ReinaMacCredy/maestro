import { basename } from "node:path";
import type { Command } from "commander";
import { MaestroError } from "../domain/errors.js";
import { output } from "../lib/output.js";
import { getServices } from "../services.js";
import { linkProjects, type LinkResult } from "../usecases/graph-link.usecase.js";
import type { GraphRelation } from "../domain/memory-types.js";

const VALID_RELATIONS: readonly GraphRelation[] = ["exposes", "consumes", "shared-types"];

export function registerGraphLinkCommand(program: Command): void {
  program
    .command("graph-link")
    .description("Link this project to another in the project graph")
    .addHelpText("after", `
Examples:
  maestro graph-link maestro-web --consumes
  maestro graph-link shared-types --exposes --via "types/mission.ts"
  maestro graph-link maestro-api --shared-types --json
`)
    .argument("<target>", "Target project name")
    .option("--consumes", "This project consumes the target")
    .option("--exposes", "This project exposes to the target")
    .option("--shared-types", "Projects share types")
    .option("--via <detail>", "Additional detail about the relationship")
    .option("--json", "Output as JSON")
    .action(async (target: string, opts) => {
      const isJson = opts.json ?? program.opts().json;

      let relation: GraphRelation | undefined;
      if (opts.consumes) relation = "consumes";
      else if (opts.exposes) relation = "exposes";
      else if (opts.sharedTypes) relation = "shared-types";

      if (!relation) {
        throw new MaestroError(
          "Specify a relation: --consumes, --exposes, or --shared-types",
          VALID_RELATIONS.map((r) => `maestro graph-link ${target} --${r}`),
        );
      }

      const services = getServices();
      const cwd = process.cwd();

      const result = await linkProjects(services.projectGraphStore, {
        targetName: target,
        relation,
        detail: opts.via,
        currentPath: cwd,
        currentName: basename(cwd),
      });

      output(isJson, result, formatLink);
    });
}

function formatLink(result: LinkResult): string[] {
  return [
    "[ok] Project linked",
    `  ${result.edge.from} --[${result.edge.relation}]--> ${result.edge.to}`,
    ...(result.edge.detail ? [`  Via: ${result.edge.detail}`] : []),
    ...(result.nodesAdded > 0 ? [`  (${result.nodesAdded} new node(s) added)`] : []),
  ];
}
