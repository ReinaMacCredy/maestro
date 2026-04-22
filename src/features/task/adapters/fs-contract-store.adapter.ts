import { join } from "node:path";
import { readdir } from "node:fs/promises";
import { MAESTRO_DIR } from "@/shared/domain/defaults.js";
import { MaestroError } from "@/shared/errors.js";
import { appendText, ensureDir, readText, removeIfExists, writeJson } from "@/shared/lib/fs.js";
import { withFileLock } from "@/shared/lib/fs-lock.js";
import { assertSafeSegment, resolveWithin } from "@/shared/lib/path-safety.js";
import {
  CONTRACT_ID_PATTERN,
  buildActiveOverlapError,
  generateContractId,
  isActiveContract,
  lastContractIndexedAt,
  normalizeStoredContractRepoRoot,
  validateContract,
  validateContractIndexEntry,
} from "../domain/contract/contract-state.js";
import type {
  Contract,
  ContractIndexEntry,
  CreateContractRecordInput,
  DeleteContractRecordInput,
} from "../domain/contract/contract-types.js";
import { CONTRACT_SCHEMA_VERSION } from "../domain/contract/contract-types.js";
import type { ContractStorePort } from "../ports/contract-store.port.js";

const LOCK_WAIT_TIMEOUT_MS = 5_000;
const LOCK_INITIAL_RETRY_DELAY_MS = 10;
const LOCK_MAX_RETRY_DELAY_MS = 100;
const LOCK_STALE_MS = 30_000;

export interface FsContractStoreAdapterOptions {
  readonly generateId?: () => string;
}

export class FsContractStoreAdapter implements ContractStorePort {
  private readonly generateId: () => string;

  constructor(
    private readonly baseDir: string,
    options: FsContractStoreAdapterOptions = {},
  ) {
    this.generateId = options.generateId ?? generateContractId;
  }

  async all(): Promise<readonly Contract[]> {
    const contractsDir = this.contractsDir();
    const entries = await this.readContractFilenames(contractsDir);
    const contracts: Contract[] = [];
    for (const filename of entries) {
      const loaded = await this.readContractFile(join(contractsDir, filename));
      contracts.push(loaded);
    }
    contracts.sort((left, right) => left.createdAt.localeCompare(right.createdAt));
    return contracts;
  }

  async get(id: string): Promise<Contract | undefined> {
    if (!CONTRACT_ID_PATTERN.test(id)) return undefined;
    const path = this.contractPath(id);
    const raw = await readText(path);
    if (raw === undefined) return undefined;
    return this.parseContract(raw, path);
  }

  async getByTaskId(taskId: string): Promise<Contract | undefined> {
    const entry = this.findLatestTaskIndexEntry(taskId, await this.readIndex());
    if (!entry) {
      return undefined;
    }
    return this.get(entry.id);
  }

  async readIndex(): Promise<readonly ContractIndexEntry[]> {
    const raw = await readText(this.indexPath());
    if (raw === undefined) return [];

    const entries: ContractIndexEntry[] = [];
    const lines = raw.split("\n");
    for (const [index, line] of lines.entries()) {
      const trimmed = line.trim();
      if (trimmed.length === 0) continue;
      let parsed: unknown;
      try {
        parsed = JSON.parse(trimmed);
      } catch {
        throw new MaestroError(`Contract index is corrupted at line ${index + 1}: ${this.indexPath()}`, [
          "Repair or remove the malformed JSON line before retrying",
        ]);
      }
      const validated = validateContractIndexEntry(parsed);
      if (!validated) {
        throw new MaestroError(`Contract index contains an invalid record at line ${index + 1}: ${this.indexPath()}`, [
          "Repair the invalid contract index JSON before retrying",
        ]);
      }
      entries.push(validated);
    }
    return entries;
  }

  async create(input: CreateContractRecordInput): Promise<Contract> {
    return this.withLock(async () => {
      const index = await this.readIndex();
      const existingByTaskEntry = this.findLatestTaskIndexEntry(input.taskId, index);
      const existingByTask = existingByTaskEntry
        ? await this.get(existingByTaskEntry.id)
        : undefined;
      if (existingByTask && existingByTask.status !== "discarded") {
        throw new MaestroError(`Task ${input.taskId} already has a contract: ${existingByTask.id}`, [
          "Edit or discard the existing contract before creating another one",
        ]);
      }

      const knownIds = new Set(index.map((entry) => entry.id));
      let id = input.id ?? this.generateId();
      if (knownIds.has(id)) {
        if (input.id !== undefined) {
          throw new MaestroError(`Contract id already exists: ${id}`, [
            "Retry without an explicit id so Maestro can generate a fresh one",
          ]);
        }
        id = this.generateId();
        if (knownIds.has(id)) {
          throw new MaestroError(`Failed to generate a unique contract id after retrying once`, [
            "Retry the command to generate a fresh contract id",
          ]);
        }
      }

      const contract = validateContract({
        schemaVersion: CONTRACT_SCHEMA_VERSION,
        id,
        taskId: input.taskId,
        repoRoot: normalizeStoredContractRepoRoot(input.repoRoot),
        status: "draft",
        createdAt: input.createdAt,
        intent: input.intent,
        scope: input.scope,
        doneWhen: input.doneWhen,
        amendments: [],
        createdBy: input.createdBy,
        configSnapshot: input.configSnapshot,
      });
      if (!contract) {
        throw new MaestroError(`Refusing to persist an invalid contract draft for task ${input.taskId}`, [
          "Repair the create-contract input before retrying",
        ]);
      }

      await this.writeContractFile(contract);
      await this.appendIndex({
        id: contract.id,
        taskId: contract.taskId,
        status: contract.status,
        at: contract.createdAt,
      });
      return contract;
    });
  }

