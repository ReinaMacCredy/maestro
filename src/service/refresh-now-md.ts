import type { TaskStorePort } from "../repo/task-store.port.js";
import type { NowMdWriterPort } from "../repo/now-md-writer.port.js";
import type { CoreServices } from "../providers/build-services.js";

export interface RefreshNowMdInput {
  readonly taskStore: TaskStorePort;
  readonly nowMdWriter: NowMdWriterPort;
}

// Errors are swallowed: the dashboard is derived state, so a failure to
// refresh it must never block the real task mutation that triggered it.
export async function refreshNowMd(input: RefreshNowMdInput): Promise<void> {
  const { taskStore, nowMdWriter } = input;
  try {
    const tasks = await taskStore.list();
    await nowMdWriter.write(tasks, new Date());
  } catch (err) {
    console.warn(
      `maestro: NOW.md refresh failed (${(err as Error).message ?? err})`,
    );
  }
}

export async function refreshNowMdFromServices(services: CoreServices): Promise<void> {
  await refreshNowMd({
    taskStore: services.taskStore,
    nowMdWriter: services.nowMdWriter,
  });
}
