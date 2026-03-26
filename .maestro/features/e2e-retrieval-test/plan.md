# agentMemory Retrieval Engine

## Discovery

Explored 15+ existing memory systems (Mem0, Supermemory, Hindsight, etc). Found that none provide workflow-aware retrieval. General memory systems store and retrieve text. agentMemory adds pipeline stage scoring, dependency graph walk, and execution feedback signals that understand the development workflow. Evaluated storage: SQLite vs markdown. Chose markdown with sidecar JSON index.

## Non-Goals

- agentMemory does NOT write memory files (maestro owns writes)
- No cross-project search (per-project index only)
- No LLM dependency for core retrieval

## Ghost Diffs

No existing maestro files modified -- agentMemory is a new separate repo at ~/Code/agentMemory.

## Tasks

### 1. Build scanner and sidecar index
Scanner crawls .maestro/ memory dirs, builds retrieval-index.json with keywords and checksums. Incremental sync via checksum comparison.

### 2. Implement keyword search and query tool
BM25-style keyword scoring with IDF weighting. Wire into memory_query MCP tool with stage and feedback signals.

### 3. Add embeddings and semantic search
ONNX local embeddings via @xenova/transformers. Cosine similarity scoring. Full 6-signal hybrid retrieval with MMR diversity.