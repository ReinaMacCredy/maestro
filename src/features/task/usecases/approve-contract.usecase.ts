import { MaestroError } from "@/shared/errors.js";
import type { ContractVersionStorePort } from "../ports/contract-version-store.port.js";

export async function approveContract(
  store: ContractVersionStorePort,
  taskId: string,
  lockedBy: string,
  lockedAt: string,
): Promise<void> {
  const current = await store.readCurrent(taskId);
  if (current === undefined) {
    throw new MaestroError(
      `No contract found for task ${taskId} — propose one first`,
      ["Call proposeContract before approveContract"],
    );
  }
  if (current.status === "locked") {
    throw new MaestroError(
      `Contract for task ${taskId} is already locked`,
      ["A locked contract cannot be re-approved"],
    );
  }

  const versions = await store.history(taskId);
  const nextVersion = versions.length + 1;

  await store.write(taskId, nextVersion, {
    ...current,
    status: "locked",
    lockedAt,
    lockedBy,
  });
}
