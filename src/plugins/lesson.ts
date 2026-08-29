import { existsSync } from "node:fs";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { basename, dirname, join, resolve } from "node:path";
import {
  CliError,
  requiredPosition,
  stringOption,
  stringOptions,
  type CliInvocation,
  type CliResult,
} from "../kernel/cli.ts";
import type { BuiltInPlugin, PluginContext } from "../kernel/loader.ts";
import { registerSessionCommand } from "./session-required.ts";

export interface LessonRecord {
  answer: string | null;
  commit: string | null;
  createdAt: string;
  evidence: string[];
  expected: string;
  happened: string;
  id: string;
  processedAt: string | null;
  project: string;
  state: "pending" | "processed";
  target: string;
  updatedAt: string;
  why: string;
}

interface LessonRow {
  answer: string | null;
  commit_sha: string | null;
  created_at: string;
  evidence: string;
  expected: string;
  happened: string;
  id: string;
  processed_at: string | null;
  project: string;
  state: "pending" | "processed";
  target: string;
  updated_at: string;
  why: string;
}

export interface LessonService {
  get(id: string): LessonRecord | null;
  list(options?: { all?: boolean; project?: string }): LessonRecord[];
}

function fromRow(row: LessonRow): LessonRecord {
  return {
    answer: row.answer,
    commit: row.commit_sha,
    createdAt: row.created_at,
    evidence: JSON.parse(row.evidence) as string[],
    expected: row.expected,
    happened: row.happened,
    id: row.id,
    processedAt: row.processed_at,
    project: row.project,
    state: row.state,
    target: row.target,
    updatedAt: row.updated_at,
    why: row.why,
  };
}

function getLesson(context: PluginContext, id: string): LessonRecord | null {
  const row = context.store.database
    .query<LessonRow, [string]>("SELECT * FROM lessons WHERE id = ?")
    .get(id);
  return row ? fromRow(row) : null;
}

function requireLesson(context: PluginContext, id: string): LessonRecord {
  const lesson = getLesson(context, id);
  if (!lesson) throw new CliError("NOT_FOUND", `lesson not found: ${id}`, { id });
  return lesson;
}

// A correction is only a lesson when it carries the gap it names and the
// reason behind it; a half-filed one teaches the improver nothing.
function requiredOption(invocation: CliInvocation, flag: string, label: string): string {
  const value = stringOption(invocation, flag.slice(2))?.trim();
  if (!value) {
    throw new CliError("MISSING_ARGUMENT", `lesson file requires ${flag} <${label}>`, {
      flag,
    });
  }
  return value;
}

// The store lives at <root>/.maestro/maestro.db, so its root basename is the
// registry name the room renders its per-project view from (d723).
function defaultProject(context: PluginContext): string {
  return basename(dirname(dirname(context.store.path))) || basename(process.cwd());
}

function format(lesson: LessonRecord): string {
  return [
    `${lesson.id} [${lesson.state}] ${lesson.happened}`,
    `target: ${lesson.target}`,
    `expected: ${lesson.expected}`,
    `why: ${lesson.why}`,
    `evidence: ${lesson.evidence.join(", ")}`,
    `project: ${lesson.project}`,
    lesson.commit ? `commit: ${lesson.commit}` : null,
    lesson.answer ? `answer: ${lesson.answer}` : null,
  ]
    .filter((line): line is string => line !== null)
    .join("\n");
}

function line(lesson: LessonRecord): string {
  return `${lesson.id} [${lesson.state}] (${lesson.project}) ${lesson.target} | ${lesson.happened}`;
}

interface SourcedLesson {
  lesson: LessonRecord;
  source: string;
}

export interface RepoLessons {
  error: boolean;
  lessons: LessonRecord[];
  missing: boolean;
  repo: string;
}

async function registeredRepos(home: string): Promise<string[]> {
  const registry = join(home, "maestro", "registry");
  if (!existsSync(registry)) return [];
  return (await readFile(registry, "utf8")).split(/\r?\n/).filter(Boolean);
}

// The store a repository owns is read through its own CLI, never by opening its
// database here: the child is what keeps a store too new to read honest.
export async function readRepoLessons(repo: string): Promise<RepoLessons> {
  if (!existsSync(repo) || !existsSync(join(repo, ".maestro"))) {
    return { error: false, lessons: [], missing: true, repo };
  }
  const cli = resolve(process.argv[1] ?? join(import.meta.dir, "..", "..", "bin", "maestro.ts"));
  const child = Bun.spawn([process.execPath, cli, "lesson", "list", "--all", "--json"], {
    cwd: repo,
    env: { ...process.env, MAESTRO_READ_ONLY: "1" },
    stdout: "pipe",
    stderr: "pipe",
  });
  const [stdout, , exitCode] = await Promise.all([
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited,
  ]);
  if (exitCode !== 0) return { error: true, lessons: [], missing: false, repo };
  try {
    const envelope = JSON.parse(stdout) as { data?: { lessons?: LessonRecord[] } };
    return { error: false, lessons: envelope.data?.lessons ?? [], missing: false, repo };
  } catch {
    return { error: true, lessons: [], missing: false, repo };
  }
}

