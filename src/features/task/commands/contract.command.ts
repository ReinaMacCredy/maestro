import { Command } from "commander";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { getServices } from "@/services.js";
import type { MaestroConfig } from "@/infra/domain/config-types.js";
import { MaestroError } from "@/shared/errors.js";
import { readTextOrStdin, writeText } from "@/shared/lib/fs.js";
import { output, resolveJsonFlag } from "@/shared/lib/output.js";
import { execArgv } from "@/shared/lib/shell.js";
import { parseYaml, stringifyYaml } from "@/shared/lib/yaml.js";
import { countMetCriteria } from "../domain/contract/contract-state.js";
import type {
  Contract,
  ContractConfigSnapshot,
  ContractScope,
  ContractStatus,
  DoneWhenCriterion,
} from "../domain/contract/contract-types.js";
import { createContract } from "../usecases/contract/create-contract.usecase.js";
import { discardContract } from "../usecases/contract/discard-contract.usecase.js";
import { listContracts } from "../usecases/contract/list-contracts.usecase.js";
import { lockContract } from "../usecases/contract/lock-contract.usecase.js";
import { showContract } from "../usecases/contract/show-contract.usecase.js";

const CONTRACT_STATUSES: readonly ContractStatus[] = [
  "draft",
  "locked",
  "amended",
  "fulfilled",
  "broken",
  "discarded",
] as const;

interface ContractDraftTemplate {
  readonly intent?: unknown;
  readonly scope?: {
    readonly filesExpected?: unknown;
    readonly filesForbidden?: unknown;
    readonly maxFilesTouched?: unknown;
  };
  readonly doneWhen?: unknown;
}

export function registerContractCommand(taskCmd: Command, program: Command): void {
  const contractCmd = taskCmd
    .command("contract")
    .description("Task contract draft, lock, and inspection commands");

  contractCmd
    .command("new <taskId>")
    .description("Create a draft contract for a task")
    .option("--from <path>", "Load a YAML template from a file ('-' for stdin)")
    .option("--editor <cmd>", "Open an editor command to write the draft YAML")
    .option("--silent", "Print only '<id> [ok]' (for scripts)")
    .option("--json", "Output as JSON")
    .action(async (taskId: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const cwd = process.cwd();
      const config = await services.config.load(cwd);
      const template = await loadContractDraftTemplate(opts.from, opts.editor);
      const contract = await createContract(services.taskStore, services.contractStore, {
        taskId,
        repoRoot: await resolveRepoRoot(cwd),
        intent: readTemplateIntent(template),
        scope: readTemplateScope(template),
        doneWhen: readTemplateDoneWhen(template),
        createdBy: await resolveContractActor(taskId),
        configSnapshot: buildContractConfigSnapshot(config),
      });

      if (emitContractSilentSuccess(isJson, opts, contract)) return;
      output(isJson, contract, formatContractDetail);
    });

  contractCmd
    .command("lock <ref>")
    .description("Lock a draft contract so completion can diff against it")
    .option("--silent", "Print only '<id> [ok]' (for scripts)")
    .option("--json", "Output as JSON")
    .action(async (ref: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const contract = await lockContract(services.contractStore, {
        ref,
        actorId: await resolveContractActor(ref),
        claimedAtCommit: await resolveHeadCommit(process.cwd()),
      });

      if (emitContractSilentSuccess(isJson, opts, contract)) return;
      output(isJson, contract, formatContractDetail);
    });

  contractCmd
    .command("show <ref>")
    .description("Show one contract by contract id or task id")
    .option("--format <format>", "Output format: md (default), json, or yaml")
    .option("--json", "Output as JSON")
    .action(async (ref: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const contract = await showContract(services.contractStore, ref);

      if (isJson || opts.format === "json") {
        output(true, contract, formatContractDetail);
        return;
      }
      if (opts.format === "yaml") {
        console.log(stringifyYaml(contract).trimEnd());
        return;
      }

      output(false, contract, formatContractDetail);
    });

  contractCmd
    .command("list")
    .description("List contracts")
    .option("--status <status>", `Filter by status (${CONTRACT_STATUSES.join("|")})`)
    .option("--task <id>", "Filter by task id")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const contracts = await listContracts(services.contractStore, {
        status: parseContractStatus(opts.status),
        taskId: opts.task,
      });
      output(isJson, contracts, formatContractList);
    });

  contractCmd
    .command("discard <ref>")
    .description("Discard a draft contract")
    .option("--silent", "Print only '<id> [ok]' (for scripts)")
    .option("--json", "Output as JSON")
    .action(async (ref: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const contract = await discardContract(services.contractStore, ref);

      if (emitContractSilentSuccess(isJson, opts, contract)) return;
      output(isJson, contract, formatContractDetail);
    });
}

