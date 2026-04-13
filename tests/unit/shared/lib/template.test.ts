import { describe, expect, it } from "bun:test";
import { renderTemplate } from "@/shared/lib/template.js";

describe("renderTemplate", () => {
  it("substitutes simple variables", () => {
    const result = renderTemplate("hello {{name}}", { name: "codex" });
    expect(result).toBe("hello codex");
  });

  it("substitutes multiple variables", () => {
    const result = renderTemplate("{{a}} and {{b}}", { a: "x", b: "y" });
    expect(result).toBe("x and y");
  });

  it("replaces missing variables with empty string", () => {
    const result = renderTemplate("hello {{name}}", {});
    expect(result).toBe("hello ");
  });

  it("renders conditional block when var is truthy", () => {
    const result = renderTemplate("{{#task}}do: {{task}}{{/task}}", {
      task: "build it",
    });
    expect(result).toBe("do: build it");
  });

  it("strips conditional block when var is missing", () => {
    const result = renderTemplate("before{{#task}} task: {{task}}{{/task}} after", {});
    expect(result).toBe("before after");
  });

  it("strips conditional block when var is empty string", () => {
    const result = renderTemplate("{{#task}}task: {{task}}{{/task}}done", {
      task: "",
    });
    expect(result).toBe("done");
  });

  it("handles multiline conditional blocks", () => {
    const tmpl = `line1
{{#task}}Your task: {{task}}
{{/task}}line2`;
    const withTask = renderTemplate(tmpl, { task: "deploy" });
    expect(withTask).toBe("line1\nYour task: deploy\nline2");

    const withoutTask = renderTemplate(tmpl, {});
    expect(withoutTask).toBe("line1\nline2");
  });

  it("handles multiple conditional blocks", () => {
    const tmpl = "{{#a}}A={{a}}{{/a}} {{#b}}B={{b}}{{/b}}";
    expect(renderTemplate(tmpl, { a: "1", b: "2" })).toBe("A=1 B=2");
    expect(renderTemplate(tmpl, { a: "1" })).toBe("A=1 ");
    expect(renderTemplate(tmpl, {})).toBe(" ");
  });

  it("works with the default prompt template shape", () => {
    const tmpl = `pickup --agent {{agent}}
{{#task}}Task: {{task}}
{{/task}}report when done`;

    const result = renderTemplate(tmpl, { agent: "codex", task: "fix bug" });
    expect(result).toContain("--agent codex");
    expect(result).toContain("Task: fix bug");

    const noTask = renderTemplate(tmpl, { agent: "gemini" });
    expect(noTask).toContain("--agent gemini");
    expect(noTask).not.toContain("Task:");
  });
});
