import { runCiVerify } from "./usecases/run-ci-verify.js";
import type { RunCiVerifyArgs, RunCiVerifyDeps } from "./usecases/run-ci-verify.js";
import type { Verdict } from "@/features/verdict/domain/types.js";

export interface CiServices {
  readonly runCiVerify: (args: RunCiVerifyArgs, deps: RunCiVerifyDeps) => Promise<Verdict>;
}

export function buildCiServices(): CiServices {
  return { runCiVerify };
}
