# JavaScript style

Follow the repository formatter and module system.

- Use ES modules, `const` by default, `let` for reassignment, strict equality,
  and braces for control flow. Do not use `var`.
- Prefer named exports, object and array literals, shorthand properties,
  `for-of`, and template literals when interpolation improves clarity.
- Keep async control flow explicit. Await promises or return them, preserve
  rejection context, and define cancellation or cleanup where needed.
- Avoid mutation of built-in prototypes, implicit globals, string-to-code
  execution, and reliance on automatic semicolon insertion.
- Validate untyped input once at the boundary and keep internal data shapes
  consistent.
- Test observable module or consumer behavior and run the configured formatter,
  lint, and tests.
