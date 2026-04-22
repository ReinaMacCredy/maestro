import type { Command } from "commander";
import { injectAgentBlocks } from "@/features/agent";
import { formatAgentResults, output } from "@/shared/lib/output.js";
import { installReleaseBinary } from "../usecases/install-release-binary.usecase.js";

export function registerUpdateCommand(program: Command): void {
  program
    .command("update")
    .description("Update maestro from the latest published release and/or refresh bundled agent skills")
    .option("--agents-only", "Only refresh bundled agent skills, skip binary download")
    .option("--version <version>", "Install a specific release version or tag")
    .option("--force", "Reinstall even when already on the latest published release")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const isJson = opts.json ?? program.opts().json;
      let binary:
        | {
          readonly binaryUpdated: boolean;
          readonly alreadyCurrent: boolean;
          readonly installPath: string;
          readonly tagName: string;
          readonly version: string;
          readonly assetName: string;
        }
        | undefined;

      if (!opts.agentsOnly) {
        binary = await installReleaseBinary({
          version: opts.version,
          force: opts.force,
        });
      }

      const agentResults = await injectAgentBlocks(process.cwd(), "home");

      output(isJson, { binary, agents: agentResults }, (r) => [
        describeBinaryUpdate(r.binary, opts.agentsOnly),
        ...(r.binary ? [
          `  --> Release: ${r.binary.tagName}`,
          `  --> Asset: ${r.binary.assetName}`,
          `  --> Installed to ${r.binary.installPath}`,
        ] : []),
        "",
        ...formatAgentResults(r.agents),
      ]);
    });
}

function describeBinaryUpdate(
  binary: {
    readonly binaryUpdated: boolean;
    readonly alreadyCurrent: boolean;
    readonly version: string;
  } | undefined,
  agentsOnly: boolean,
): string {
  if (agentsOnly) {
    return "[--] Binary skipped (--agents-only)";
  }

  if (!binary) {
    return "[--] Binary skipped";
  }

  if (binary.alreadyCurrent) {
    return `[ok] Binary already current at ${binary.version}`;
  }

  if (binary.binaryUpdated) {
    return `[ok] Binary updated to ${binary.version}`;
  }

  return "[--] Binary skipped";
}
