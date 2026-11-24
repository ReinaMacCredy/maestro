import { expect, test } from "bun:test";
import { readFile } from "node:fs/promises";
import { dirname, join, relative, resolve, sep } from "node:path";

const root = join(import.meta.dir, "..");

// The SLP modules: the descriptor src/plugins/slp.ts and every file under src/plugins/slp/, herdr-client.ts among them.
function isSlpModule(path: string): boolean {
  return path === join(root, "src", "plugins", "slp.ts") || path.startsWith(join(root, "src", "plugins", "slp") + sep);
}

test("d879 [lint] outside the SLP modules only src/plugins/index.ts imports one, the descriptor, and bin/maestro.ts imports none", async () => {
  const files: string[] = [];
  for (const pattern of ["src/**/*.ts", "bin/*.ts"]) {
    for await (const file of new Bun.Glob(pattern).scan({ cwd: root })) files.push(join(root, file));
  }
  expect(files).toContain(join(root, "bin", "maestro.ts"));
  const crossings: string[] = [];
  for (const file of files.sort()) {
    if (isSlpModule(file)) continue;
    const text = await readFile(file, "utf8");
    for (const match of text.matchAll(/(?:from\s+|import\s*\(\s*)["'](\.{1,2}\/[^"']+)["']/g)) {
      const target = resolve(dirname(file), match[1]!);
      if (isSlpModule(target)) crossings.push(`${relative(root, file)} -> ${relative(root, target)}`);
    }
  }
  expect(crossings).toEqual(["src/plugins/index.ts -> src/plugins/slp.ts"]);
  const descriptor = await readFile(join(root, "src", "plugins", "slp.ts"), "utf8");
  const staticImports = [...descriptor.matchAll(/from\s+["'](\.{1,2}\/[^"']+)["']/g)]
    .map((match) => resolve(join(root, "src", "plugins"), match[1]!));
  expect(staticImports.filter(isSlpModule)).toEqual([]);
});
