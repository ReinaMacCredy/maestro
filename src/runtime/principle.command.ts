import { Command } from "commander";
import { buildCoreServices } from "../providers/build-services.js";
import {
  CorrectionNotFoundError,
  CorrectionNotLintViolationError,
  principlePromote,
} from "../service/principle-promote.usecase.js";
import { PrincipleParseError } from "../types/principle.js";

export interface PrincipleCommandOptions {
  readonly resolveRepoRoot: () => string;
}

function findOrCreatePrincipleCommand(program: Command): Command {
  const existing = program.commands.find((c) => c.name() === "principle");
  if (existing) return existing;
  return program.command("principle").description("Behavioral principles");
}

function reportError(verb: string, err: unknown): void {
  if (
    err instanceof CorrectionNotFoundError ||
    err instanceof CorrectionNotLintViolationError ||
    err instanceof PrincipleParseError
  ) {
    console.error(`maestro ${verb}: ${(err as Error).message}`);
    process.exitCode = 1;
    return;
  }
  throw err;
}

export function registerPrincipleCommands(
  program: Command,
  opts: PrincipleCommandOptions,
): void {
  const principle = findOrCreatePrincipleCommand(program);

  principle
    .command("promote <correctionId>")
    .description(
      "Materialize docs/principles/<slug>.md from a lint-violation evidence row (slug from rule_id)",
    )
    .option("--json", "emit JSON instead of text")
    .action(async function (this: Command, correctionId: string, flags: { json?: boolean }): Promise<void> {
      try {
        const repoRoot = opts.resolveRepoRoot();
        const services = buildCoreServices({ repoRoot });
        const result = await principlePromote(
          {
            evidenceStore: services.evidenceStore,
            principlesStore: services.principlesStore,
          },
          { correction_id: correctionId },
        );
        const wantJson = flags.json === true || this.optsWithGlobals().json === true;
        if (wantJson) {
          console.log(JSON.stringify(result, null, 2));
          return;
        }
        console.log(`${result.slug} -> ${result.path}`);
        console.log(`  from ${correctionId} (rule_id=${result.rule_id})`);
      } catch (err) {
        reportError("principle promote", err);
      }
    });
}
