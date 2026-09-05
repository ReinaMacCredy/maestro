import { expect, test } from "bun:test";
import { readFile, readdir } from "node:fs/promises";
import { join } from "node:path";

test("graph-no-slp: the graph plugin calls no dispatch, handback, team or herdr code and reads slp-v2 only through requireSlpActor (red 13, A7)", async () => {
  const pluginDirectory = join(import.meta.dir, "..", "src", "plugins");
  const sources = (await readdir(pluginDirectory)).filter((entry) => entry.startsWith("graph") && entry.endsWith(".ts"));
  expect(sources.sort()).toEqual(["graph-file.ts", "graph.ts"]);
  for (const source of sources) {
    const text = await readFile(join(pluginDirectory, source), "utf8");
    expect({ source, hits: text.match(/dispatch|handback|team |herdr|HerdrClient|slp-runtime|slp-process|slp-attention/gi) ?? [] }).toEqual({ source, hits: [] });
    const slpImports = [...text.matchAll(/import \{([^}]*)\} from "\.\/slp-v2\.ts"/g)].map((match) => (match[1] ?? "").trim());
    expect(slpImports.length <= 1).toBe(true);
    if (slpImports[0] !== undefined) expect(slpImports[0]).toBe("requireSlpActor");
  }
});
