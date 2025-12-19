#!/usr/bin/env python3
"""
Semantic memory search for Amp's Letta-style memory system.

Uses LanceDB for vector storage and OpenAI for embeddings.
Falls back to grep if unavailable.

Usage:
    python3 memory_search.py index [--rebuild]
    python3 memory_search.py search "query" [--top-k N]
    python3 memory_search.py add "source_file"
"""

import os
import sys
import re
import json
import hashlib
from datetime import datetime
from pathlib import Path
from typing import Optional

SCRIPT_DIR = Path(__file__).parent
PROJECT_ROOT = SCRIPT_DIR.parent
MEMORY_DIR = PROJECT_ROOT / ".memory"
ARCHIVE_DIR = PROJECT_ROOT / "history" / "memory-archive"
INDEX_DIR = PROJECT_ROOT / ".memory-index"

OLLAMA_URL = "http://localhost:11434"
OLLAMA_EMBED_MODEL = "nomic-embed-text"
EMBEDDING_DIM = 768

DEFAULT_BASE_URL = "http://localhost:8317/v1"
DEFAULT_API_KEY = "proxypal-local"


def get_config() -> dict:
    """Get configuration for chat and embeddings (all local)."""
    return {
        "chat_url": os.environ.get("OPENAI_BASE_URL", DEFAULT_BASE_URL),
        "chat_key": os.environ.get("OPENAI_API_KEY", DEFAULT_API_KEY),
        "ollama_url": os.environ.get("OLLAMA_URL", OLLAMA_URL),
        "embed_model": os.environ.get("OLLAMA_EMBED_MODEL", OLLAMA_EMBED_MODEL),
    }


def chunk_markdown(content: str, source: str, max_chunk_size: int = 500) -> list[dict]:
    """Split markdown content into passages, respecting section boundaries."""
    chunks = []
    lines = content.split("\n")
    
    current_section = ""
    current_chunk = []
    current_size = 0
    
    for line in lines:
        if line.startswith("## "):
            if current_chunk:
                chunk_text = "\n".join(current_chunk).strip()
                if chunk_text:
                    chunks.append({
                        "text": chunk_text,
                        "section": current_section,
                        "source": source
                    })
            current_section = line.replace("## ", "").strip()
            current_chunk = [line]
            current_size = len(line)
        elif line.startswith("# "):
            if current_chunk:
                chunk_text = "\n".join(current_chunk).strip()
                if chunk_text:
                    chunks.append({
                        "text": chunk_text,
                        "section": current_section,
                        "source": source
                    })
            current_section = line.replace("# ", "").strip()
            current_chunk = [line]
            current_size = len(line)
        else:
            if current_size + len(line) > max_chunk_size and current_chunk:
                chunk_text = "\n".join(current_chunk).strip()
                if chunk_text:
                    chunks.append({
                        "text": chunk_text,
                        "section": current_section,
                        "source": source
                    })
                current_chunk = [line]
                current_size = len(line)
            else:
                current_chunk.append(line)
                current_size += len(line) + 1
    
    if current_chunk:
        chunk_text = "\n".join(current_chunk).strip()
        if chunk_text:
            chunks.append({
                "text": chunk_text,
                "section": current_section,
                "source": source
            })
    
    return [c for c in chunks if len(c["text"]) > 20]


def generate_embeddings(texts: list[str], ollama_url: str, model: str) -> list[list[float]]:
    """Generate embeddings using Ollama (local, no API key needed)."""
    import urllib.request
    
    embeddings = []
    try:
        for text in texts:
            data = json.dumps({"model": model, "input": text}).encode()
            req = urllib.request.Request(
                f"{ollama_url}/api/embed",
                data=data,
                headers={"Content-Type": "application/json"}
            )
            with urllib.request.urlopen(req, timeout=30) as resp:
                result = json.loads(resp.read().decode())
                if "embeddings" in result and result["embeddings"]:
                    embeddings.append(result["embeddings"][0])
                else:
                    print(f"Warning: No embedding returned for text", file=sys.stderr)
                    return []
        return embeddings
    except Exception as e:
        print(f"Error generating embeddings: {e}", file=sys.stderr)
        return []


