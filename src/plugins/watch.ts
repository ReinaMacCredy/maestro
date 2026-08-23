import type { CliResult } from "../kernel/cli.ts";
import type { BuiltInPlugin } from "../kernel/loader.ts";
import type { WorkRecord, WorkService } from "./work.ts";

function renderTree(works: WorkRecord[]): string[] {
  const byParent = new Map<string | null, WorkRecord[]>();
  for (const work of works) {
    const children = byParent.get(work.parentId) ?? [];
    children.push(work);
    byParent.set(work.parentId, children);
  }
  const lines: string[] = [];
  const visit = (parentId: string | null, depth: number): void => {
    for (const work of byParent.get(parentId) ?? []) {
      lines.push(`${"  ".repeat(depth)}${work.id} [${work.state}] ${work.title}`);
      visit(work.id, depth + 1);
    }
  };
  visit(null, 0);
  return lines;
}

export const watchPlugin: BuiltInPlugin = {
  name: "watch",
  inject: ["work"],
  apply(context) {
    const snapshot = (): string => {
      const work = context.work as WorkService;
      const sessions = context.sessions.list().filter((session) => session.live);
      const tree = renderTree(work.list());
      return [
        "work",
        ...(tree.length > 0 ? tree : ["  none"]),
        "sessions",
        ...(sessions.length > 0
          ? sessions.map((session) => `  ${session.id} ${session.lastEvent} pid=${session.pid}`)
          : ["  none"]),
      ].join("\n");
    };

    context.effect(() =>
      context.cli.register(
        "watch",
        async (invocation): Promise<CliResult> => {
          if (invocation.options.once) {
            const text = snapshot();
            return { data: { snapshot: text }, text };
          }
          while (true) {
            process.stdout.write(`\u001b[2J\u001b[H${snapshot()}\n`);
            await Bun.sleep(1000);
          }
        },
        { "--once": {} },
      ),
    );
  },
};
