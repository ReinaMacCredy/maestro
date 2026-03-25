/**
 * Shared worker rules constant.
 * Used by pre-agent hook injection and DCP preview.
 */

export const WORKER_RULES = `
## Worker Rules
- Call maestro_task_done with a summary when your work is complete.
- Call maestro_task_block with a reason if you are stuck and need a decision.
- Do not start or claim other tasks.
- Focus exclusively on the task described in this spec.
`.trim();