function entry({ lesson, source }: SourcedLesson): string {
  return [
    `### ${lesson.id} (${basename(source)}) ${lesson.happened}`,
    `- target: ${lesson.target}`,
    `- expected: ${lesson.expected}`,
    `- why: ${lesson.why}`,
    `- evidence: ${lesson.evidence.join(", ")}`,
    lesson.commit ? `- commit: ${lesson.commit}` : null,
    lesson.answer ? `- answer: ${lesson.answer}` : null,
    `- filed: ${lesson.createdAt}`,
    `- source: ${source}`,
  ]
    .filter((line): line is string => line !== null)
    .join("\n");
}

function view(project: string, sourced: SourcedLesson[]): string {
  const section = (title: string, state: "pending" | "processed"): string[] => {
    const entries = sourced.filter((item) => item.lesson.state === state).map(entry);
    return entries.length === 0 ? [] : [`## ${title}`, "", entries.join("\n\n")];
  };
  return [
    `# Lessons: ${project}`,
    "",
    "Rendered by `maestro lesson render` from the room store and every registered",
    "repository's store. This file is a view and is never hand-edited: an edit here",
    "is lost on the next render. File a correction with `maestro lesson file`.",
    "",
    ...section("Pending", "pending"),
    "",
    ...section("Processed", "processed"),
    "",
  ].join("\n");
}

