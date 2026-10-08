import { existsSync } from "node:fs";
import { mkdir, readdir, stat, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { CliError, type CliResult } from "../kernel/cli.ts";
import type { BuiltInPlugin } from "../kernel/loader.ts";
import { resolveStoreLocation } from "../kernel/store.ts";

const slugPattern = /^[A-Za-z0-9][A-Za-z0-9._-]*$/;

function topicRoot(): string {
  return join(resolveStoreLocation(process.cwd()).root, ".maestro", "topic");
}

// The folder ignores itself so a repo that never ran maestro install still
// keeps topic files out of git.
async function ensureTopicRoot(root: string): Promise<void> {
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

export const topicPlugin: BuiltInPlugin = {
  name: "topic",
  apply(context) {
    context.effect(() =>
      context.cli.register(
        "topic path",
        async (invocation): Promise<CliResult> => {
          const slug = invocation.positionals[0]?.trim() ?? "";
          if (!slugPattern.test(slug)) {
            throw new CliError(
              "INVALID_TOPIC_SLUG",
              `topic slug must be one path segment of letters, digits, '.', '_' or '-': ${slug}`,
            );
          }
          const root = topicRoot();
          await ensureTopicRoot(root);
          const directory = join(root, slug);
          await mkdir(directory, { recursive: true });
          return { data: { slug, directory }, text: directory };
        },
        {
          description: "Create the folder for a topic and print its absolute path.",
          mutates: true,
          positionals: [{ name: "slug", required: true }],
          rootDescription:
            "Keep what agents produce while discussing a topic (demos, reports, notes, captures) in .maestro/topic, out of git.",
        },
      ),
    );

    context.effect(() =>
      context.cli.register(
        "topic list",
        async (): Promise<CliResult> => {
          const root = topicRoot();
          const topics = [];
          if (existsSync(root)) {
            for (const entry of await readdir(root, { withFileTypes: true })) {
              if (!entry.isDirectory()) continue;
              const directory = join(root, entry.name);
              topics.push({
                slug: entry.name,
                directory,
                files: await listFiles(directory),
                modifiedAt: (await stat(directory)).mtime.toISOString(),
              });
            }
          }
          topics.sort((left, right) => left.slug.localeCompare(right.slug));
          return {
            data: { topics },
            text: topics.length > 0
              ? topics.map((topic) => `${topic.slug}  ${topic.files.join(", ")}`).join("\n")
              : 'no topics; run: maestro topic path "<slug>"',
          };
        },
        { description: "List topics under .maestro/topic with their files.", mutates: false },
      ),
    );
  },
};
