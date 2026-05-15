import type { TaskQueryPort } from "@/shared/domain/legacy-task";
import { MaestroError } from "@/shared/errors.js";
import type { HandoffRecord, HandoffStorePort } from "../domain/handoff-types.js";
import { isOpenHandoffRecord } from "../domain/handoff-state.js";
import { isHandoffInProject } from "../domain/project-scope.js";
import { reconcileHandoffRecord } from "./reconcile-handoff-record.usecase.js";

export interface HandoffReadOptions {
  readonly taskStore?: Pick<TaskQueryPort, "get">;
  readonly currentProjectRoot?: string;
}

export interface HandoffListReadOptions extends HandoffReadOptions {
  readonly openOnly?: boolean;
}

interface HandoffScopeOptions extends HandoffReadOptions {
  readonly projectScoped: boolean;
}

export async function listProjectHandoffs(
  store: HandoffStorePort,
  options: HandoffListReadOptions = {},
): Promise<readonly HandoffRecord[]> {
  return readHandoffList(store, { ...options, projectScoped: true });
}

export async function listAllHandoffs(
  store: HandoffStorePort,
  options: HandoffListReadOptions = {},
): Promise<readonly HandoffRecord[]> {
  return readHandoffList(store, { ...options, projectScoped: false });
}

export async function showProjectHandoff(
  store: HandoffStorePort,
  id: string,
  options: HandoffReadOptions = {},
): Promise<HandoffRecord> {
  return readHandoffById(store, id, { ...options, projectScoped: true });
}

export async function showAnyHandoff(
  store: HandoffStorePort,
  id: string,
  options: HandoffReadOptions = {},
): Promise<HandoffRecord> {
  return readHandoffById(store, id, { ...options, projectScoped: false });
}

export async function listOpenProjectHandoffIdsForTask(
  store: HandoffStorePort,
  taskId: string,
  options: HandoffReadOptions = {},
): Promise<readonly string[]> {
  const relevantOpen = options.currentProjectRoot
    ? await store.listOpenForTask({ taskId, projectRoot: options.currentProjectRoot })
    : (await store.list()).filter((record) => (
        record.refs.taskId === taskId
        && isOpenHandoffRecord(record)
      ));
  const reconciled = await reconcileHandoffRecords(store, relevantOpen, options);
  return reconciled
    .filter(isOpenHandoffRecord)
    .sort(byNewestFirst)
    .map((record) => record.id);
}

async function readHandoffList(
  store: HandoffStorePort,
  options: HandoffListReadOptions & { readonly projectScoped: boolean },
): Promise<readonly HandoffRecord[]> {
  const all = await store.list();
  const visible = filterProjectScope(all, options);
  const candidates = options.openOnly ? visible.filter(isOpenHandoffRecord) : visible;
  const reconciled = await reconcileHandoffRecords(store, candidates, options);
  const filtered = options.openOnly ? reconciled.filter(isOpenHandoffRecord) : reconciled;
  return [...filtered].sort(byNewestFirst);
}

async function readHandoffById(
  store: HandoffStorePort,
  id: string,
  options: HandoffScopeOptions,
): Promise<HandoffRecord> {
  const record = await store.get(id);
  if (!record || !isRecordVisible(record, options)) {
    throw new MaestroError(
      `Handoff packet not found: ${id}`,
      ["Run `maestro handoff list` to see available packets"],
      "HANDOFF_NOT_FOUND",
    );
  }
  return reconcileHandoffRecords(store, [record], options).then((records) => records[0]!);
}

async function reconcileHandoffRecords(
  store: HandoffStorePort,
  records: readonly HandoffRecord[],
  options: HandoffReadOptions,
): Promise<readonly HandoffRecord[]> {
  if (!options.taskStore || !options.currentProjectRoot) {
    return records;
  }
  return Promise.all(records.map((record) => (
    record.refs.taskId
      ? reconcileHandoffRecord({
          handoffStore: store,
          taskStore: options.taskStore!,
          currentProjectRoot: options.currentProjectRoot,
        }, record)
      : record
  )));
}

function filterProjectScope(
  records: readonly HandoffRecord[],
  options: HandoffScopeOptions,
): readonly HandoffRecord[] {
  if (!options.projectScoped || !options.currentProjectRoot) {
    return records;
  }
  return records.filter((record) => isHandoffInProject(record, options.currentProjectRoot!));
}

function isRecordVisible(record: HandoffRecord, options: HandoffScopeOptions): boolean {
  return !options.projectScoped
    || !options.currentProjectRoot
    || isHandoffInProject(record, options.currentProjectRoot);
}

function byNewestFirst(left: HandoffRecord, right: HandoffRecord): number {
  return left.createdAt < right.createdAt ? 1 : -1;
}
