import type { Command } from "commander";
import { buildV2Services } from "../providers/build-services.js";
import { specNew, InvalidSpecSlugError } from "../service/spec-new.usecase.js";
import { specValidate } from "../service/spec-validate.usecase.js";
import {
  SpecAlreadyExistsError,
  SpecParseError,
} from "../repo/spec-store.port.js";
import { isSpecMode } from "../types/product-spec.js";

export interface SpecCommandV2Options {
  readonly resolveRepoRoot: () => string;
}

function findOrCreateSpecCommand(program: Command): Command {
  const existing = program.commands.find((c) => c.name() === "spec");
  if (existing) return existing;
  return program.command("spec").description("Product-spec authoring (v2)");
}

export function registerSpecV2Commands(program: Command, opts: SpecCommandV2Options): void {
  const spec = findOrCreateSpecCommand(program);

  spec
    .command("new <slug>")
    .description("Scaffold a new product-spec markdown file at .maestro/specs/<slug>.md")
    .option("--title <title>", "human-readable title for the spec body")
    .option("--mode <mode>", "light | heavy (default: light)")
    .action(
      async (
        slug: string,
        flags: { title?: string; mode?: string },
      ) => {
        try {
          const repoRoot = opts.resolveRepoRoot();
          const services = buildV2Services({ repoRoot });
          const mode = flags.mode;
          if (mode !== undefined && !isSpecMode(mode)) {
            console.error(`maestro spec new: invalid --mode value "${mode}" (expected light | heavy)`);
            process.exitCode = 1;
            return;
          }
          const result = await specNew(
            { store: services.specStore },
            { slug, title: flags.title, mode },
          );
          console.log(`Created spec at ${result.path}`);
        } catch (err) {
          if (err instanceof InvalidSpecSlugError || err instanceof SpecAlreadyExistsError) {
            console.error(`maestro spec new: ${err.message}`);
            process.exitCode = 1;
            return;
          }
          throw err;
        }
      },
    );

  spec
    .command("validate <path>")
    .description("Validate a product-spec markdown file against the frontmatter schema")
    .action(async (pathArg: string) => {
      try {
        const repoRoot = opts.resolveRepoRoot();
        const result = await specValidate({ repoRoot }, pathArg);
        if (result.valid) {
          console.log(`Spec ${pathArg} is valid`);
          return;
        }
        for (const message of result.errors) {
          console.error(`maestro spec validate: ${message}`);
        }
        process.exitCode = 1;
      } catch (err) {
        if (err instanceof SpecParseError) {
          console.error(`maestro spec validate: ${err.message}`);
          process.exitCode = 1;
          return;
        }
        throw err;
      }
    });
}
