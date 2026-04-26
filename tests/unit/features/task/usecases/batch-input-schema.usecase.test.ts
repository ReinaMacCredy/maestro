import { beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { JsonlTaskStoreAdapter } from "@/features/task/adapters/jsonl-task-store.adapter.js";
import { planTasks } from "@/features/task/usecases/plan-tasks.usecase.js";
import { buildBatchInputSchema } from "@/features/task/usecases/batch-input-schema.usecase.js";

describe("buildBatchInputSchema", () => {
  it("returns a JSON Schema document with the expected top-level shape", () => {
    const schema = buildBatchInputSchema();

    expect(schema.$schema).toBe("https://json-schema.org/draft/2020-12/schema");
    expect(schema.type).toBe("object");
    expect(schema.required).toEqual(["tasks"]);
    const props = schema.properties as Record<string, unknown>;
    expect(props).toHaveProperty("batchId");
    expect(props).toHaveProperty("tasks");

    const tasks = props.tasks as Record<string, unknown>;
    expect(tasks.type).toBe("array");
    expect(tasks.minItems).toBe(1);
    expect(tasks.maxItems).toBe(500);
  });

  it("defines BatchTaskInput with title required", () => {
    const schema = buildBatchInputSchema();
    const defs = schema.$defs as Record<string, { required: string[]; properties: Record<string, unknown> }>;
    expect(defs.BatchTaskInput.required).toEqual(["title"]);
    expect(defs.BatchTaskInput.properties).toHaveProperty("title");
    expect(defs.BatchTaskInput.properties).toHaveProperty("type");
    expect(defs.BatchTaskInput.properties).toHaveProperty("priority");
    expect(defs.BatchTaskInput.properties).toHaveProperty("labels");
    expect(defs.BatchTaskInput.properties).toHaveProperty("parent");
    expect(defs.BatchTaskInput.properties).toHaveProperty("blockedBy");
    expect(defs.BatchTaskInput.properties).toHaveProperty("slug");
  });

  it("enumerates the same task types and priorities the validator accepts", () => {
    const schema = buildBatchInputSchema();
    const defs = schema.$defs as Record<string, { properties: Record<string, { enum?: unknown[] }> }>;
    const typeEnum = defs.BatchTaskInput.properties.type.enum ?? [];
    expect(typeEnum).toEqual(expect.arrayContaining(["task", "bug", "feature", "epic", "chore"]));
    const priorityEnum = defs.BatchTaskInput.properties.priority.enum ?? [];
    expect(priorityEnum).toEqual([0, 1, 2, 3, 4]);
  });

  it("serializes to valid JSON", () => {
    const schema = buildBatchInputSchema();
    const serialized = JSON.stringify(schema);
    expect(() => JSON.parse(serialized)).not.toThrow();
  });
});

describe("buildBatchInputSchema / planTasks drift guard", () => {
  let tmpDir: string;
  let store: JsonlTaskStoreAdapter;

  beforeEach(async () => {
    tmpDir = await mkdtemp(join(tmpdir(), "task-plan-schema-"));
    store = new JsonlTaskStoreAdapter(tmpDir);
  });

  it("accepts a fixture that exercises every documented field", async () => {
    const result = await planTasks(store, {
      batchId: "drift-guard-fixture",
      tasks: [
        {
          name: "root",
          title: "Root task",
          description: "top-level",
          type: "epic",
          priority: 0,
          labels: ["schema", "drift"],
        },
        {
          name: "child",
          title: "Child task",
          type: "feature",
          priority: 2,
          parent: "root",
          blockedBy: ["root"],
        },
      ],
    });
    expect(result.created).toHaveLength(2);
  });
});
