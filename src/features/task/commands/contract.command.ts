import { userInfo } from "node:os";
import { Command } from "commander";
import { fstatSync } from "node:fs";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { getServices } from "@/services.js";
import type { MaestroConfig } from "@/infra/domain/config-types.js";
import { MaestroError } from "@/shared/errors.js";
import { fileExists, readTextOrStdin, writeText } from "@/shared/lib/fs.js";
import { output, resolveJsonFlag, warn } from "@/shared/lib/output.js";
import { normalizeSlashes } from "@/shared/lib/path-normalize.js";
import { resolveMaestroProjectRoot } from "@/shared/lib/project-root.js";
import { parseYaml, stringifyYaml } from "@/shared/lib/yaml.js";
import {
  DONE_WHEN_ID_PATTERN,
  countMetCriteria,
  isActiveContract,
} from "../domain/contract/contract-state.js";
import type {
  Contract,
  ContractConfigSnapshot,
  ContractScope,
  ContractStatus,
  ContractVerdict,
  DoneWhenCriterion,
} from "../domain/contract/contract-types.js";
import { amendContract } from "../usecases/contract/amend-contract.usecase.js";
import { computeContractVerdictForTask } from "../usecases/contract/compute-verdict.usecase.js";
import { editContract } from "../usecases/contract/edit-contract.usecase.js";
import {
  addContractCriterion,
  markContractCriterion,
  removeContractCriterion,
} from "../usecases/contract/criteria.usecase.js";
import { createContract } from "../usecases/contract/create-contract.usecase.js";
import { discardContract } from "../usecases/contract/discard-contract.usecase.js";
import { listContracts } from "../usecases/contract/list-contracts.usecase.js";
import { lockContract } from "../usecases/contract/lock-contract.usecase.js";
import { showContract } from "../usecases/contract/show-contract.usecase.js";
import { reopenTaskFlow } from "../usecases/reopen-task-flow.usecase.js";
import { buildTaskOwnerId } from "../usecases/task-continuation.usecase.js";
import { resolveTaskSilentMode } from "./command-silence.js";

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

interface ContractVerdictPreview {
  readonly contractId: string;
  readonly taskId: string;
  readonly contractStatus: ContractStatus;
  readonly closedAtCommit?: string;
  readonly verdict: ContractVerdict;
  readonly criteria: readonly DoneWhenCriterion[];
}

