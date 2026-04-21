import {
  findMaestroProjectRoot,
  formatStartupPointer,
  resolveLatestTaskContinuation,
} from "./_task-continuation.mjs";

async function main() {
  const projectRoot = await findMaestroProjectRoot();
  if (!projectRoot) {
    return;
  }

  const result = await resolveLatestTaskContinuation(projectRoot);
  const additionalContext = formatStartupPointer(result);
  if (!additionalContext) {
    return;
  }

  process.stdout.write(JSON.stringify({
    hookSpecificOutput: {
      hookEventName: "SessionStart",
      additionalContext,
    },
  }));
}

main().catch(() => {
  process.exit(0);
});