  async save(contract: Contract): Promise<Contract> {
    return this.withLock(async () => {
      const validated = validateContract({
        ...contract,
        repoRoot: normalizeStoredContractRepoRoot(contract.repoRoot),
      });
      if (!validated) {
        throw new MaestroError(`Refusing to persist invalid contract ${contract.id}`, [
          "Repair the contract object before saving it",
        ]);
      }
      await this.assertActiveOverlapInvariant(validated);
      await this.writeContractFile(validated);
      await this.appendIndex({
        id: validated.id,
        taskId: validated.taskId,
        status: validated.status,
        at: lastContractIndexedAt(validated),
      });
      return validated;
    });
  }

  async delete(id: string, input: DeleteContractRecordInput): Promise<boolean> {
    assertSafeSegment(id, "contract id", CONTRACT_ID_PATTERN, "'c-' followed by 6 hex characters");
    return this.withLock(async () => {
      const removed = await removeIfExists(this.contractPath(id));
      await this.appendIndex({
        id,
        taskId: input.taskId,
        status: input.status ?? "discarded",
        at: input.at,
        ...(input.reason ? { reason: input.reason } : {}),
      });
      return removed;
    });
  }

  private contractsDir(): string {
    return join(this.baseDir, MAESTRO_DIR, "tasks", "contracts");
  }

  private contractPath(id: string): string {
    return resolveWithin(this.contractsDir(), `${id}.json`, "Contract storage path");
  }

  private indexPath(): string {
    return join(this.contractsDir(), "index.jsonl");
  }

  private lockPath(): string {
    return join(this.contractsDir(), ".contracts.lock");
  }

  private async writeContractFile(contract: Contract): Promise<void> {
    await ensureDir(this.contractsDir());
    await writeJson(this.contractPath(contract.id), contract);
  }

  private async appendIndex(entry: ContractIndexEntry): Promise<void> {
    await ensureDir(this.contractsDir());
    await appendText(this.indexPath(), `${JSON.stringify(entry)}\n`);
  }

  private findLatestTaskIndexEntry(
    taskId: string,
    index: readonly ContractIndexEntry[],
  ): ContractIndexEntry | undefined {
    for (let i = index.length - 1; i >= 0; i -= 1) {
      const entry = index[i];
      if (entry?.taskId === taskId) {
        return entry;
      }
    }
    return undefined;
  }

  private async listActiveIndexedContracts(): Promise<readonly Contract[]> {
    const latestById = new Map<string, ContractIndexEntry>();
    for (const entry of await this.readIndex()) {
      latestById.set(entry.id, entry);
    }

    const candidateIds: string[] = [];
    for (const entry of latestById.values()) {
      if (entry.status === "locked" || entry.status === "amended") {
        candidateIds.push(entry.id);
      }
    }

    const loaded = await Promise.all(candidateIds.map((id) => this.get(id)));
    return loaded.filter((contract): contract is Contract => contract !== undefined && isActiveContract(contract));
  }

  private async assertActiveOverlapInvariant(contract: Contract): Promise<void> {
    if (!isActiveContract(contract) || contract.configSnapshot.overlapPolicy !== "fail") {
      return;
    }

    const overlapping = (await this.listActiveIndexedContracts()).filter((candidate) => candidate.id !== contract.id);
    if (overlapping.length === 0) {
      return;
    }

    throw buildActiveOverlapError(contract.id, overlapping.map((item) => item.id));
  }

  private async readContractFilenames(dir: string): Promise<readonly string[]> {
    try {
      const entries = await readdir(dir, { withFileTypes: true });
      return entries
        .filter((entry) => entry.isFile() && CONTRACT_ID_PATTERN.test(entry.name.replace(/\.json$/, "")) && entry.name.endsWith(".json"))
        .map((entry) => entry.name)
        .sort();
    } catch (error) {
      const errno = error as NodeJS.ErrnoException;
      if (errno.code === "ENOENT") return [];
      throw error;
    }
  }

  private async readContractFile(path: string): Promise<Contract> {
    const raw = await readText(path);
    if (raw === undefined) {
      throw new MaestroError(`Contract file disappeared while reading: ${path}`);
    }
    return this.parseContract(raw, path);
  }

  private parseContract(raw: string, path: string): Contract {
    let parsed: unknown;
    try {
      parsed = JSON.parse(raw);
    } catch {
      throw new MaestroError(`Contract storage is corrupted: ${path}`, [
        "Repair or remove the malformed contract JSON before retrying",
      ]);
    }
    const validated = validateContract(parsed);
    if (!validated) {
      throw new MaestroError(`Contract storage contains an invalid record: ${path}`, [
        "Repair the contract JSON before retrying",
        "Invalid hand-edits are rejected to avoid rewriting broken contract state",
      ]);
    }
    return validated;
  }

  private async withLock<T>(fn: () => Promise<T>): Promise<T> {
    await ensureDir(this.contractsDir());
    const lockPath = this.lockPath();
    return withFileLock(
      {
        lockPath,
        staleMs: LOCK_STALE_MS,
        timeoutMs: LOCK_WAIT_TIMEOUT_MS,
        initialRetryDelayMs: LOCK_INITIAL_RETRY_DELAY_MS,
        maxRetryDelayMs: LOCK_MAX_RETRY_DELAY_MS,
        timeoutMessage: `Contract store lock is still active: ${lockPath}`,
        timeoutHints: [
          "Retry once the other contract command finishes",
          `If this lock is stale, remove it manually: rm ${lockPath}`,
        ],
      },
      fn,
    );
  }
}
