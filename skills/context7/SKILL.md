---
name: context7
description: "Fetches up-to-date library documentation via Context7 MCP for external libraries, APIs, and frameworks. Use when working with external dependencies or unfamiliar APIs."
triggers:
  - "documentation"
  - "library"
  - "api"
  - "docs"
  - "context7"
priority: 30
---

# Context7 — Library Documentation

> Always use Context7 MCP tools to fetch up-to-date library documentation when the task involves external libraries, APIs, or frameworks. Do not rely on training data for library-specific APIs — fetch current docs instead.

## MCP Tools

| Tool | Purpose | Parameters |
|------|---------|------------|
| `resolve-library-id` | Resolves a library name (e.g., "nextjs", "supabase") into a Context7-compatible library ID | `query` (the user's question/task), `libraryName` (the library to search for) |
| `query-docs` | Retrieves documentation for a resolved library | `libraryId` (e.g., `/vercel/next.js`), `query` (what to find in the docs) |

## Usage Workflow

1. Identify libraries/frameworks in the design request
2. Call `resolve-library-id` with the library name to get the Context7 library ID
3. Call `query-docs` with the library ID and a focused query to get relevant docs
4. If you already know the library ID (slash syntax like `/supabase/supabase`), skip step 2
5. For version-specific docs, include the version in the query

## Prerequisites

Context7 MCP server must be configured. Install with:

```bash
claude mcp add context7 -- npx -y @upstash/context7-mcp --api-key YOUR_API_KEY
```

Get a free API key at context7.com/dashboard

## When to Use

- Working with external libraries, APIs, or frameworks
- Need version-specific API documentation
- Verifying whether an API exists or checking its current signature
- Setting up or configuring a third-party tool

## When NOT to Use

- Pure internal codebase changes with no external dependencies
- Simple refactors or bug fixes that don't touch library APIs
- When you already have sufficient context from codebase research