export const lessonPlugin: BuiltInPlugin = {
  name: "lesson",
  apply(context) {
    context.store.migrate(`
      CREATE TABLE IF NOT EXISTS lessons (
        id TEXT PRIMARY KEY,
        target TEXT NOT NULL,
        happened TEXT NOT NULL,
        expected TEXT NOT NULL,
        why TEXT NOT NULL,
        evidence TEXT NOT NULL,
        project TEXT NOT NULL,
        state TEXT NOT NULL CHECK(state IN ('pending', 'processed')),
        commit_sha TEXT,
        answer TEXT,
        processed_at TEXT,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
      );
    `);

    const service: LessonService = {
      get: (id) => getLesson(context, id),
      list: (options = {}) =>
        context.store.database
          .query<LessonRow, []>("SELECT * FROM lessons ORDER BY CAST(SUBSTR(id, 2) AS INTEGER)")
          .all()
          .map(fromRow)
          .filter((lesson) => options.all === true || lesson.state === "pending")
          .filter((lesson) => options.project === undefined || lesson.project === options.project),
    };
    context.effect(() => context.provide("lesson", service));

    context.effect(() =>
      registerSessionCommand(context, "lesson file", (invocation): CliResult => {
        const happened = requiredPosition(invocation, 0, "what happened");
        const target = requiredOption(invocation, "--target", "doctrine target");
        const expected = requiredOption(invocation, "--expected", "what was expected");
        const why = requiredOption(invocation, "--why", "why");
        const evidence = stringOptions(invocation, "evidence")
          .flatMap((value) => value.split(","))
          .map((value) => value.trim())
          .filter(Boolean);
        if (evidence.length === 0) {
          throw new CliError(
            "MISSING_ARGUMENT",
            "lesson file requires --evidence <w/h/d id>",
            { flag: "--evidence" },
          );
        }
        const project = stringOption(invocation, "project")?.trim() || defaultProject(context);
        const now = new Date().toISOString();
        const transaction = context.store.database.transaction(() => {
          const id = context.store.nextPrefixedId("lessons", "l");
          context.store.database
            .query(
              `INSERT INTO lessons
                (id, target, happened, expected, why, evidence, project, state, created_at, updated_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, 'pending', ?, ?)`,
            )
            .run(id, target, happened, expected, why, JSON.stringify(evidence), project, now, now);
          context.log.append({
            type: "lesson.file",
            entityType: "lesson",
            entityId: id,
            sessionId: context.sessions.current().id,
            payload: { target, project, evidence },
          });
          return service.get(id) as LessonRecord;
        });
        const filed = transaction.immediate();
        return { data: { lesson: filed }, text: format(filed) };
      }, {
        description: "File a correction as a lesson the improver will read.",
        flags: {
          "--target": {
            description: "Name the doctrine this corrects: a recipe section, a rule, a template, a skill, or a Workspace Protocol.",
            value: true,
          },
          "--expected": { description: "What was expected instead.", value: true },
          "--why": { description: "Why the expectation is the right one.", value: true },
          "--evidence": {
            description: "A work, handback, or decision id that evidences it.",
            value: true,
            multiple: true,
          },
          "--project": {
            description: "Tag another project; defaults to this store's project.",
            value: true,
          },
        },
        positionals: [{ name: "what-happened", required: true }],
        rootDescription: "Record corrections the improver turns into doctrine edits.",
      }),
    );

    context.effect(() =>
      registerSessionCommand(context, "lesson process", (invocation): CliResult => {
        const id = requiredPosition(invocation, 0, "lesson id");
        const commit = stringOption(invocation, "commit")?.trim() || null;
        const answer = stringOption(invocation, "answer")?.trim() || null;
        if (!commit && !answer) {
          throw new CliError(
            "MISSING_ARGUMENT",
            `lesson process requires --commit <sha> for an edit or --answer <reason> for one it rejects; run: maestro lesson process ${id} --answer "<why>"`,
            { command: `maestro lesson process ${id} --answer "<why>"`, id },
          );
        }
        const transaction = context.store.database.transaction(() => {
          const lesson = requireLesson(context, id);
          if (lesson.state !== "pending") {
            throw new CliError("INVALID_STATE", `${id} is already processed`, {
              id,
              state: lesson.state,
            });
          }
          const now = new Date().toISOString();
          const marked = context.store.database
            .query(
              `UPDATE lessons
               SET state = 'processed', commit_sha = ?, answer = ?, processed_at = ?, updated_at = ?
               WHERE id = ? AND state = 'pending'`,
            )
            .run(commit, answer, now, now, id);
          if (marked.changes === 0) {
            throw new CliError("INVALID_STATE", `${id} is already processed`, { id });
          }
          context.log.append({
            type: "lesson.process",
            entityType: "lesson",
            entityId: id,
            sessionId: context.sessions.current().id,
            payload: { commit, answer },
          });
          return service.get(id) as LessonRecord;
        });
        const processed = transaction.immediate();
        return { data: { lesson: processed }, text: format(processed) };
      }, {
        description: "Mark a lesson processed by the commit that answers it, or by the reason it was rejected.",
        flags: {
          "--commit": { description: "The commit that carries the doctrine edit.", value: true },
          "--answer": { description: "Why this lesson produced no edit.", value: true },
        },
        positionals: [{ name: "id", required: true }],
      }),
    );

    context.effect(() =>
      context.cli.register("lesson show", (invocation): CliResult => {
        const lesson = requireLesson(context, requiredPosition(invocation, 0, "lesson id"));
        return { data: { lesson }, text: format(lesson) };
      }, {
        description: "Show one lesson and the evidence behind it.",
        mutates: false,
        positionals: [{ name: "id", required: true }],
      }),
    );

    context.effect(() =>
      context.cli.register("lesson list", (invocation): CliResult => {
        const lessons = service.list({
          all: invocation.options["all"] === true,
          project: stringOption(invocation, "project"),
        });
        return {
          data: { lessons },
          text: lessons.map(line).join("\n"),
        };
      }, {
        description: "List pending lessons, or every lesson with --all.",
        flags: {
          "--all": { description: "Include lessons already processed." },
          "--project": { description: "Only lessons tagged with this project.", value: true },
        },
        mutates: false,
      }),
    );

    context.effect(() =>
      context.cli.register("lesson render", async (): Promise<CliResult> => {
        const home = process.env["HOME"] ?? process.cwd();
        const local = dirname(dirname(context.store.path));
        const repos = (await registeredRepos(home)).filter((repo) => resolve(repo) !== local);
        const scanned = await Promise.all(repos.map(readRepoLessons));
        const sourced: SourcedLesson[] = [
          ...service.list({ all: true }).map((lesson) => ({ lesson, source: local })),
          ...scanned.flatMap((result) =>
            result.lessons.map((lesson) => ({ lesson, source: result.repo }))
          ),
        ];

        const byProject = new Map<string, SourcedLesson[]>();
        for (const item of sourced) {
          const group = byProject.get(item.lesson.project) ?? [];
          group.push(item);
          byProject.set(item.lesson.project, group);
        }

        const directory = join(home, "maestro", "PROJECT");
        await mkdir(directory, { recursive: true });
        const written: string[] = [];
        for (const [project, group] of [...byProject].sort(([a], [b]) => a.localeCompare(b))) {
          group.sort((left, right) => left.lesson.createdAt.localeCompare(right.lesson.createdAt));
          const path = join(directory, `${project}.md`);
          await writeFile(path, view(project, group));
          const pending = group.filter((item) => item.lesson.state === "pending").length;
          written.push(`PROJECT/${project}.md: ${group.length} lessons (${pending} pending)`);
        }

        const unavailable = scanned.flatMap((result) =>
          result.missing
            ? [`skipped: ${result.repo} (missing)`]
            : result.error
              // The child's stderr is discarded, so the line names the command
              // that shows what it said; a store left out is why the view below
              // is incomplete, so it is read before the view, not after it.
              ? [
                `Unreadable repository: ${result.repo}; run: cd ${result.repo} && maestro lesson list --all`,
              ]
              : []
        );
        const lines = [...unavailable, ...written];
        return {
          data: { directory, projects: [...byProject.keys()], written: written.length },
          text: lines.length > 0 ? lines.join("\n") : "No lessons to render.",
        };
      }, {
        description: "Render the per-project lesson view under ~/maestro/PROJECT.",
      }),
    );
  },
};
