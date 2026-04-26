import { TASK_ID_PATTERN } from "../domain/task-id.js";
import { TASK_PRIORITIES, TASK_TYPES } from "../domain/task-types.js";

const TASK_ID_REGEX_SOURCE = TASK_ID_PATTERN.source;
const TASK_REFERENCE_REGEX_SOURCE = "^(?:" + TASK_ID_REGEX_SOURCE.replace(/^\^|\$$/g, "") + "|[^\\s].*)$";

export function buildBatchInputSchema(): Record<string, unknown> {
  return {
    $schema: "https://json-schema.org/draft/2020-12/schema",
    $id: "https://maestrocli.dev/schemas/task-plan-batch-input.json",
    title: "Maestro task plan batch input",
    description:
      "Input accepted by `maestro task plan --file`. One valid task fails the whole batch; see `task plan --help` for semantics.",
    type: "object",
    additionalProperties: false,
    required: ["tasks"],
    properties: {
      batchId: {
        type: "string",
        description:
          "Optional idempotency key. Replays a prior receipt when the same id is submitted twice.",
        minLength: 1,
        maxLength: 64,
      },
      tasks: {
        type: "array",
        description: "One or more tasks to create atomically.",
        minItems: 1,
        maxItems: 500,
        items: { $ref: "#/$defs/BatchTaskInput" },
      },
    },
    $defs: {
      BatchTaskInput: {
        type: "object",
        additionalProperties: false,
        required: ["title"],
        properties: {
          name: {
            type: "string",
            description:
              "Optional batch-local symbolic name. Referenced by other tasks' `parent`/`blockedBy`. Must not match the `tsk-*` id pattern.",
            not: { pattern: TASK_ID_REGEX_SOURCE },
          },
          title: {
            type: "string",
            description: "Short human-readable title. Required, non-empty after trim.",
            minLength: 1,
          },
          description: {
            type: "string",
            description: "Longer description or notes.",
          },
          type: {
            type: "string",
            description: "Task kind.",
            enum: [...TASK_TYPES],
          },
          priority: {
            type: "integer",
            description: "Priority. 0 is highest.",
            enum: [...TASK_PRIORITIES],
          },
          labels: {
            type: "array",
            description: "Freeform labels.",
            items: { type: "string", minLength: 1 },
          },
          parent: {
            type: "string",
            description:
              "Parent reference. Either a real `tsk-*` id, a batch-local `name`, or another entry's `slug`. Forbidden in combination with `slug` on this entry (only top-level tasks carry slugs).",
            pattern: TASK_REFERENCE_REGEX_SOURCE,
          },
          slug: {
            type: "string",
            description:
              "Mandatory human-readable slug for top-level entries. Shape: '<verb>/<kebab>' (verbs: implement, fix, chore, spike, epic). When omitted on a top-level entry, derived from the title. Forbidden when `parent` is set.",
            pattern: "^(?:implement|fix|chore|spike|epic)/[a-z0-9]+(?:-[a-z0-9]+)*$",
            maxLength: 60,
          },
          blockedBy: {
            type: "array",
            description:
              "Blocker references. Each element is either a real `tsk-*` id OR a batch-local `name`.",
            items: {
              type: "string",
              pattern: TASK_REFERENCE_REGEX_SOURCE,
            },
          },
        },
      },
    },
  };
}
