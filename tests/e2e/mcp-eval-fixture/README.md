# MCP evaluation fixture

A frozen `.maestro/` project state used by `docs/mcp-evaluation.xml`. The
state is hand-curated so every answer in the evaluation can be derived
deterministically through MCP tool calls.

## Running the evaluation

Point the MCP server at this directory:

```bash
maestro mcp serve --project-root tests/e2e/mcp-eval-fixture
```

or via env var:

```bash
MAESTRO_PROJECT_ROOT=tests/e2e/mcp-eval-fixture maestro mcp serve
```

Then run the evaluator from `.claude/skills/mcp-builder/`:

```bash
python .claude/skills/mcp-builder/scripts/evaluation.py \
  -t stdio \
  -c maestro \
  -a mcp serve \
  -e MAESTRO_PROJECT_ROOT=$(pwd)/tests/e2e/mcp-eval-fixture \
  -o eval-report.md \
  docs/mcp-evaluation.xml
```

## Fixture contents

- 6 tasks across 2 missions plus 2 unmissioned tasks
- 3 versioned contracts (`tsk-aa0001` has 2 versions; `tsk-bb0001` and `tsk-cc0001` have 1 each)
- 10 evidence rows across 4 tasks, covering 7 kinds and 3 witness levels
- 3 verdicts (decisions: PASS, HUMAN, PASS)

The fixture is intentionally small. It is large enough to require multi-step
queries but small enough to audit by hand.

## Mutating the fixture

Any change must keep all 10 evaluation answers correct. Re-run the
evaluation script (or the inline smoke flow at the end of this README) and
update `docs/mcp-evaluation.xml` if an answer shifts.
