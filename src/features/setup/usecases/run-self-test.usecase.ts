import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { recordEvidence, FsEvidenceStoreAdapter } from "@/features/evidence/index.js";

export interface SelfTestStep {
  readonly name: string;
  readonly ok: boolean;
  readonly detail?: string;
}

export interface SelfTestReport {
  readonly ok: boolean;
  readonly steps: readonly SelfTestStep[];
  readonly sandboxRoot: string;
}

export interface RunSelfTestDeps {
  readonly makeSandbox?: () => Promise<string>;
  readonly removeSandbox?: (path: string) => Promise<void>;
}

const TASK_ID = "tsk-ab12cd";

export async function runSetupSelfTest(deps: RunSelfTestDeps = {}): Promise<SelfTestReport> {
  const make = deps.makeSandbox ?? defaultMakeSandbox;
  const remove = deps.removeSandbox ?? defaultRemoveSandbox;

  const sandboxRoot = await make();
  const steps: SelfTestStep[] = [];
  let ok = true;

  try {
    steps.push(await tryStep("layout", async () => {
      await mkdir(join(sandboxRoot, ".maestro/runs", TASK_ID), { recursive: true });
      await mkdir(join(sandboxRoot, ".maestro/evidence"), { recursive: true });
      await writeFile(join(sandboxRoot, ".maestro/runs", TASK_ID, "state.json"), "{}\n", "utf8");
    }));

    steps.push(await tryStep("evidence-store", async () => {
      const store = new FsEvidenceStoreAdapter(sandboxRoot);
      const row = await recordEvidence(store, {
        task_id: TASK_ID,
        kind: "manual-note",
        witness_level: "agent-claimed-locally",
        payload: { note: "setup-self-test" },
      });
      if (!row.id) throw new Error("evidence row has no id");
    }));
  } catch (err) {
    ok = false;
    steps.push({ name: "fatal", ok: false, detail: err instanceof Error ? err.message : String(err) });
  } finally {
    await remove(sandboxRoot).catch(() => undefined);
  }

  if (steps.some((s) => !s.ok)) ok = false;
  return { ok, steps, sandboxRoot };
}

async function tryStep(name: string, fn: () => Promise<void>): Promise<SelfTestStep> {
  try {
    await fn();
    return { name, ok: true };
  } catch (err) {
    return { name, ok: false, detail: err instanceof Error ? err.message : String(err) };
  }
}

async function defaultMakeSandbox(): Promise<string> {
  return mkdtemp(join(tmpdir(), "maestro-setup-selftest-"));
}

async function defaultRemoveSandbox(path: string): Promise<void> {
  await rm(path, { recursive: true, force: true });
}
