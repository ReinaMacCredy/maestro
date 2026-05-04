import { runCiVerify } from "./usecases/run-ci-verify.js";
import type { RunCiVerifyArgs, RunCiVerifyDeps } from "./usecases/run-ci-verify.js";
import { GhCliAdapter } from "./adapters/gh-cli.adapter.js";
import type { GithubApiPort } from "./ports/github-api.port.js";
import type { Verdict } from "@/features/verdict/domain/types.js";

export interface CiServices {
  readonly runCiVerify: (args: RunCiVerifyArgs, deps: RunCiVerifyDeps) => Promise<Verdict>;
  readonly githubApi: GithubApiPort;
}

export function buildCiServices(): CiServices {
  return {
    runCiVerify,
    githubApi: new GhCliAdapter(),
  };
}