function buildContractConfigSnapshot(config: MaestroConfig): ContractConfigSnapshot {
  return {
    strict: config.contracts?.strict ?? false,
    defaultMaxFilesTouched: config.contracts?.defaultMaxFilesTouched,
    overlapPolicy: config.contracts?.overlapPolicy ?? "fail",
    rebaseFallback: config.contracts?.rebaseFallback ?? "best-effort",
    staleReclaimContractPolicy: config.contracts?.staleReclaimContractPolicy ?? "inherit",
  };
}

async function loadContractDraftTemplate(
  fromPath: string | undefined,
  editorCommand: string | undefined,
): Promise<ContractDraftTemplate> {
  if (!fromPath && !editorCommand && !process.env.EDITOR && !process.env.VISUAL) {
    throw new MaestroError("Provide --from <path> or --editor <cmd> to create a contract draft", [
      "Example: maestro task contract new <id> --from contract.yaml",
      "Or set $EDITOR and rerun without --from",
    ]);
  }

  const resolvedEditor = editorCommand
    ?? (fromPath ? undefined : (process.env.EDITOR ?? process.env.VISUAL));
  const baseContent = fromPath
    ? await readDraftSource(fromPath)
    : defaultContractTemplate();
  const finalContent = resolvedEditor
    ? await editContractDraft(baseContent, resolvedEditor)
    : baseContent;

  try {
    return parseYaml<ContractDraftTemplate>(finalContent) ?? {};
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    throw new MaestroError(`Cannot parse contract draft YAML: ${detail}`, [
      "Fix the YAML syntax in the template and retry",
    ]);
  }
}

async function readDraftSource(path: string): Promise<string> {
  const raw = await readTextOrStdin(path);
  if (raw === undefined) {
    throw new MaestroError(`Contract template not found: ${path}`, [
      "Check the file path and retry",
      "Use '-' to read YAML from stdin",
    ]);
  }
  return raw;
}

async function editContractDraft(initialContent: string, editorCommand: string): Promise<string> {
  const draftDir = await mkdtemp(join(tmpdir(), "maestro-contract-draft-"));
  const draftPath = join(draftDir, "contract.yaml");
  await writeText(draftPath, initialContent);

  try {
    const result = Bun.spawnSync(["sh", "-lc", `${editorCommand} "${draftPath}"`], {
      stdio: ["inherit", "inherit", "inherit"],
    });
    if ((result.exitCode ?? 1) !== 0) {
      throw new MaestroError(`Editor command failed: ${editorCommand}`, [
        "Retry with a working editor command",
        "Or pass --from <path> to skip the editor",
      ]);
    }
    return await Bun.file(draftPath).text();
  } finally {
    await rm(draftDir, { recursive: true, force: true });
  }
}

function defaultContractTemplate(): string {
  return `${stringifyYaml({
    intent: "",
    scope: {
      filesExpected: [],
      filesForbidden: [],
    },
    doneWhen: [
      { text: "", kind: "manual" },
    ],
  }).trimEnd()}\n`;
}

function readTemplateIntent(template: ContractDraftTemplate): string {
  return typeof template.intent === "string" ? template.intent : "";
}

function readTemplateScope(template: ContractDraftTemplate): ContractScope {
  const scope = template.scope ?? {};
  return {
    filesExpected: readStringList(scope.filesExpected ?? [], "scope.filesExpected"),
    filesForbidden: readStringList(scope.filesForbidden ?? [], "scope.filesForbidden"),
    ...(scope.maxFilesTouched === undefined
      ? {}
      : { maxFilesTouched: readPositiveInteger(scope.maxFilesTouched, "scope.maxFilesTouched") }),
  };
}

function readTemplateDoneWhen(
  template: ContractDraftTemplate,
): readonly Array<{ readonly text: string; readonly kind?: DoneWhenCriterion["kind"] }> {
  if (template.doneWhen === undefined) return [];
  if (!Array.isArray(template.doneWhen)) {
    throw new MaestroError("Invalid contract draft: doneWhen must be an array", [
      "Use YAML like: doneWhen: [{ text: ..., kind: manual }]",
    ]);
  }

  return template.doneWhen.map((entry, index) => {
    if (typeof entry === "string") {
      return { text: entry };
    }
    if (typeof entry !== "object" || entry === null) {
      throw new MaestroError(`Invalid contract draft: doneWhen[${index}] must be a string or object`, [
        "Each doneWhen item needs at least a text field",
      ]);
    }

    const text = (entry as { text?: unknown }).text;
    const kind = (entry as { kind?: unknown }).kind;
    if (typeof text !== "string") {
      throw new MaestroError(`Invalid contract draft: doneWhen[${index}].text must be a string`, [
        "Each doneWhen item needs human-readable text",
      ]);
    }
    if (kind !== undefined && kind !== "manual" && kind !== "receipt-hint") {
      throw new MaestroError(`Invalid contract draft: doneWhen[${index}].kind must be manual or receipt-hint`);
    }

    return {
      text,
      kind,
    };
  });
}

