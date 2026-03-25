import { readStdin, writeOutput, resolveProjectDir, logHookError, HOOK_EVENTS } from './_helpers.ts';

const HOOK_NAME = 'pretooluse';

async function main(): Promise<void> {
  const input = await readStdin();
  const toolName = input.tool_name as string | undefined;
  const toolInput = (input.tool_input as Record<string, unknown>) || {};

  if (toolName !== 'Bash') return;

  const command = (toolInput.command as string) || '';
  if (!/\bgit\s+commit\b/.test(command)) {
    return;
  }

  if (!resolveProjectDir()) {
    return;
  }

  writeOutput({
    hookSpecificOutput: {
      hookEventName: HOOK_EVENTS.PreToolUse,
      additionalContext:
        'If this commit belongs to a maestro task, finish the task with maestro_task_finish so report and audit metadata are recorded.',
    },
  });
}

try {
  await main();
} catch (error) {
  logHookError(resolveProjectDir(), HOOK_NAME, error);
}
