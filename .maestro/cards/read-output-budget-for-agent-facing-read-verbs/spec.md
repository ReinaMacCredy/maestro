# Read-output budget for agent-facing read verbs

## Current state

Read-verb output sweep, live maestro repo (145 cards), stdout line/byte counts:
- list / card list (no filter) = 146 lines / 28259 bytes -- dumps ALL 145 cards incl. every terminal shipped/closed/verified/rejected/dismissed/measured card, fixed-width layout. No default narrowing to live work.
- list --json = 1456 lines / 43922 bytes -- each card a multi-line pretty-printed object (~8 keys: id/type/title/status/parent/claimed_by/claimed_at/archived).
- decision list / query decisions = 80 lines / 14012 bytes -- every decision across all features, full-width HOME column.
- query matrix = 44 lines / 7649 bytes. status --json = 102 lines / 2788 bytes.
- Already-lean (do it right): ready = 2 lines, task list = 3 lines, task next = 5, feature list = 8, query friction = 9, active = 10, resume = 17, status = 21.
Layout detail (list): fixed-width columns pad title to ~60 cols + HOME to ~60 cols, producing large whitespace runs; bytes >> information.

## Problem

An agent orienting in a repo runs list/status/decision list and re-ingests the entire store every call. Output scales with repo size (worse over the project's life), and terminal cards -- which an agent almost never needs to act on -- dominate the default dump (most of the 145 are shipped/closed/locked). The shipped write-confirm policy (peer card, Tier 1+2) fixed the WRITE side; the peer flagged this read side as Tier 3, a separate read-output-budget feature, explicitly out of their scope. Read verbs differ from write verbs: their job IS to return data, so the fix is not echo-suppression but: bound the default to the live/relevant slice, with full output one explicit flag away.
