export function resolveTaskSilentMode(opts: { silent?: unknown }): boolean {
  if (opts.silent === true) {
    return true;
  }

  const envFlag = process.env.MAESTRO_TASK_SILENT?.toLowerCase();
  return envFlag === "1" || envFlag === "true";
}
