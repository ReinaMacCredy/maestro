---
tags: [storage, architecture, markdown, retrieval]
priority: 1
category: decision
---
Decided to use markdown files with sidecar JSON index instead of SQLite for the agentMemory retrieval engine. Reasons: human-readable, git-friendly, aligns with maestro's existing .maestro/ file structure. The sidecar index stores pre-computed embeddings and keyword tokens for fast retrieval.