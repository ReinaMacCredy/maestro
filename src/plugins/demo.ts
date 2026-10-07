import { existsSync } from "node:fs";
import { mkdir, readdir, stat, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { CliError, type CliResult } from "../kernel/cli.ts";
import type { BuiltInPlugin } from "../kernel/loader.ts";
import { resolveStoreLocation } from "../kernel/store.ts";

const slugPattern = /^[A-Za-z0-9][A-Za-z0-9._-]*$/;

function demoRoot(): string {
  return join(resolveStoreLocation(process.cwd()).root, ".maestro", "demo");
}

// The folder ignores itself so a repo that never ran maestro install still
// keeps demos out of git.
async function ensureDemoRoot(root: string): Promise<void> {
  await mkdir(root, { recursive: true });
  const ignore = join(root, ".gitignore");
  if (!existsSync(ignore)) await writeFile(ignore, "*\n");
}

async function listFiles(directory: string, prefix = ""): Promise<string[]> {
  const files: string[] = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const name = `${prefix}${entry.name}`;
    if (entry.isDirectory()) files.push(...await listFiles(join(directory, entry.name), `${name}/`));
    else files.push(name);
  }
  return files.sort();
}

export const demoPlugin: BuiltInPlugin = {
  name: "demo",
  apply(context) {
    context.effect(() =>
      context.cli.register(
        "demo path",
        async (invocation): Promise<CliResult> => {
          const slug = invocation.positionals[0]?.trim() ?? "";
          if (!slugPattern.test(slug)) {
            throw new CliError(
              "INVALID_DEMO_SLUG",
              `demo slug must be one path segment of letters, digits, '.', '_' or '-': ${slug}`,
            );
          }
          const root = demoRoot();
          await ensureDemoRoot(root);
          const directory = join(root, slug);
          await mkdir(directory, { recursive: true });
          return { data: { slug, directory }, text: directory };
        },
        {
          description: "Create the demo folder for a slug and print its absolute path.",
          mutates: true,
          positionals: [{ name: "slug", required: true }],
          rootDescription: "Keep throwaway HTML demos and their assets in .maestro/demo, out of git.",
        },
      ),
    );

    context.effect(() =>
      context.cli.register(
        "demo list",
        async (): Promise<CliResult> => {
          const root = demoRoot();
          const demos = [];
          if (existsSync(root)) {
            for (const entry of await readdir(root, { withFileTypes: true })) {
              if (!entry.isDirectory()) continue;
              const directory = join(root, entry.name);
              demos.push({
                slug: entry.name,
                directory,
                files: await listFiles(directory),
                modifiedAt: (await stat(directory)).mtime.toISOString(),
              });
            }
          }
          demos.sort((left, right) => left.slug.localeCompare(right.slug));
          return {
            data: { demos },
            text: demos.length > 0
              ? demos.map((demo) => `${demo.slug}  ${demo.files.join(", ")}`).join("\n")
              : 'no demos; run: maestro demo path "<slug>"',
          };
        },
        { description: "List demos under .maestro/demo with their files.", mutates: false },
      ),
    );
  },
};