def init_db():
    """Initialize or connect to LanceDB."""
    try:
        import lancedb
        INDEX_DIR.mkdir(parents=True, exist_ok=True)
        db = lancedb.connect(str(INDEX_DIR / "memory.lance"))
        return db
    except Exception as e:
        print(f"Error initializing LanceDB: {e}", file=sys.stderr)
        raise


def get_or_create_table(db, embeddings: list[dict] = None):
    """Get existing table or create new one with embeddings."""
    import pyarrow as pa
    
    schema = pa.schema([
        pa.field("id", pa.string()),
        pa.field("text", pa.string()),
        pa.field("section", pa.string()),
        pa.field("source", pa.string()),
        pa.field("indexed_at", pa.string()),
        pa.field("vector", pa.list_(pa.float32(), EMBEDDING_DIM)),
    ])
    
    try:
        if embeddings:
            if "memory_passages" in db.list_tables().tables:
                table = db.open_table("memory_passages")
                table.add(embeddings)
            else:
                table = db.create_table("memory_passages", embeddings, schema=schema)
        else:
            if "memory_passages" in db.list_tables().tables:
                table = db.open_table("memory_passages")
            else:
                table = db.create_table("memory_passages", schema=schema)
        return table
    except Exception as e:
        print(f"Error with table: {e}", file=sys.stderr)
        return None


def index_file(file_path: Path, config: dict, db) -> int:
    """Index a single file into the vector database."""
    if not file_path.exists():
        return 0
    
    content = file_path.read_text()
    source = str(file_path.relative_to(PROJECT_ROOT))
    
    chunks = chunk_markdown(content, source)
    if not chunks:
        return 0
    
    texts = [c["text"] for c in chunks]
    embeddings = generate_embeddings(texts, config["ollama_url"], config["embed_model"])
    
    if not embeddings or len(embeddings) != len(chunks):
        print(f"Warning: embedding count mismatch for {source}", file=sys.stderr)
        return 0
    
    records = []
    for i, (chunk, embedding) in enumerate(zip(chunks, embeddings)):
        chunk_id = hashlib.md5(f"{source}:{i}:{chunk['text'][:50]}".encode()).hexdigest()
        records.append({
            "id": chunk_id,
            "text": chunk["text"],
            "section": chunk["section"],
            "source": source,
            "indexed_at": datetime.now().isoformat(),
            "vector": embedding
        })
    
    get_or_create_table(db, records)
    return len(records)


def rebuild_index(config: dict) -> dict:
    """Rebuild entire index from .memory/ and history/memory-archive/."""
    try:
        db = init_db()
    except Exception as e:
        return {"success": False, "error": f"Could not initialize LanceDB: {e}"}
    
    if "memory_passages" in db.list_tables().tables:
        db.drop_table("memory_passages")
    
    total_indexed = 0
    files_indexed = []
    
    if MEMORY_DIR.exists():
        for md_file in MEMORY_DIR.glob("*.md"):
            count = index_file(md_file, config, db)
            if count > 0:
                files_indexed.append(str(md_file.relative_to(PROJECT_ROOT)))
                total_indexed += count
    
    if ARCHIVE_DIR.exists():
        for md_file in ARCHIVE_DIR.glob("*.md"):
            count = index_file(md_file, config, db)
            if count > 0:
                files_indexed.append(str(md_file.relative_to(PROJECT_ROOT)))
                total_indexed += count
    
    return {
        "success": True,
        "passages_indexed": total_indexed,
        "files_indexed": files_indexed
    }


def add_to_index(file_path: str, config: dict) -> dict:
    """Add a single file to the index (for incremental updates)."""
    try:
        db = init_db()
    except Exception as e:
        return {"success": False, "error": f"Could not initialize LanceDB: {e}"}
    
    path = Path(file_path)
    if not path.is_absolute():
        path = PROJECT_ROOT / path
    
    count = index_file(path, config, db)
    
    return {
        "success": True,
        "passages_indexed": count,
        "file": str(path.relative_to(PROJECT_ROOT))
    }


def semantic_search(query: str, config: dict, top_k: int = 5) -> list[dict]:
    """Search for passages similar to query."""
    try:
        db = init_db()
    except Exception:
        return []
    
    if "memory_passages" not in db.list_tables().tables:
        return []
    
    query_embedding = generate_embeddings([query], config["ollama_url"], config["embed_model"])
    if not query_embedding:
        return []
    
    table = db.open_table("memory_passages")
    
    try:
        results = table.search(query_embedding[0]).limit(top_k).to_list()
        
        formatted = []
        for r in results:
            formatted.append({
                "text": r["text"],
                "section": r["section"],
                "source": r["source"],
                "score": 1 - r.get("_distance", 0)
            })
        
        return formatted
    except Exception as e:
        print(f"Search error: {e}", file=sys.stderr)
        return []


