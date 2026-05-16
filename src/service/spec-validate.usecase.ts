import { readFile } from "node:fs/promises";
import { isAbsolute, resolve } from "node:path";
import { parseSpecFile } from "../repo/fs-spec-store.adapter.js";
import { SpecParseError } from "../repo/spec-store.port.js";

export interface SpecValidateResult {
  readonly valid: boolean;
  readonly path: string;
  readonly errors: readonly string[];
}

export interface SpecValidateDeps {
  readonly repoRoot: string;
}

export async function specValidate(
  deps: SpecValidateDeps,
  pathArg: string,
): Promise<SpecValidateResult> {
  const path = isAbsolute(pathArg) ? pathArg : resolve(deps.repoRoot, pathArg);
  let raw: string;
  try {
    raw = await readFile(path, "utf8");
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === "ENOENT") {
      return { valid: false, path, errors: [`Spec file not found: ${pathArg}`] };
    }
    throw err;
  }
  try {
    parseSpecFile(raw, path);
    return { valid: true, path, errors: [] };
  } catch (err) {
    if (err instanceof SpecParseError) {
      return { valid: false, path, errors: [err.message] };
    }
    throw err;
  }
}
