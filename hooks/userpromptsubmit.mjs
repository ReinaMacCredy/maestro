import {
  extractPromptFromHookPayload,
  findMaestroProjectRoot,
  formatResumeContext,
  isResumeIntent,
  resolveLatestTaskContinuation,
} from "./_task-continuation.mjs";

async function readHookPayload() {
  let raw = "";
  for await (const chunk of process.stdin) {
    raw += chunk;
  }
  if (!raw.trim()) {
    return null;
  }
  try {
    return JSON.parse(raw);
  } catch {
    return null;
  }
}

async function main() {
  const payload = await readHookPayload();
  const prompt = extractPromptFromHookPayload(payload);
  if (!isResumeIntent(prompt)) {
    return;
  }

  const projectRoot = await findMaestroProjectRoot();
  if (!projectRoot) {
    return;
  }

  const result = await resolveLatestTaskContinuation(projectRoot);
  const additionalContext = formatResumeContext(result);
  if (!additionalContext) {
    return;
  }

  process.stdout.write(JSON.stringify({
    hookSpecificOutput: {
      hookEventName: "UserPromptSubmit",
      additionalContext,
    },
  }));
}

main().catch(() => {
  process.exit(0);
});