def grep_fallback(pattern: str) -> list[dict]:
    """Fallback to grep if LanceDB unavailable."""
    import subprocess
    
    results = []
    
    for search_dir in [MEMORY_DIR, ARCHIVE_DIR]:
        if not search_dir.exists():
            continue
        
        try:
            output = subprocess.run(
                ["grep", "-rn", "-i", pattern, str(search_dir)],
                capture_output=True,
                text=True,
                timeout=5
            )
            
            for line in output.stdout.strip().split("\n"):
                if line:
                    parts = line.split(":", 2)
                    if len(parts) >= 3:
                        source = str(Path(parts[0]).relative_to(PROJECT_ROOT))
                        results.append({
                            "source": source,
                            "line": int(parts[1]),
                            "text": parts[2].strip(),
                            "type": "grep"
                        })
        except Exception:
            pass
    
    return results


def format_search_results(results: list[dict], query: str) -> str:
    """Format search results for display."""
    if not results:
        return f"No matches found for: {query}"
    
    output = [f"## Memory Search Results for: {query}\n"]
    
    for i, r in enumerate(results, 1):
        source = r.get("source", "unknown")
        section = r.get("section", "")
        text = r.get("text", "")
        score = r.get("score", 0)
        
        if r.get("type") == "grep":
            line = r.get("line", 0)
            output.append(f"**{i}. {source}:{line}**")
            output.append(f"> {text[:200]}...")
        else:
            output.append(f"**{i}. {source}** (section: {section}, relevance: {score:.2f})")
            output.append(f"> {text[:300]}...")
        output.append("")
    
    return "\n".join(output)


def main():
    if len(sys.argv) < 2:
        print("Usage: memory_search.py [index|search|add|status] [args]", file=sys.stderr)
        sys.exit(1)
    
    command = sys.argv[1]
    config = get_config()
    
    if command == "index":
        result = rebuild_index(config)
        print(json.dumps(result, indent=2))
    
    elif command == "search":
        if len(sys.argv) < 3:
            print("Usage: memory_search.py search 'query' [--top-k N]", file=sys.stderr)
            sys.exit(1)
        
        query = sys.argv[2]
        top_k = 5
        
        if "--top-k" in sys.argv:
            idx = sys.argv.index("--top-k")
            if idx + 1 < len(sys.argv):
                top_k = int(sys.argv[idx + 1])
        
        results = semantic_search(query, config, top_k)
        if results:
            print(format_search_results(results, query))
        else:
            results = grep_fallback(query)
            if results:
                print("(Falling back to grep search)\n")
                print(format_search_results(results, query))
            else:
                print(f"No matches found for: {query}")
    
    elif command == "add":
        if len(sys.argv) < 3:
            print("Usage: memory_search.py add 'file_path'", file=sys.stderr)
            sys.exit(1)
        
        result = add_to_index(sys.argv[2], config)
        print(json.dumps(result, indent=2))
    
    elif command == "status":
        try:
            db = init_db()
            if "memory_passages" in db.list_tables().tables:
                table = db.open_table("memory_passages")
                count = table.count_rows()
                print(json.dumps({
                    "indexed": True,
                    "passage_count": count,
                    "index_path": str(INDEX_DIR / "memory.lance"),
                    "ollama_url": config["ollama_url"],
                    "embed_model": config["embed_model"]
                }, indent=2))
            else:
                print(json.dumps({
                    "indexed": False,
                    "index_path": str(INDEX_DIR / "memory.lance"),
                    "ollama_url": config["ollama_url"],
                    "embed_model": config["embed_model"]
                }, indent=2))
        except Exception as e:
            print(json.dumps({
                "indexed": False,
                "error": str(e),
                "index_path": str(INDEX_DIR / "memory.lance"),
                "ollama_url": config["ollama_url"],
                "embed_model": config["embed_model"]
            }, indent=2))
    
    else:
        print(f"Unknown command: {command}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
