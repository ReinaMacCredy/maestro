---
tags: [mcp, node, import.meta.dir, bundler]
priority: 3
category: debug
---
MCP server crashes under Node.js because import.meta.dir is Bun-only. Affects loader.ts (tool manifests) and agents/loader.ts + agents/registry.ts (agent manifests, guidance, protocols). Fix: build-time generators embed all data into .generated.ts files that the bundler inlines.