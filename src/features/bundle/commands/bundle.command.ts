import type { Command } from "commander";
import { MaestroError } from "@/shared/errors.js";
import { output, resolveJsonFlag } from "@/shared/lib/output.js";
import { getServices } from "@/services.js";
import { exportBundle } from "../usecases/export-bundle.usecase.js";
import { inspectBundle } from "../usecases/inspect-bundle.usecase.js";
import type {
  BundleExportResult,
  BundleManifest,
  BundleRedactScope,
} from "../domain/bundle-types.js";

const VALID_REDACT_SCOPES: readonly BundleRedactScope[] = [
  "memory",
  "prompts",
  "replies",
];

export function registerBundleCommand(program: Command): void {
  const bundleCmd = program
    .command("bundle")
    .description("Package a mission and its artifacts as a portable archive")
    .option("--json", "Output as JSON");

  registerExportCommand(bundleCmd, program);
  registerInspectCommand(bundleCmd, program);
}

function registerExportCommand(bundleCmd: Command, program: Command): void {
  bundleCmd
    .command("export <missionId>")
    .description("Export a mission as a .mission.tar.gz bundle")
    .addHelpText("after", `
Examples:
  maestro bundle export 2026-04-15-001
  maestro bundle export 2026-04-15-001 --out ./review.mission.tar.gz
  maestro bundle export 2026-04-15-001 --base main --redact memory,prompts
`)
    .option("--out <path>", "Output path for the bundle archive")
    .option("--base <ref>", "Include diff.patch computed from <ref>..HEAD")
    .option(
      "--redact <scope>",
      `Comma-separated redaction scopes (${VALID_REDACT_SCOPES.join("|")})`,
    )
    .option("--json", "Output as JSON")
    .action(async (missionId: string, opts) => {
      const isJson = resolveJsonFlag(opts, program);
      const services = getServices();
      const redact = parseRedactScopes(opts.redact);

      const result = await exportBundle(
        {
          missions: services.missions,
          replyStore: services.replyStore,
          handoffStore: services.handoffStore,
          archive: services.archive,
          sessionDetect: services.sessionDetect,
        },
        {
          missionId,
          projectDir: process.cwd(),
          options: {
            ...(opts.out !== undefined && { out: opts.out }),
            ...(opts.base !== undefined && { base: opts.base }),
            redact,
          },
        },
      );

      output(isJson, result, formatExportResult);
    });
}

function registerInspectCommand(bundleCmd: Command, program: Command): void {
  bundleCmd
    .command("inspect <path>")
    .description("Print the manifest of a mission bundle without extracting it")
    .addHelpText("after", `
Examples:
  maestro bundle inspect ./2026-04-15-001-20260415-120000.mission.tar.gz
  maestro bundle inspect ./review.mission.tar.gz --json
`)
    .option("--json", "Output as JSON")
    .action(async (path: string, opts) => {
      const isJson = resolveJsonFlag(opts, program);
      const services = getServices();

      const manifest = await inspectBundle({ archive: services.archive }, path);

      output(isJson, manifest, formatInspectResult);
    });
}

function parseRedactScopes(raw: string | undefined): BundleRedactScope[] {
  if (!raw) return [];
  const scopes = raw
    .split(",")
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
  const invalid = scopes.filter(
    (s): s is string => !(VALID_REDACT_SCOPES as readonly string[]).includes(s),
  );
  if (invalid.length > 0) {
    throw new MaestroError(`Unknown --redact scope: ${invalid.join(", ")}`, [
      `Valid scopes: ${VALID_REDACT_SCOPES.join(", ")}`,
    ]);
  }
  return scopes as BundleRedactScope[];
}

function formatExportResult(result: BundleExportResult): string[] {
  const { manifest, outputPath, bytes } = result;
  const lines: string[] = [
    "[ok] Bundle exported",
    `  Mission: ${manifest.mission.id} (${manifest.mission.title})`,
    `  Status:  ${manifest.mission.status}`,
    `  Output:  ${outputPath}`,
    `  Size:    ${formatBytes(bytes)}`,
    formatBundleStats(manifest, { includeMilestones: false }),
    `  Principles: ${manifest.stats.principlesSnapshot} principles, ${manifest.stats.outcomesSnapshot} outcomes`,
  ];
  if (manifest.stats.memorySnapshot) {
    lines.push(
      `  Memory:  ${manifest.stats.memorySnapshot.corrections} corrections, ${manifest.stats.memorySnapshot.learnings} learnings`,
    );
  } else {
    lines.push("  Memory:  (redacted)");
  }
  if (manifest.redacted.length > 0) {
    lines.push(`  Redacted: ${manifest.redacted.join(", ")}`);
  }
  if (manifest.gitPatch) {
    lines.push(
      `  Patch:   ${manifest.gitPatch.commits} commits from ${manifest.gitPatch.base} (${formatBytes(manifest.gitPatch.bytes)})`,
    );
  }
  return lines;
}

function formatInspectResult(manifest: BundleManifest): string[] {
  const lines: string[] = [
    `Bundle ${manifest.bundleId}`,
    `  Schema:  v${manifest.schemaVersion}`,
    `  Created: ${manifest.createdAt}${manifest.createdBy ? ` by ${manifest.createdBy}` : ""}`,
    `  Maestro: ${manifest.maestroVersion}`,
    `  Mission: ${manifest.mission.id} (${manifest.mission.title})`,
    `  Status:  ${manifest.mission.status}`,
    formatBundleStats(manifest, { includeMilestones: true }),
    `  Principles: ${manifest.stats.principlesSnapshot} principles, ${manifest.stats.outcomesSnapshot} outcomes`,
  ];
  if (manifest.stats.memorySnapshot) {
    lines.push(
      `  Memory:  ${manifest.stats.memorySnapshot.corrections} corrections, ${manifest.stats.memorySnapshot.learnings} learnings`,
    );
  } else {
    lines.push("  Memory:  (redacted)");
  }
  if (manifest.redacted.length > 0) {
    lines.push(`  Redacted: ${manifest.redacted.join(", ")}`);
  }
  if (manifest.gitPatch) {
    lines.push(
      `  Patch:   ${manifest.gitPatch.commits} commits from ${manifest.gitPatch.base} (${formatBytes(manifest.gitPatch.bytes)})`,
    );
  }
  return lines;
}

function formatBundleStats(
  manifest: BundleManifest,
  options: {
    readonly includeMilestones: boolean;
  },
): string {
  const milestoneSegment = options.includeMilestones
    ? ` / ${manifest.stats.milestones} mile`
    : "";
  return `  Stats:   ${manifest.stats.features} feat${milestoneSegment} / ${manifest.stats.assertions} asrt / ${manifest.stats.agents} work / ${manifest.stats.replies} reply / ${manifest.stats.handoffs} handoff / ${manifest.stats.checkpoints} chkpt`;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}
