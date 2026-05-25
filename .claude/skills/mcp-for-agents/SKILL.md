---
name: mcp-for-agents
description: >-
  Designs or reviews MCP servers so AI agents can use them reliably: outcome-oriented
  tools, flat constrained parameters, actionable errors via isError, token-efficient
  responses, composable outputs, and disciplined tool surfaces. Use when building an
  MCP server, adding tools to one, reviewing MCP tool design, or when the user mentions
  MCP optimization, tool descriptions, MCP best practices, or agent-friendly MCP design.
  Also use when the user has too many tools causing agent confusion, bloated responses
  wasting tokens, or agents picking the wrong tool.
---

# MCP for agents

Developer-oriented MCP servers often fail agents: 1:1 REST-to-tool mappings that force multi-step orchestration, vague descriptions that cause wrong tool selection, nested parameter objects that invite hallucination, and raw API passthrough that exhausts the context window. Design for the agent's constraints, not the developer's convenience.

## Outcomes over operations

The agent decides *when* to call; the server decides *how*. Combine backend operations server-side so the agent makes one call, not three.

**Bad:** Expose `get_user_by_email`, `list_orders`, `get_order_status` separately -- agent chains three calls.
**Good:** Expose `track_latest_order(email)` -- server handles the lookup internally, returns what the agent needs.

A tool that maps 1:1 to a REST endpoint is almost always wrong. Ask: "What outcome does the agent want?" and build the tool around that.

## Flat, constrained parameters

Agents hallucinate missing keys in nested objects. Flatten parameters to top-level primitives, constrain with enums, and add sensible defaults so the agent makes fewer decisions.

**Bad:**
```json
{
  "filters": {
    "status": "string",
    "date_range": { "start": "string", "end": "string" },
    "sort": { "field": "string", "order": "string" }
  }
}
```

**Good:**
```json
{
  "status": { "type": "string", "enum": ["pending", "shipped", "delivered"], "default": "pending" },
  "since_date": { "type": "string", "description": "ISO 8601 date. Defaults to 30 days ago." },
  "sort_by": { "type": "string", "enum": ["date", "total"], "default": "date" },
  "limit": { "type": "integer", "default": 20, "minimum": 1, "maximum": 100 }
}
```

Mark `required` fields explicitly in the schema. Add `description` to every property -- the agent reads these, not your README. Use consistent parameter names across all tools: pick `user_id` or `userId`, never both.

## Descriptions that trigger correctly

The description is the *only* signal the agent uses to pick your tool. A study of 856 tools across 103 servers found 97% of descriptions have quality deficiencies, and 56% have unclear purpose.

Write the description as an answer to: "When should the agent reach for this?"

**Bad:** `"Sends a message"`
**Good:** `"Send a Slack message to a channel or user. Use when the user asks to notify someone, post an update, or communicate via Slack. Requires channel_id or user_id. Messages must be under 4000 characters."`

Cover six components:
1. **Purpose** -- what the tool does, in one sentence
2. **When to use** -- trigger conditions in natural language matching user queries
3. **Limitations** -- what it cannot do, known constraints
4. **Parameters** -- key arguments summarized (detailed descriptions go in the schema)
5. **Completeness** -- detail proportional to complexity
6. **Examples** -- concrete usage demonstrations where helpful

## Actionable errors via isError

MCP has two error mechanisms. Use the right one:

- **Protocol errors** (JSON-RPC `error` object): malformed requests, unknown tool names. The agent cannot self-correct from these.
- **Tool execution errors** (`isError: true` in result content): validation failures, API errors, business logic issues. The agent *can* self-correct from these.

Always return tool execution failures as result content with `isError: true`, not as protocol errors. The error text is an observation the agent uses to retry -- write it as an instruction.

**Bad:** `"Error: 400 Bad Request"`
**Good:** `"User not found for email 'foo@bar.com'. Verify the email is lowercase, or search by user_id with find_user(user_id: '...')"`

Never expose stack traces, SQL errors, or infrastructure details. Distinguish user errors (wrong input -- explain what's valid) from server errors (backend down -- say whether to retry and when).

## Token-efficient responses

Tool schemas are injected into the agent's context on every request. Input schemas alone account for 60-80% of total MCP token usage. Every description, enum value, and property competes for context window space.

**In responses:**
- Return only what the agent needs to complete the task. Do not pass through raw API responses.
- Paginate by default: add `limit` (default 20-50), return `has_more` and `total_count`.
- Prefer plain text over JSON when structure is not needed -- plain text uses ~80% fewer tokens.
- For structured data the agent must parse, use `structuredContent` with `outputSchema`.

**In schemas:**
- Keep tool count low. 5-15 tools per server is the practical ceiling for reliable selection. 30+ tools cause the agent to confuse overlapping descriptions.
- For very large surfaces (40+ tools), consider dynamic toolsets: a `search_tools(query)` discovery tool, a `describe_tool(name)` loader, and an `execute_tool(name, args)` runner. This can reduce input tokens by 90%+.

## Composable outputs

Tool outputs should be directly usable as inputs to other tools without the agent needing to parse prose or guess at field names.

**Bad:** `"Successfully created user John Smith (ID: usr_abc123) in the system."`
**Good:**
```json
{ "user_id": "usr_abc123", "name": "John Smith", "created": true }
```

Use consistent field names across tools. If `create_user` returns `user_id`, then `get_user` and `update_user` accept `user_id` -- not `id`, `userId`, or `user`.

Return IDs, URIs, and status fields the agent can feed directly into the next call. The agent should never need to regex an ID out of a sentence.

## Read/write separation

Clearly distinguish tools that read state from tools that mutate it. This lets agents (and humans reviewing agent actions) understand impact before calling.

- Name reads: `get_*`, `list_*`, `search_*`
- Name writes: `create_*`, `update_*`, `delete_*`
- Use tool annotations when your framework supports them: `readOnlyHint`, `destructiveHint`, `idempotentHint`

Mutations should be idempotent where possible. An agent that retries `update_user_email(user_id, email)` twice should not create a duplicate or error -- it should succeed silently.

## Predictable naming

In multi-server environments, generic names collide. Prefix with the service domain.

**Bad:** `create_issue` -- is this GitHub, Jira, or Linear?
**Good:** `github_create_issue`, `linear_create_issue`

Pick one case style (snake_case or camelCase) and apply it everywhere. Use a consistent verb vocabulary: `get` (single item), `list` (collection), `search` (filtered), `create`, `update`, `delete`.

## When reviewing an existing MCP server

Check: outcome-oriented tools (not 1:1 REST mapping), flat parameters with enums and defaults, descriptions with purpose + trigger conditions + limitations, errors via isError with correction hints, token-efficient responses with pagination, composable structured outputs, consistent naming across tools, read/write separation, tool count under 15, consistent parameter names across the surface.
