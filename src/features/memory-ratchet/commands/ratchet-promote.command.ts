import type { Command } from "commander";
import { MaestroError } from "@/shared/errors.js";
import { output } from "@/shared/lib/output.js";
import { getServices, type Services } from "@/services.js";
import { promoteToRatchet, type PromoteResult } from "../usecases/ratchet-promote.usecase.js";

interface RatchetPromoteCommandDeps {
  readonly getServices: () => Pick<Services, "correctionStore" | "ratchetStore">;
}

export function registerRatchetPromoteCommand(
  program: Command,
  deps: RatchetPromoteCommandDeps = { getServices },
): void {
  program
    .command("ratchet-promote")
    .description("Promote a correction to a ratchet assertion")
    .addHelpText("after", `
Examples:
  maestro ratchet-promote 2026-04-05-001 --check "npm install"
  maestro ratchet-promote 2026-04-05-001 --check "fire.*forget" --json
`)
    .argument("<correctionId>", "ID of the correction to promote")
    .option("--check <pattern>", "Regex pattern to check for violations")
    .option("--json", "Output as JSON")
    .action(async (correctionId: string, opts): Promise<void> => {
      if (!opts.check) {
        throw new MaestroError("--check is required", [
          'maestro ratchet-promote <id> --check "violation pattern"',
        ]);
      }

      const services = deps.getServices();
      const isJson = opts.json ?? program.opts().json;

      const result = await promoteToRatchet(
        services.correctionStore,
        services.ratchetStore,
        { correctionId, check: opts.check },
      );

      output(isJson, result, formatPromote);
    });
}

function formatPromote(result: PromoteResult): string[] {
  return [
    "[ok] Correction promoted to ratchet",
    `  Assertion ID: ${result.assertion.id}`,
    `  Rule: ${result.assertion.rule}`,
    `  Check: ${result.assertion.check}`,
  ];
}
