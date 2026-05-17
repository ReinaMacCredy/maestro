import { readdir } from "node:fs/promises";
import { join } from "node:path";
import { dirExists, fileExists } from "@/shared/lib/fs.js";

export type SetupCheckSeverity = "ok" | "warn" | "missing";

export interface SetupCheckEntry {
  readonly path: string;
  readonly kind: "directory" | "file" | "pack";
  readonly status: SetupCheckSeverity;
  readonly detail?: string;
}

export interface SetupCheckReport {
  readonly ok: boolean;
  readonly entries: readonly SetupCheckEntry[];
}

export interface SetupCheckDeps {
  readonly repoRoot: string;
}

const V2_DIRECTORIES: readonly { path: string; kind: "directory" }[] = [
  { path: ".maestro/tasks", kind: "directory" },
  { path: ".maestro/plans", kind: "directory" },
  { path: ".maestro/evidence", kind: "directory" },
  { path: ".maestro/runs", kind: "directory" },
  { path: "docs/principles", kind: "directory" },
];

const PRINCIPLES_DIR = "docs/principles";

export async function setupCheck(deps: SetupCheckDeps): Promise<SetupCheckReport> {
  const entries: SetupCheckEntry[] = [];

  for (const dir of V2_DIRECTORIES) {
    const abs = join(deps.repoRoot, dir.path);
    const exists = await dirExists(abs);
    entries.push({
      path: dir.path,
      kind: "directory",
      status: exists ? "ok" : "missing",
      detail: exists ? undefined : "directory not found; run `maestro setup`",
    });
  }

  const principlesAbs = join(deps.repoRoot, PRINCIPLES_DIR);
  if (await dirExists(principlesAbs)) {
    const principleFiles = await listPrincipleMarkdown(principlesAbs);
    entries.push({
      path: `${PRINCIPLES_DIR}/*.md`,
      kind: "pack",
      status: principleFiles.length === 0 ? "warn" : "ok",
      detail:
        principleFiles.length === 0
          ? "no principles found; run `maestro setup` to seed the default pack"
          : `${principleFiles.length} principle file${principleFiles.length === 1 ? "" : "s"}`,
    });
  } else {
    entries.push({
      path: `${PRINCIPLES_DIR}/*.md`,
      kind: "pack",
      status: "missing",
      detail: "docs/principles/ not present",
    });
  }

  const configPath = ".maestro/config.yaml";
  const configExists = await fileExists(join(deps.repoRoot, configPath));
  entries.push({
    path: configPath,
    kind: "file",
    status: configExists ? "ok" : "warn",
    detail: configExists ? undefined : "config.yaml not present (optional)",
  });

  // ok semantics: nothing is `missing`. `warn` entries (empty principles pack,
  // absent config.yaml) are informational and do not gate the report.
  const ok = entries.every((e) => e.status !== "missing");
  return { ok, entries };
}

async function listPrincipleMarkdown(dir: string): Promise<readonly string[]> {
  try {
    const entries = await readdir(dir);
    return entries.filter((name) => name.endsWith(".md") && name !== ".gitkeep");
  } catch {
    return [];
  }
}
