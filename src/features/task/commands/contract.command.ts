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
  AmendmentBudget,
  Contract,
  ContractConfigSnapshot,
  ContractScope,
  ContractStatus,
  ContractVerdict,
  CostBudget,
  DoneWhenCriterion,
} from "../domain/contract/contract-types.js";
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
  readonly amendmentBudget?: unknown;
  readonly costBudget?: unknown;
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
    .option("--allow-unknown-keys", "Warn instead of error on unknown contract draft keys")
    .option("--json", "Output as JSON")
    .action(async (taskId: string, opts): Promise<void> => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const cwd = process.cwd();
      const config = await services.config.load(resolveMaestroProjectRoot(cwd));
      const template = await loadContractDraftTemplate(opts.from, opts.editor, undefined, Boolean(opts.allowUnknownKeys));
      const amendmentBudget = readTemplateAmendmentBudget(template);
      const costBudget = readTemplateCostBudget(template);
      const contract = await services.contracts.draft({
        taskId,
        repoRoot: await services.gitAnchor.resolveRepoRoot(cwd),
        intent: readTemplateIntent(template),
        scope: readTemplateScope(template),
        doneWhen: readTemplateDoneWhen(template),
        createdBy: await resolveDraftContractActor(taskId, opts.session),
        configSnapshot: buildContractConfigSnapshot(config),
        ...(amendmentBudget ? { amendmentBudget } : {}),
        ...(costBudget ? { costBudget } : {}),
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
    .action(async (ref: string, opts): Promise<void> => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const config = await services.config.load(resolveMaestroProjectRoot(process.cwd()));
      const contract = await services.contracts.lock({
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
    .option("--allow-unknown-keys", "Warn instead of error on unknown contract draft keys")
    .option("--json", "Output as JSON")
    .action(async (ref: string, opts): Promise<void> => {
      await resolveDraftContractActor(ref, opts.session);
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const contract = await services.contracts.load(ref);
      const template = await loadContractDraftTemplate(opts.from, opts.editor, renderEditableContract(contract), Boolean(opts.allowUnknownKeys));
      const edited = await services.contracts.editDraft({
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
    .command("amend [ref]")
    .description("Amend a locked contract and record why it changed")
    .option("--reason <text>", "Why the contract changed")
    .option("--from <path>", "Load YAML from a file or named template ('-' for stdin)")
    .option("--editor <cmd>", "Open an editor command to update the draft YAML")
    .option("--session <id>", "Use an explicit session id instead of auto-detection")
    .option("--silent", "Print only '<id> [ok]' (for scripts)")
    .option("--allow-unknown-keys", "Warn instead of error on unknown contract draft keys")
    .option("--json", "Output as JSON")
    .option("--task <id>", "[L2 flag — see hint below]")
    .option("--add-path <path>", "[L2 flag — see hint below]")
    .option("--remove-path <path>", "[L2 flag — see hint below]")
    .action(async (ref: string | undefined, opts): Promise<void> => {
      if (opts.task !== undefined || opts.addPath !== undefined || opts.removePath !== undefined) {
        const taskId = opts.task ?? ref ?? "<task-id>";
        const flags: string[] = [];
        if (opts.addPath !== undefined) flags.push(`--add-path "${opts.addPath}"`);
        if (opts.removePath !== undefined) flags.push(`--remove-path "${opts.removePath}"`);
        if (opts.reason !== undefined) flags.push(`--reason "${opts.reason}"`);
        else flags.push(`--reason "<why>"`);
        throw new MaestroError(
          "Path-scoped amend uses the L2 contract verb, not 'task contract amend'",
          [
            `Run: maestro contract amend --task ${taskId} ${flags.join(" ")}`,
            "'task contract amend <ref>' is the L1 editor-based replace flow (uses --from / --editor on full YAML)",
            "'contract amend --task <id>' is the L2 path-scoped flow (uses --add-path / --remove-path)",
          ],
        );
      }
      if (ref === undefined) {
        throw new MaestroError("Missing required argument: <ref>", [
          "Usage: maestro task contract amend <task-id-or-contract-id> --reason '<why>' --from <yaml>",
          "For path-scoped amends use: maestro contract amend --task <id> --add-path <path> --reason '<why>'",
        ]);
      }
      if (opts.reason === undefined) {
        throw new MaestroError("Missing required option: --reason", [
          "Pass --reason '<why>' to record why the contract changed",
        ]);
      }
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const contract = await services.contracts.load(ref);
      const template = await loadContractDraftTemplate(opts.from, opts.editor, renderEditableContract(contract), Boolean(opts.allowUnknownKeys));
      const amended = await services.contracts.amend({
        kind: "replace",
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
    .action(async (ref: string, opts): Promise<void> => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const contract = await services.contracts.load(ref);

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
    .action(async (ref: string, opts): Promise<void> => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const contract = await services.contracts.load(ref);
      assertContractCanPreviewVerdict(contract);

      const task = await services.taskStore.get(contract.taskId);
      if (!task) {
        throw new MaestroError(`Task ${contract.taskId} linked to contract ${contract.id} was not found`, [
          "Inspect .maestro/tasks/tasks.jsonl for stale or corrupted state",
        ]);
      }

      const preview = await services.contracts.previewVerdict({
        contract,
        task,
        runtimeRepoRoot: await services.gitAnchor.resolveRepoRoot(process.cwd()),
      });

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
    .action(async (opts): Promise<void> => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const contracts = await services.contracts.list({
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
    .action(async (ref: string, opts): Promise<void> => {
      await resolveDraftContractActor(ref, opts.session);
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const contract = await services.contracts.discard(ref);
      await refreshContractNowMd();

      if (emitContractSilentSuccess(isJson, opts, contract)) return;
      output(isJson, contract, formatContractDetail);
    });

  contractCmd
    .command("reopen <ref>")
    .description("Reopen the completed task linked to a contract and reactivate the contract")
    .option("--silent", "Print only '<id> [ok]' (for scripts)")
    .option("--json", "Output as JSON")
    .action(async (ref: string, opts): Promise<void> => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const contract = await services.contracts.load(ref);
      const reopened = await reopenTaskFlow({
        taskStore: services.taskStore,
        continuationStore: services.taskContinuationStore,
        continuationHistory: services.taskContinuationHistory,
        contractStore: services.contractStore,
        contracts: services.contracts,
      }, contract.taskId);
      await refreshContractNowMd();

      const payload = reopened.contract ?? await services.contracts.load(contract.id);
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
    .action(async (ref: string, criterionId: string, opts): Promise<void> => {
      if (opts.met === true && opts.unmet === true) {
        throw new MaestroError("Choose either --met or --unmet, not both");
      }

      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const contract = await services.contracts.amend({
        kind: "markCriterion",
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
    .action(async (ref: string, text: string, opts): Promise<void> => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const contract = await services.contracts.amend({
        kind: "addCriterion",
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
    .action(async (ref: string, criterionId: string, opts): Promise<void> => {
      const services = getServices();
      const isJson = resolveJsonFlag(opts, program);
      const contract = await services.contracts.amend({
        kind: "removeCriterion",
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
  allowUnknownKeys = false,
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

  // If we'd fall back to $EDITOR but stdin is not a TTY, refuse instead of
  // launching an interactive editor that has nowhere to read keystrokes from.
  // Otherwise the verb appears to hang silently while the editor blocks on
  // stdin — fatal for autonomous agents and CI runs.
  if (!resolvedFromPath && !editorCommand && envEditor && !process.stdin.isTTY) {
    throw new MaestroError(
      "Cannot open $EDITOR in a non-interactive context (no TTY on stdin)",
      [
        "Pass --from <path> with the contract YAML, or pipe it on stdin",
        "Example: maestro task contract new <id> --from contract.yaml",
        "Example: cat contract.yaml | maestro task contract new <id>",
        "Use --editor <cmd> if you really mean to run a non-blocking editor command",
      ],
    );
  }

  const resolvedEditor = editorCommand
    ?? (resolvedFromPath ? undefined : envEditor);
  const baseContent = resolvedFromPath
    ? (resolvedFromPath === "-" && autoDetectedDraft !== undefined ? autoDetectedDraft : await readDraftSource(resolvedFromPath))
    : initialContent;
  const finalContent = resolvedEditor
    ? await editContractDraft(baseContent, resolvedEditor)
    : baseContent;

  if (finalContent.trim().length === 0) {
    throw new MaestroError("Contract draft is empty", [
      "Provide intent, scope, and doneWhen — see `maestro task contract new --help`",
      "An empty draft would silently wipe contract fields; refusing to proceed",
    ]);
  }

  let parsed: ContractDraftTemplate;
  try {
    parsed = parseYaml<ContractDraftTemplate>(finalContent) ?? {};
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    throw new MaestroError(`Cannot parse contract draft YAML: ${detail}`, [
      "Fix the YAML syntax in the template and retry",
    ]);
  }
  if (typeof parsed !== "object" || parsed === null || Object.keys(parsed).length === 0) {
    throw new MaestroError("Contract draft has no fields", [
      "Provide intent, scope, and doneWhen — see `maestro task contract new --help`",
      "An empty or fields-less draft would silently wipe contract state; refusing to proceed",
    ]);
  }
  if (allowUnknownKeys) {
    warnUnknownContractDraftKeys(parsed);
  } else {
    rejectUnknownContractDraftKeys(parsed);
  }
  return parsed;
}

const KNOWN_CONTRACT_DRAFT_KEYS = ["intent", "scope", "doneWhen", "amendmentBudget", "costBudget"] as const;
const KNOWN_CONTRACT_DRAFT_SCOPE_KEYS = [
  "filesExpected",
  "filesForbidden",
  "maxFilesTouched",
] as const;

// Refuse drafts with unknown keys — silently swallowing them produced
// half-initialized contracts (e.g. `expectedPaths` typo'd in for
// `filesExpected` saved a contract with empty scope and no error). Surfaced
// in R27. Aggregating both top-level and scope-level unknowns into one
// MaestroError keeps the user from playing whack-a-mole.
function rejectUnknownContractDraftKeys(template: ContractDraftTemplate): void {
  const messages: string[] = [];
  const hints: string[] = [];

  const topUnknown = Object.keys(template).filter(
    (k) => !(KNOWN_CONTRACT_DRAFT_KEYS as readonly string[]).includes(k),
  );
  for (const key of topUnknown) {
    messages.push(`Unknown contract draft key: '${key}'`);
  }
  if (topUnknown.length > 0) {
    hints.push(`Known top-level keys: ${KNOWN_CONTRACT_DRAFT_KEYS.join(", ")}`);
  }

  if (template.scope && typeof template.scope === "object") {
    const scopeUnknown = Object.keys(template.scope).filter(
      (k) => !(KNOWN_CONTRACT_DRAFT_SCOPE_KEYS as readonly string[]).includes(k),
    );
    for (const key of scopeUnknown) {
      const hint = key === "allowedPaths" || key === "expectedPaths"
        ? " (did you mean 'filesExpected'?)"
        : key === "forbiddenPaths"
          ? " (did you mean 'filesForbidden'?)"
          : "";
      messages.push(`Unknown contract draft key: 'scope.${key}'${hint}`);
    }
    if (scopeUnknown.length > 0) {
      hints.push(`Known scope keys: ${KNOWN_CONTRACT_DRAFT_SCOPE_KEYS.join(", ")}`);
    }
  }

  if (messages.length > 0) {
    throw new MaestroError(messages.join("; "), [
      ...hints,
      "Fix the YAML and re-run, or pass --allow-unknown-keys to keep the previous warn-and-ignore behavior",
    ]);
  }
}

// Suppress noise from the `silent` parameter — we still want stderr
// surfacing when a user explicitly passed --allow-unknown-keys but kept
// typo'd keys around.
function warnUnknownContractDraftKeys(template: ContractDraftTemplate): void {
  const topUnknown = Object.keys(template).filter(
    (k) => !(KNOWN_CONTRACT_DRAFT_KEYS as readonly string[]).includes(k),
  );
  for (const key of topUnknown) {
    warn(`Ignoring unknown contract draft key: '${key}'. Known keys: ${KNOWN_CONTRACT_DRAFT_KEYS.join(", ")}.`);
  }
  if (template.scope && typeof template.scope === "object") {
    const scopeUnknown = Object.keys(template.scope).filter(
      (k) => !(KNOWN_CONTRACT_DRAFT_SCOPE_KEYS as readonly string[]).includes(k),
    );
    for (const key of scopeUnknown) {
      const hint = key === "allowedPaths" || key === "expectedPaths"
        ? " (did you mean 'filesExpected'?)"
        : key === "forbiddenPaths"
          ? " (did you mean 'filesForbidden'?)"
          : "";
      warn(`Ignoring unknown contract draft key: 'scope.${key}'${hint}. Known scope keys: ${KNOWN_CONTRACT_DRAFT_SCOPE_KEYS.join(", ")}.`);
    }
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
  let raw: string | undefined;
  try {
    raw = await readTextOrStdin(path);
  } catch (err: unknown) {
    const code = (err as NodeJS.ErrnoException).code;
    if (code === "EISDIR") {
      throw new MaestroError(`Contract draft path is a directory: ${path}`, [
        "Pass a path to a YAML or JSON file, not a directory",
      ]);
    }
    const msg = err instanceof Error ? err.message : String(err);
    throw new MaestroError(`Cannot read contract draft: ${path}`, [msg]);
  }
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
): Array<{ readonly id?: string; readonly text: string; readonly kind?: DoneWhenCriterion["kind"] }> {
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
      throw new MaestroError(
        `Invalid contract draft: doneWhen[${index}].id must be 'dw-' followed by exactly 6 lowercase hex chars (0-9, a-f), e.g. dw-a1b2c3`,
        ["Or omit the id entirely — maestro will generate one for you."],
      );
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

function readTemplateAmendmentBudget(template: ContractDraftTemplate): AmendmentBudget | undefined {
  const raw = template.amendmentBudget;
  if (raw === undefined) return undefined;
  if (typeof raw !== "object" || raw === null || Array.isArray(raw)) {
    throw new MaestroError("Invalid contract draft: amendmentBudget must be an object", [
      "Use YAML like: amendmentBudget: { maxAmendments: 3 }",
    ]);
  }
  const known = new Set(["maxAmendments", "maxPathsPerAmendment", "forbiddenAmendmentPaths"]);
  for (const key of Object.keys(raw)) {
    if (!known.has(key)) {
      warn(
        `Ignoring unknown contract draft key: 'amendmentBudget.${key}'.`
        + ` Known keys: ${[...known].join(", ")}.`,
      );
    }
  }
  const obj = raw as Record<string, unknown>;
  const maxAmendments = obj.maxAmendments === undefined
    ? 3
    : readPositiveInteger(obj.maxAmendments, "amendmentBudget.maxAmendments");
  const maxPathsPerAmendment = obj.maxPathsPerAmendment === undefined
    ? 5
    : readPositiveInteger(obj.maxPathsPerAmendment, "amendmentBudget.maxPathsPerAmendment");
  const forbiddenAmendmentPaths = obj.forbiddenAmendmentPaths === undefined
    ? []
    : readStringList(obj.forbiddenAmendmentPaths, "amendmentBudget.forbiddenAmendmentPaths");
  return { maxAmendments, maxPathsPerAmendment, forbiddenAmendmentPaths };
}

function readTemplateCostBudget(template: ContractDraftTemplate): CostBudget | undefined {
  const raw = template.costBudget;
  if (raw === undefined) return undefined;
  if (typeof raw !== "object" || raw === null || Array.isArray(raw)) {
    throw new MaestroError("Invalid contract draft: costBudget must be an object", [
      "Use YAML like: costBudget: { maxRetries: 3, maxWallClockSeconds: 1800 }",
    ]);
  }
  const known = new Set(["maxRetries", "maxWallClockSeconds", "maxTokens"]);
  for (const key of Object.keys(raw)) {
    if (!known.has(key)) {
      warn(
        `Ignoring unknown contract draft key: 'costBudget.${key}'.`
        + ` Known keys: ${[...known].join(", ")}.`,
      );
    }
  }
  const obj = raw as Record<string, unknown>;
  if (obj.maxRetries === undefined) {
    throw new MaestroError("Invalid contract draft: costBudget.maxRetries is required", [
      "Set costBudget.maxRetries to a positive integer (e.g. 3)",
    ]);
  }
  if (obj.maxWallClockSeconds === undefined) {
    throw new MaestroError("Invalid contract draft: costBudget.maxWallClockSeconds is required", [
      "Set costBudget.maxWallClockSeconds to a positive integer (e.g. 1800)",
    ]);
  }
  const maxRetries = readPositiveInteger(obj.maxRetries, "costBudget.maxRetries");
  const maxWallClockSeconds = readPositiveInteger(obj.maxWallClockSeconds, "costBudget.maxWallClockSeconds");
  const maxTokens = obj.maxTokens === undefined
    ? undefined
    : readPositiveInteger(obj.maxTokens, "costBudget.maxTokens");
  return {
    maxRetries,
    maxWallClockSeconds,
    ...(maxTokens !== undefined ? { maxTokens } : {}),
  };
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
    lines.push(`  Verdict: ${formatVerdictResult(contract.verdict)}`);
    lines.push(`  Files touched: ${contract.verdict.actualFilesTouched.join(", ") || "(none)"}`);
    if (contract.verdict.actualFilesTouchedTruncated) {
      lines.push(
        `  Files touched stored: ${contract.verdict.actualFilesTouchedTruncated.stored}/${contract.verdict.actualFilesTouchedTruncated.actual}`,
      );
    }
  }
  return lines;
}

// "broken" alongside "Done when: 3/3 met" reads as nonsense — broken means
// scope/forbidden/cap violation. Append the structural reasons inline so the
// reader doesn't have to scan for the explanation. Surfaced in R27.
function formatVerdictResult(verdict: ContractVerdict): string {
  if (verdict.fulfilled) return "fulfilled";
  const reasons: string[] = [];
  if (verdict.outOfScopeFiles.length > 0) reasons.push(`out-of-scope files: ${verdict.outOfScopeFiles.length}`);
  if (verdict.forbiddenTouched.length > 0) reasons.push(`forbidden files: ${verdict.forbiddenTouched.length}`);
  if (verdict.unmetCriteria.length > 0) reasons.push(`unmet criteria: ${verdict.unmetCriteria.length}`);
  if (verdict.capExceeded) reasons.push(`files-touched cap exceeded: ${verdict.capExceeded.actual}/${verdict.capExceeded.cap}`);
  return reasons.length > 0 ? `broken — ${reasons.join(", ")}` : "broken";
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
    `  Result: ${formatVerdictResult(preview.verdict)}`,
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

async function resolveContractTask(ref: string): Promise<void> {
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
