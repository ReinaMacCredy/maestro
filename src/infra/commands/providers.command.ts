import type { Command } from "commander";
import { access } from "node:fs/promises";
import { listProviders, getProvider, type ProviderDescriptor } from "@/infra/domain/providers.js";
import { output, resolveJsonFlag } from "@/shared/lib/output.js";
import { MaestroError } from "@/shared/errors.js";
import { execArgv } from "@/shared/lib/shell.js";
import { readText } from "@/shared/lib/fs.js";
import { resolveAgentSkillsSharedRoot } from "@/shared/domain/defaults.js";
import { parseYaml } from "@/shared/lib/yaml.js";

interface ProviderDoctorResult extends ProviderDescriptor {
  readonly detected: boolean;
  readonly binaryFound?: boolean;
  readonly issues: readonly string[];
}

export function registerProvidersCommand(program: Command): void {
  const providers = program
    .command("providers")
    .description("Inspect Maestro runtime and skill target providers");

  providers
    .command("list")
    .option("--json", "Output as JSON")
    .action(async (opts): Promise<void> => {
      const isJson = resolveJsonFlag(opts, program);
      const rows = listProviders();
      output(isJson, rows, formatProviderList);
    });

  providers
    .command("doctor")
    .argument("[provider]", "Provider id or slug")
    .option("--json", "Output as JSON")
    .action(async (provider: string | undefined, opts): Promise<void> => {
      const isJson = resolveJsonFlag(opts, program);
      const selected = provider
        ? [getProvider(provider)].filter((p): p is ProviderDescriptor => p !== undefined)
        : [...listProviders()];
      if (provider && selected.length === 0) {
        throw new MaestroError(`Unknown provider '${provider}'`, [
          "Valid providers: codex, claude, hermes, agentskills",
        ]);
      }
      const checked = await Promise.all(selected.map(doctorProvider));
      output(isJson, provider ? checked[0]! : checked, (value) =>
        Array.isArray(value) ? formatDoctorList(value) : formatDoctorList([value])
      );
    });
}

function formatProviderList(providers: readonly ProviderDescriptor[]): string[] {
  return [
    `[ok] ${providers.length} provider(s)`,
    ...providers.map((provider) =>
      `  ${provider.id}  runtime=${provider.runtime ? "yes" : "no"}  skills=${provider.skillTarget ? "yes" : "no"}  root=${provider.skillsRoot}`
    ),
  ];
}

async function doctorProvider(provider: ProviderDescriptor): Promise<ProviderDoctorResult> {
  const issues: string[] = [];
  const configExists = await exists(provider.configPath);
  const skillsRootExists = await exists(provider.skillsRoot);
  if (!configExists && provider.id !== "agentskills") {
    issues.push(`config missing: ${provider.configPath}`);
  }
  if (!skillsRootExists) {
    issues.push(`skills root missing: ${provider.skillsRoot}`);
  }

  let binaryFound: boolean | undefined;
  if (provider.binary) {
    const version = await execArgv([provider.binary, "--version"], { timeout: 3_000 });
    binaryFound = version.exitCode === 0;
    if (!binaryFound) issues.push(`binary not found or not runnable: ${provider.binary}`);
  }

  if (provider.id === "hermes" && configExists) {
    const raw = await readText(provider.configPath);
    try {
      const parsed = raw ? parseYaml<Record<string, unknown>>(raw) : {};
      const externalDirs = (parsed.skills as { external_dirs?: unknown } | undefined)?.external_dirs;
      const sharedRoot = resolveAgentSkillsSharedRoot();
      if (!Array.isArray(externalDirs) || !externalDirs.includes(sharedRoot)) {
        issues.push(`Hermes skills.external_dirs does not include ${sharedRoot}`);
      }
    } catch (error) {
      issues.push(`Hermes config is not valid YAML: ${error instanceof Error ? error.message : String(error)}`);
    }
  }

  return {
    ...provider,
    detected: issues.length === 0,
    ...(binaryFound !== undefined ? { binaryFound } : {}),
    issues,
  };
}

function formatDoctorList(results: readonly ProviderDoctorResult[]): string[] {
  return [
    `[ok] ${results.length} provider check(s)`,
    ...results.flatMap((result) => [
      `  ${result.id}: ${result.detected ? "ok" : "needs-attention"}`,
      ...result.issues.map((issue) => `    - ${issue}`),
    ]),
  ];
}

async function exists(path: string): Promise<boolean> {
  try {
    await access(path);
    return true;
  } catch {
    return false;
  }
}
