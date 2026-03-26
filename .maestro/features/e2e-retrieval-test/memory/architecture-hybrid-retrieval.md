---
tags: [retrieval, hybrid, signals, architecture, mmr]
priority: 0
category: architecture
connections: [decision-use-markdown-storage:extends]
---
The retrieval engine uses 6 weighted signals: semantic (0.25), keyword BM25 (0.15), pipeline stage proximity (0.20), dependency graph walk (0.20), execution feedback (0.15), and recency (0.05). Workflow signals dominate at 0.55 combined -- this is the core differentiator vs general memory systems. MMR diversity selection prevents redundant memories in the token budget.