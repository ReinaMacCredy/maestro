import type { MaestroConfig } from "../domain/types.js";
import { DEFAULT_PROMPT_TEMPLATE } from "../domain/defaults.js";
import { renderTemplate } from "../lib/template.js";

export interface GeneratePromptOpts {
  readonly agent?: string;
  readonly task?: string;
  readonly instructions?: string;
  readonly handoffId: string;
}

export function generatePrompt(
  config: MaestroConfig,
  opts: GeneratePromptOpts,
): string {
  const template = config.promptTemplate ?? DEFAULT_PROMPT_TEMPLATE;
  const agent = opts.agent ?? config.defaultAgent ?? "TARGET_AGENT";

  return renderTemplate(template, {
    agent,
    task: opts.task ?? "",
    instructions: opts.instructions ?? "",
    handoffId: opts.handoffId,
  });
}