export function registerContractCommand(taskCmd: Command, program: Command): void {
  const contractCmd = taskCmd
    .command("contract")
    .description("Task contract draft, lock, and inspection commands");

  contractCmd
    .command("new <taskId>")
    .description("Create a draft contract for a task")
    .option("--from <path>", "Load YAML from a file or named template ('-' for stdin)")
    .option("--editor <cmd>", "Open an editor command to write the draft YAML")
    .option("--session <id>", "Use an explicit session id instead of auto-detection")
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
        repoRoot: await services.gitAnchor.resolveRepoRoot(cwd),
        intent: readTemplateIntent(template),
        scope: readTemplateScope(template),
        doneWhen: readTemplateDoneWhen(template),
        createdBy: await resolveDraftContractActor(taskId, opts.session),
        configSnapshot: buildContractConfigSnapshot(config),
      });
      await refreshContractNowMd();

      if (emitContractSilentSuccess(isJson, opts, contract)) return;
      output(isJson, contract, formatContractDetail);
    });

  contractCmd
    .command("lock <ref>")
    .description("Lock a draft contract so completion can diff against it")
    .option("--session <id>", "Use an explicit session id instead of auto-detection")
    .option("--silent", "Print only '<id> [ok]' (for scripts)")
    .option("--json", "Output as JSON")
    .action(async (ref: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const config = await services.config.load(process.cwd());
      const contract = await lockContract(services.contractStore, {
        ref,
        actorId: await resolveDraftContractActor(ref, opts.session),
        claimedAtCommit: await services.gitAnchor.resolveHeadCommit(process.cwd()),
        configSnapshot: buildContractConfigSnapshot(config),
      });
      await refreshContractNowMd();
      warnScopeOverlap(contract, opts);

      if (emitContractSilentSuccess(isJson, opts, contract)) return;
      output(isJson, contract, formatContractDetail);
    });

  contractCmd
    .command("edit <ref>")
    .description("Edit a draft contract before lock")
    .option("--from <path>", "Load YAML from a file or named template ('-' for stdin)")
    .option("--editor <cmd>", "Open an editor command to update the draft YAML")
    .option("--session <id>", "Use an explicit session id instead of auto-detection")
    .option("--silent", "Print only '<id> [ok]' (for scripts)")
    .option("--json", "Output as JSON")
    .action(async (ref: string, opts) => {
      await resolveDraftContractActor(ref, opts.session);
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const contract = await showContract(services.contractStore, ref);
      const template = await loadContractDraftTemplate(opts.from, opts.editor, renderEditableContract(contract));
      const edited = await editContract(services.contractStore, {
        ref,
        intent: readTemplateIntent(template),
        scope: readTemplateScope(template),
        doneWhen: readTemplateDoneWhen(template),
      });
      await refreshContractNowMd();

      if (emitContractSilentSuccess(isJson, opts, edited)) return;
      output(isJson, edited, formatContractDetail);
    });

  contractCmd
    .command("amend <ref>")
    .description("Amend a locked contract and record why it changed")
    .requiredOption("--reason <text>", "Why the contract changed")
    .option("--from <path>", "Load YAML from a file or named template ('-' for stdin)")
    .option("--editor <cmd>", "Open an editor command to update the draft YAML")
    .option("--session <id>", "Use an explicit session id instead of auto-detection")
    .option("--silent", "Print only '<id> [ok]' (for scripts)")
    .option("--json", "Output as JSON")
    .action(async (ref: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const contract = await showContract(services.contractStore, ref);
      const template = await loadContractDraftTemplate(opts.from, opts.editor, renderEditableContract(contract));
      const amended = await amendContract(services.contractStore, {
        ref,
        actorId: await resolveActiveContractActor(ref, opts.session),
        reason: opts.reason,
        intent: readTemplateIntent(template),
        scope: readTemplateScope(template),
        doneWhen: readTemplateDoneWhen(template),
      });
      await refreshContractNowMd();

      if (emitContractSilentSuccess(isJson, opts, amended)) return;
      output(isJson, amended, formatContractDetail);
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
    .command("verdict <ref>")
    .description("Preview the current verdict without closing the task")
    .option("--json", "Output as JSON")
    .action(async (ref: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const contract = await showContract(services.contractStore, ref);
      assertContractCanPreviewVerdict(contract);

      const task = await services.taskStore.get(contract.taskId);
      if (!task) {
        throw new MaestroError(`Task ${contract.taskId} linked to contract ${contract.id} was not found`, [
          "Inspect .maestro/tasks/tasks.jsonl for stale or corrupted state",
        ]);
      }

      const preview = await computeContractVerdictForTask(
        services.contractStore,
        services.gitAnchor,
        contract,
        task,
        undefined,
        await services.gitAnchor.resolveRepoRoot(process.cwd()),
      );

      output(isJson, {
        contractId: contract.id,
        taskId: contract.taskId,
        contractStatus: contract.status,
        closedAtCommit: preview.closedAtCommit,
        verdict: preview.verdict,
        criteria: preview.criteria,
      } satisfies ContractVerdictPreview, formatContractVerdictPreview);
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
    .option("--session <id>", "Use an explicit session id instead of auto-detection")
    .option("--silent", "Print only '<id> [ok]' (for scripts)")
    .option("--json", "Output as JSON")
    .action(async (ref: string, opts) => {
      await resolveDraftContractActor(ref, opts.session);
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const contract = await discardContract(services.taskStore, services.contractStore, ref);
      await refreshContractNowMd();

      if (emitContractSilentSuccess(isJson, opts, contract)) return;
      output(isJson, contract, formatContractDetail);
    });

  contractCmd
    .command("reopen <ref>")
    .description("Reopen the completed task linked to a contract and reactivate the contract")
    .option("--silent", "Print only '<id> [ok]' (for scripts)")
    .option("--json", "Output as JSON")
    .action(async (ref: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const contract = await showContract(services.contractStore, ref);
      const reopened = await reopenTaskFlow({
        taskStore: services.taskStore,
        continuationStore: services.taskContinuationStore,
        continuationHistory: services.taskContinuationHistory,
        contractStore: services.contractStore,
      }, contract.taskId);
      await refreshContractNowMd();

      const payload = reopened.contract ?? await showContract(services.contractStore, contract.id);
      if (emitContractSilentSuccess(isJson, opts, payload)) return;
      output(isJson, payload, formatContractDetail);
    });

  const criteriaCmd = contractCmd
    .command("criteria")
    .description("Manage contract done-when criteria");

  criteriaCmd
    .command("mark <ref> <criterionId>")
    .description("Mark a criterion met or unmet")
    .option("--met", "Mark the criterion met (default)")
    .option("--unmet", "Mark the criterion unmet and clear evidence")
    .option("--evidence <text>", "Attach met evidence")
    .option("--session <id>", "Use an explicit session id instead of auto-detection")
    .option("--silent", "Print only '<id> [ok]' (for scripts)")
    .option("--json", "Output as JSON")
    .action(async (ref: string, criterionId: string, opts) => {
      if (opts.met === true && opts.unmet === true) {
        throw new MaestroError("Choose either --met or --unmet, not both");
      }

      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const contract = await markContractCriterion(services.contractStore, {
        ref,
        criterionId,
        actorId: await resolveActiveContractActor(ref, opts.session),
        met: opts.unmet === true ? false : true,
        evidence: opts.evidence,
      });
      await refreshContractNowMd();

      if (emitContractSilentSuccess(isJson, opts, contract)) return;
      output(isJson, contract, formatContractDetail);
    });

  criteriaCmd
    .command("add <ref> <text>")
    .description("Add a manual criterion to a locked contract")
    .option("--session <id>", "Use an explicit session id instead of auto-detection")
    .option("--silent", "Print only '<id> [ok]' (for scripts)")
    .option("--json", "Output as JSON")
    .action(async (ref: string, text: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const contract = await addContractCriterion(services.contractStore, {
        ref,
        text,
        actorId: await resolveActiveContractActor(ref, opts.session),
      });
      await refreshContractNowMd();

      if (emitContractSilentSuccess(isJson, opts, contract)) return;
      output(isJson, contract, formatContractDetail);
    });

  criteriaCmd
    .command("remove <ref> <criterionId>")
    .description("Remove one criterion from a locked contract")
    .option("--session <id>", "Use an explicit session id instead of auto-detection")
    .option("--silent", "Print only '<id> [ok]' (for scripts)")
    .option("--json", "Output as JSON")
    .action(async (ref: string, criterionId: string, opts) => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const contract = await removeContractCriterion(services.contractStore, {
        ref,
        criterionId,
        actorId: await resolveActiveContractActor(ref, opts.session),
      });
      await refreshContractNowMd();

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
  initialContent = defaultContractTemplate(),
): Promise<ContractDraftTemplate> {
  const envEditor = process.env.EDITOR ?? process.env.VISUAL;
  const autoDetectedStdin = fromPath === undefined
    && editorCommand === undefined
    && hasRealStdinPayload();
  // Some runners hand the child an empty pipe/file on fd0. If we auto-read
  // that and skip the editor, edit/amend silently collapse to an empty draft.
  const autoDetectedDraft = autoDetectedStdin
    ? await readDraftSource("-")
    : undefined;
  // Auto-detect real piped/redirected stdin when the caller passed neither
  // --from nor --editor. Lets `cat contract.yaml | maestro task contract new`
  // and `maestro task contract new <id> < contract.yaml` work without spelling
  // `--from -`. Keep non-empty stdin ahead of an ambient editor, but let the
  // editor win when the inherited stdin is just an empty placeholder.
  const resolvedFromPath = fromPath
    ?? (autoDetectedDraft !== undefined && autoDetectedDraft.trim().length > 0 ? "-" : undefined);

  if (!resolvedFromPath && !editorCommand && !envEditor) {
    throw new MaestroError("Provide --from <path>, pipe YAML on stdin, or pass --editor <cmd>", [
      "Example: maestro task contract new <id> --from contract.yaml",
      "Example: cat contract.yaml | maestro task contract new <id>",
      "Or set $EDITOR and rerun without --from",
    ]);
  }

  const resolvedEditor = editorCommand
    ?? (resolvedFromPath ? undefined : envEditor);
  const baseContent = resolvedFromPath
    ? (resolvedFromPath === "-" && autoDetectedDraft !== undefined ? autoDetectedDraft : await readDraftSource(resolvedFromPath))
    : initialContent;
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

function hasRealStdinPayload(): boolean {
  try {
    const stat = fstatSync(0);
    return stat.isFIFO() || stat.isFile();
  } catch {
    return false;
  }
}

async function readDraftSource(path: string): Promise<string> {
  const raw = await readTextOrStdin(path);
  if (raw !== undefined) {
    return raw;
  }

  const namedTemplate = await resolveNamedContractTemplate(path);
  if (namedTemplate) {
    return await Bun.file(namedTemplate).text();
  }

  throw new MaestroError(`Contract template not found: ${path}`, [
    "Check the file path and retry",
    "Or add a reusable draft under .maestro/tasks/contract-templates/",
    "Use '-' to read YAML from stdin",
  ]);
}

async function resolveNamedContractTemplate(path: string): Promise<string | undefined> {
  if (
    path === "-"
    || path.trim().length === 0
    || path.includes("/")
    || path.includes("\\")
  ) {
    return undefined;
  }

  const templateDir = join(
    resolveMaestroProjectRoot(process.cwd()),
    ".maestro",
    "tasks",
    "contract-templates",
  );
  for (const suffix of ["", ".md", ".yaml", ".yml"] as const) {
    const candidate = join(templateDir, `${path}${suffix}`);
    if (await fileExists(candidate)) {
      return candidate;
    }
  }

  return undefined;
}

async function editContractDraft(initialContent: string, editorCommand: string): Promise<string> {
  const draftDir = await mkdtemp(join(tmpdir(), "maestro-contract-draft-"));
  const draftPath = join(draftDir, "contract.yaml");
  await writeText(draftPath, initialContent);

  try {
    const editorArgv = parseEditorCommand(editorCommand);
    const result = Bun.spawnSync([...editorArgv, draftPath], {
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

function parseEditorCommand(command: string): string[] {
  const argv: string[] = [];
  let current = "";
  let quote: "\"" | "'" | undefined;
  let escaped = false;

  for (const char of command) {
    if (escaped) {
      current += char;
      escaped = false;
      continue;
    }

    if (char === "\\" && quote !== "'") {
      escaped = true;
      continue;
    }

    if (quote) {
      if (char === quote) {
        quote = undefined;
      } else {
        current += char;
      }
      continue;
    }

    if (char === "\"" || char === "'") {
      quote = char;
      continue;
    }

    if (/\s/.test(char)) {
      if (current.length > 0) {
        argv.push(current);
        current = "";
      }
      continue;
    }

    current += char;
  }

  if (escaped || quote) {
    throw new MaestroError(`Editor command is malformed: ${command}`, [
      "Close any open quotes or trailing escapes and retry",
    ]);
  }
  if (current.length > 0) {
    argv.push(current);
  }
  if (argv.length === 0) {
    throw new MaestroError("Editor command is empty", [
      "Pass --editor '<cmd>' or set $EDITOR to a real executable",
    ]);
  }

  return argv;
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
): readonly Array<{ readonly id?: string; readonly text: string; readonly kind?: DoneWhenCriterion["kind"] }> {
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
    const id = (entry as { id?: unknown }).id;
    if (typeof text !== "string") {
      throw new MaestroError(`Invalid contract draft: doneWhen[${index}].text must be a string`, [
        "Each doneWhen item needs human-readable text",
      ]);
    }
    if (id !== undefined && (typeof id !== "string" || !DONE_WHEN_ID_PATTERN.test(id))) {
      throw new MaestroError(`Invalid contract draft: doneWhen[${index}].id must look like dw-xxxxxx`);
    }
    if (kind !== undefined && kind !== "manual" && kind !== "receipt-hint") {
      throw new MaestroError(`Invalid contract draft: doneWhen[${index}].kind must be manual or receipt-hint`);
    }

    return {
      ...(id ? { id } : {}),
      text,
      ...(kind !== undefined ? { kind } : {}),
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
    "  Repo root: current workspace",
    `  Created: ${contract.createdAt}`,
    ...(contract.lockedAt ? [`  Locked at: ${contract.lockedAt}`] : []),
    ...(contract.lockedBy ? [`  Locked by: ${contract.lockedBy}`] : []),
    ...(contract.claimedAtCommit ? [`  Claimed at commit: ${contract.claimedAtCommit}`] : []),
    ...(contract.closedAt ? [`  Closed at: ${contract.closedAt}`] : []),
    ...(contract.closedBy ? [`  Closed by: ${contract.closedBy}`] : []),
    ...(contract.discardedAt ? [`  Discarded at: ${contract.discardedAt}`] : []),
    `  Scope expected: ${contract.scope.filesExpected.join(", ") || "(none)"}`,
    `  Scope forbidden: ${contract.scope.filesForbidden.join(", ") || "(none)"}`,
    `  Done when: ${countMetCriteria(contract.doneWhen)}/${contract.doneWhen.length} met`,
    `  Amendments: ${contract.amendments.length}`,
    ...(contract.ownershipHistory && contract.ownershipHistory.length > 0
      ? [`  Ownership transfers: ${contract.ownershipHistory.length}`]
      : []),
  ];

  for (const criterion of contract.doneWhen) {
    lines.push(
      `    - [${criterion.met ? "x" : " "}] ${criterion.id} (${criterion.kind}) ${criterion.text}`
      + (criterion.metEvidence ? ` [${criterion.metEvidence}]` : ""),
    );
  }
  if (contract.amendments.length > 0) {
    lines.push("  Amendment log:");
    for (const amendment of contract.amendments) {
      lines.push(`    - ${amendment.id} ${amendment.at} ${amendment.by}: ${amendment.reason}`);
    }
  }
  if (contract.ownershipHistory && contract.ownershipHistory.length > 0) {
    lines.push("  Ownership log:");
    for (const transfer of contract.ownershipHistory) {
      lines.push(`    - ${transfer.at} ${transfer.from} -> ${transfer.to} (${transfer.reason})`);
    }
  }
  if (contract.verdict) {
    lines.push(`  Verdict: ${contract.verdict.fulfilled ? "fulfilled" : "broken"}`);
    lines.push(`  Files touched: ${contract.verdict.actualFilesTouched.join(", ") || "(none)"}`);
    if (contract.verdict.actualFilesTouchedTruncated) {
      lines.push(
        `  Files touched stored: ${contract.verdict.actualFilesTouchedTruncated.stored}/${contract.verdict.actualFilesTouchedTruncated.actual}`,
      );
    }
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

function formatContractVerdictPreview(preview: ContractVerdictPreview): string[] {
  const lines = [
    `Contract verdict preview: ${preview.contractId}`,
    `  Task: ${preview.taskId}`,
    `  Status: ${preview.contractStatus}`,
    `  Result: ${preview.verdict.fulfilled ? "fulfilled" : "broken"}`,
    ...(preview.closedAtCommit ? [`  Closed at commit: ${preview.closedAtCommit}`] : []),
    `  Done when: ${countMetCriteria(preview.criteria)}/${preview.criteria.length} met`,
    `  Files touched: ${preview.verdict.actualFilesTouched.join(", ") || "(none)"}`,
  ];

  if (preview.verdict.actualFilesTouchedTruncated) {
    lines.push(
      `  Files touched stored: ${preview.verdict.actualFilesTouchedTruncated.stored}/${preview.verdict.actualFilesTouchedTruncated.actual}`,
    );
  }

  if (preview.verdict.outOfScopeFiles.length > 0) {
    lines.push(`  Out of scope: ${preview.verdict.outOfScopeFiles.join(", ")}`);
  }
  if (preview.verdict.forbiddenTouched.length > 0) {
    lines.push(`  Forbidden touched: ${preview.verdict.forbiddenTouched.join(", ")}`);
  }
  if (preview.verdict.unmetCriteria.length > 0) {
    lines.push(
      `  Unmet criteria: ${preview.verdict.unmetCriteria.map((criterion) => criterion.id).join(", ")}`,
    );
  }
  if (preview.verdict.overlapDetected) {
    lines.push(
      `  Overlap: ${preview.verdict.overlapDetected.otherContractIds.join(", ")} (${preview.verdict.overlapDetected.policy})`,
    );
  }
  if (preview.verdict.anchorFallback && preview.verdict.anchorFallback !== "direct") {
    lines.push(`  Anchor fallback: ${preview.verdict.anchorFallback}`);
  }
  if (preview.verdict.notes) {
    lines.push(`  Notes: ${preview.verdict.notes}`);
  }

  return lines;
}

function resolveContractSilent(opts: { silent?: unknown }): boolean {
  return resolveTaskSilentMode(opts);
}

function warnScopeOverlap(contract: Contract, opts: { silent?: unknown }): void {
  if (resolveContractSilent(opts)) {
    return;
  }
  const overlappingForbidden = findLikelyScopeOverlaps(contract.scope);
  if (overlappingForbidden.length === 0) {
    return;
  }
  warn(`Contract ${contract.id} filesForbidden overlaps filesExpected; forbidden wins for: ${overlappingForbidden.join(", ")}`);
}

function findLikelyScopeOverlaps(scope: ContractScope): readonly string[] {
  const overlapping = new Set<string>();
  for (const forbidden of scope.filesForbidden) {
    if (scope.filesExpected.some((expected) => patternsLikelyOverlap(expected, forbidden))) {
      overlapping.add(forbidden);
    }
  }
  return [...overlapping].sort();
}

function patternsLikelyOverlap(left: string, right: string): boolean {
  const normalizedLeft = normalizeSlashes(left.trim());
  const normalizedRight = normalizeSlashes(right.trim());
  if (!normalizedLeft || !normalizedRight) {
    return false;
  }
  if (normalizedLeft === normalizedRight) {
    return true;
  }

  const leftPrefix = staticGlobPrefix(normalizedLeft);
  const rightPrefix = staticGlobPrefix(normalizedRight);
  if (!leftPrefix || !rightPrefix) {
    return false;
  }
  return leftPrefix.startsWith(rightPrefix) || rightPrefix.startsWith(leftPrefix);
}

function staticGlobPrefix(pattern: string): string {
  const index = pattern.search(/[*?[{]/);
  const prefix = index === -1 ? pattern : pattern.slice(0, index);
  return prefix.replace(/[^/]*$/, "");
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
  if (contract) {
    const owner = await services.taskStore.get(contract.taskId);
    return owner?.assignee ?? contract.lockedBy ?? contract.createdBy;
  }
  return "user";
}

async function resolveDraftContractActor(
  ref: string,
  explicitSessionId: string | undefined,
): Promise<string> {
  const services = getServices();
  const task = await resolveContractTask(ref);
  const currentActorId = await resolveOptionalContractActorSessionId(explicitSessionId);

  if (!task?.assignee) {
    return currentActorId ?? "user";
  }
  if (!currentActorId) {
    throw new MaestroError(`Task ${task.id} is claimed by ${task.assignee}; contract draft changes require the owner session`, [
      `Retry from the owning session or pass '--session ${task.assignee}'`,
      "Use 'maestro task show <task-id>' to inspect task ownership",
    ]);
  }
  if (currentActorId !== task.assignee) {
    throw new MaestroError(`Task ${task.id} is claimed by ${task.assignee}; current session cannot modify its contract draft`, [
      `Retry from the owning session or pass '--session ${task.assignee}'`,
      "If the owner is dead, reclaim the task before changing its contract",
    ]);
  }

  return currentActorId;
}

async function resolveActiveContractActor(
  ref: string,
  explicitSessionId: string | undefined,
): Promise<string> {
  const services = getServices();
  const contract = await resolveContractRef(ref);
  if (!contract || !isActiveContract(contract)) {
    return resolveContractActor(ref);
  }

  const task = await services.taskStore.get(contract.taskId);
  const ownerId = task?.assignee ?? contract.lockedBy ?? contract.createdBy ?? "user";
  const currentActorId = await resolveOptionalContractActorSessionId(explicitSessionId);

  if (ownerId === "user") {
    return currentActorId ?? "user";
  }

  if (!currentActorId) {
    throw new MaestroError(`Contract ${contract.id} is owned by ${ownerId}; mutating it requires the owner session`, [
      `Retry from the owning session or pass '--session ${ownerId}'`,
      "If the owner is dead, reclaim the task before amending the contract",
    ]);
  }
  if (currentActorId !== ownerId) {
    throw new MaestroError(`Contract ${contract.id} is owned by ${ownerId}; current session cannot modify it`, [
      `Retry from the owning session or pass '--session ${ownerId}'`,
      "Use 'maestro task show <task-id>' to inspect task ownership",
    ]);
  }

  return currentActorId;
}

async function resolveOptionalContractActorSessionId(
  explicitSessionId: string | undefined,
): Promise<string | undefined> {
  if (explicitSessionId !== undefined) {
    const trimmed = explicitSessionId.trim();
    if (trimmed.length === 0) {
      throw new MaestroError("Invalid --session value", [
        "Pass a non-empty session id such as 'codex-1234' or 'operator-recovery'",
      ]);
    }
    return trimmed;
  }

  const services = getServices();
  const session = await services.sessionDetect.detect(process.cwd());
  if (session) {
    return buildTaskOwnerId(session.agent, session.sessionId);
  }
  // Synthesize the same per-user fallback as the task command so task ownership
  // established in one shell can be matched by contract-mutating commands in
  // another. Without this, the synthesized `local-<user>` assignee on a task
  // is rejected by contract new/edit/amend because this resolver returned
  // undefined for the caller.
  return buildTaskOwnerId("local", fallbackContractUserId());
}

function fallbackContractUserId(): string {
  const envUser = (process.env.USER ?? process.env.USERNAME ?? "").trim();
  if (envUser.length > 0) return envUser;
  try {
    return userInfo().username;
  } catch {
    return "default";
  }
}

async function resolveContractRef(ref: string): Promise<Contract | undefined> {
  const services = getServices();
  return await services.contractStore.get(ref) ?? await services.contractStore.getByTaskId(ref);
}

async function resolveContractTask(ref: string) {
  const services = getServices();
  const task = await services.taskStore.get(ref);
  if (task) {
    return task;
  }

  const contract = await resolveContractRef(ref);
  return contract ? await services.taskStore.get(contract.taskId) : undefined;
}

async function refreshContractNowMd(): Promise<void> {
  try {
    const services = getServices();
    await services.taskNowMdWriter.write(await services.taskStore.all());
  } catch {
    // NOW.md is derived output; never block a contract mutation on it.
  }
}

function assertContractCanPreviewVerdict(contract: Contract): asserts contract is Contract & {
  readonly status: "locked" | "amended";
} {
  if (isActiveContract(contract)) {
    return;
  }
  if (contract.status === "draft") {
    throw new MaestroError(`Contract ${contract.id} is still draft`, [
      `Lock it first: maestro task contract lock ${contract.id}`,
    ]);
  }
  if (contract.status === "discarded") {
    throw new MaestroError(`Contract ${contract.id} was discarded`, [
      `Show the discarded draft: maestro task contract show ${contract.id}`,
    ]);
  }
  throw new MaestroError(`Contract ${contract.id} already has a stored verdict`, [
    `Show it instead: maestro task contract show ${contract.id}`,
  ]);
}

function renderEditableContract(contract: Contract): string {
  return `${stringifyYaml({
    intent: contract.intent,
    scope: {
      filesExpected: contract.scope.filesExpected,
      filesForbidden: contract.scope.filesForbidden,
      ...(contract.scope.maxFilesTouched !== undefined
        ? { maxFilesTouched: contract.scope.maxFilesTouched }
        : {}),
    },
    doneWhen: contract.doneWhen.map((criterion) => ({
      id: criterion.id,
      text: criterion.text,
      kind: criterion.kind,
    })),
  }).trimEnd()}\n`;
}