function readStringList(value: unknown, field: string): readonly string[] {
  if (!Array.isArray(value) || !value.every((item) => typeof item === "string")) {
    throw new MaestroError(`Invalid contract draft: ${field} must be a string array`);
  }
  return value;
}

function readPositiveInteger(value: unknown, field: string): number {
  if (typeof value !== "number" || !Number.isInteger(value) || value <= 0) {
    throw new MaestroError(`Invalid contract draft: ${field} must be a positive integer`);
  }
  return value;
}

function parseContractStatus(value: string | undefined): ContractStatus | undefined {
  if (value === undefined) return undefined;
  if ((CONTRACT_STATUSES as readonly string[]).includes(value)) {
    return value as ContractStatus;
  }
  throw new MaestroError(`Invalid --status '${value}'`, [
    `Valid contract statuses: ${CONTRACT_STATUSES.join(", ")}`,
  ]);
}

function formatContractDetail(contract: Contract): string[] {
  const lines = [
    `Contract: ${contract.id}`,
    `  Task: ${contract.taskId}`,
    `  Status: ${contract.status}`,
    `  Intent: ${contract.intent || "(empty)"}`,
    `  Repo root: ${contract.repoRoot}`,
    `  Created: ${contract.createdAt}`,
    ...(contract.lockedAt ? [`  Locked at: ${contract.lockedAt}`] : []),
    ...(contract.claimedAtCommit ? [`  Claimed at commit: ${contract.claimedAtCommit}`] : []),
    ...(contract.discardedAt ? [`  Discarded at: ${contract.discardedAt}`] : []),
    `  Scope expected: ${contract.scope.filesExpected.join(", ") || "(none)"}`,
    `  Scope forbidden: ${contract.scope.filesForbidden.join(", ") || "(none)"}`,
    `  Done when: ${countMetCriteria(contract.doneWhen)}/${contract.doneWhen.length} met`,
  ];

  for (const criterion of contract.doneWhen) {
    lines.push(`    - [${criterion.met ? "x" : " "}] ${criterion.id} (${criterion.kind}) ${criterion.text}`);
  }
  return lines;
}

function formatContractList(contracts: readonly Contract[]): string[] {
  if (contracts.length === 0) {
    return ["No contracts found"];
  }

  const lines = [`${contracts.length} contract(s)`, ""];
  for (const contract of contracts) {
    lines.push(`${contract.id}  ${contract.status.padEnd(10)}  ${contract.taskId}`);
  }
  return lines;
}

function resolveContractSilent(opts: { silent?: unknown }): boolean {
  return opts.silent === true || process.env.MAESTRO_TASK_SILENT === "1";
}

function emitContractSilentSuccess(
  isJson: boolean,
  opts: { silent?: unknown },
  contract: Contract,
): boolean {
  if (isJson || !resolveContractSilent(opts)) return false;
  console.log(`${contract.id} [ok]`);
  return true;
}

async function resolveRepoRoot(cwd: string): Promise<string> {
  const result = await execArgv(["git", "rev-parse", "--show-toplevel"], { cwd });
  return result.exitCode === 0 && result.stdout ? result.stdout : cwd;
}

async function resolveHeadCommit(cwd: string): Promise<string | undefined> {
  const result = await execArgv(["git", "rev-parse", "HEAD"], { cwd });
  if (result.exitCode === 0 && result.stdout) {
    return result.stdout;
  }

  const repoCheck = await execArgv(["git", "rev-parse", "--is-inside-work-tree"], { cwd });
  if (repoCheck.exitCode === 0 && repoCheck.stdout === "true") {
    return "4b825dc642cb6eb9a060e54bf8d69288fbee4904";
  }

  return undefined;
}

async function resolveContractActor(ref: string): Promise<string> {
  const services = getServices();
  const task = await services.taskStore.get(ref);
  if (task) {
    return task.assignee ?? "user";
  }

  const byTask = await services.contractStore.getByTaskId(ref);
  if (byTask) {
    return byTask.lockedBy ?? byTask.createdBy ?? "user";
  }

  const contract = await services.contractStore.get(ref);
  return contract?.lockedBy ?? contract?.createdBy ?? "user";
}
