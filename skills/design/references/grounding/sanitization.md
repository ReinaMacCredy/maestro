# Query Sanitization

Removes sensitive information from queries before external sources.

## Overview

Before sending queries to external sources (web_search, read_web_page), sanitize to:
- Remove secrets (API keys, passwords, tokens)
- Remove internal paths that could leak structure
- Log sanitization events (GR-005)

---

## Sanitization Patterns

```python
SENSITIVE_PATTERNS = [
    # API keys and secrets
    (r'API[_-]?KEY[=:]\s*\S+', '[REDACTED:api-key]'),
    (r'SECRET[=:]\s*\S+', '[REDACTED:secret]'),
    (r'PASSWORD[=:]\s*\S+', '[REDACTED:password]'),
    (r'TOKEN[=:]\s*\S+', '[REDACTED:token]'),
    (r'AUTH[=:]\s*\S+', '[REDACTED:auth]'),
    
    # Bearer tokens
    (r'Bearer\s+[A-Za-z0-9\-._~+/]+=*', 'Bearer [REDACTED]'),
    
    # Base64 encoded secrets (long alphanumeric strings)
    (r'[A-Za-z0-9+/]{40,}={0,2}', '[REDACTED:encoded]'),
    
    # AWS keys
    (r'AKIA[0-9A-Z]{16}', '[REDACTED:aws-key]'),
    (r'aws_secret_access_key\s*=\s*\S+', 'aws_secret_access_key=[REDACTED]'),
    
    # Private keys
    (r'-----BEGIN\s+\w+\s+PRIVATE\s+KEY-----.*?-----END\s+\w+\s+PRIVATE\s+KEY-----', 
     '[REDACTED:private-key]'),
    
    # Connection strings
    (r'(mongodb|postgres|mysql|redis)://[^@]+@', r'\1://[REDACTED]@'),
]
```

---

## Sanitization Function

```python
def sanitize_query(query: str) -> SanitizationResult:
    """Remove sensitive content from query before external calls."""
    
    sanitized = query
    redactions = []
    
    for pattern, replacement in SENSITIVE_PATTERNS:
        matches = re.findall(pattern, sanitized, re.I | re.DOTALL)
        if matches:
            redactions.extend(matches)
            sanitized = re.sub(pattern, replacement, sanitized, flags=re.I | re.DOTALL)
    
    return SanitizationResult(
        original_length=len(query),
        sanitized_query=sanitized,
        redaction_count=len(redactions),
        was_modified=len(redactions) > 0
    )
```

---

## Sanitization Result

```python
@dataclass
class SanitizationResult:
    original_length: int
    sanitized_query: str
    redaction_count: int
    was_modified: bool
```

---

## Path Sanitization

Internal paths may leak project structure:

```python
PATH_PATTERNS = [
    # Absolute paths
    (r'/Users/[^/]+/', '/~/'),
    (r'/home/[^/]+/', '/~/'),
    (r'C:\\Users\\[^\\]+\\', 'C:\\~\\'),
    
    # Internal paths (configurable)
    (r'/internal/[^\s]+', '/internal/[PATH]'),
]

def sanitize_paths(query: str) -> str:
    """Anonymize internal paths."""
    result = query
    for pattern, replacement in PATH_PATTERNS:
        result = re.sub(pattern, replacement, result)
    return result
```

---

## When to Sanitize

| Source | Sanitize? | Reason |
|--------|-----------|--------|
| repo | No | Local, trusted |
| web | Yes | External, untrusted |
| history | No | Local threads, already sanitized |

```python
def should_sanitize(source: Source) -> bool:
    return source in [Source.WEB]
```

---

## Logging (GR-005)

When sanitization occurs, log for audit:

```python
def log_sanitization(result: SanitizationResult, source: Source) -> None:
    """Log sanitization event."""
    if result.was_modified:
        log_event({
            "code": "GR-005",
            "message": "Query sanitized",
            "details": {
                "redaction_count": result.redaction_count,
                "source": source.value,
                "original_length": result.original_length,
                "sanitized_length": len(result.sanitized_query)
            }
        })
```

---

## Integration with Router

```python
def query_source(
    source: Source, 
    question: str, 
    timeout: int
) -> SourceResult:
    """Query a source with sanitization for external sources."""
    
    query = question
    
    if should_sanitize(source):
        sanitization = sanitize_query(question)
        query = sanitization.sanitized_query
        
        if sanitization.was_modified:
            log_sanitization(sanitization, source)
    
    return execute_query(source, query, timeout)
```

---

## Error Code

| Code | Message | Action |
|------|---------|--------|
| GR-005 | Query sanitized | Sensitive content removed before external query |

---

## Testing Sanitization

```python
def test_sanitization():
    test_cases = [
        ("API_KEY=abc123", "API_KEY=[REDACTED:api-key]"),
        ("password: secret", "password: [REDACTED:password]"),
        ("Bearer eyJhbGciOiJI...", "Bearer [REDACTED]"),
        ("normal question", "normal question"),  # No change
    ]
    
    for input_query, expected in test_cases:
        result = sanitize_query(input_query)
        assert result.sanitized_query == expected
```

---

## Schema Extension

Sanitization status in result:

```json
{
  "queries": [
    {
      "question": "How to use API_KEY=[REDACTED:api-key]?",
      "source": "web",
      "sanitized": true,
      "redaction_count": 1
    }
  ]
}
```

---

## Related

- [router.md](router.md) - Source routing
- [cache.md](cache.md) - Query caching
- [tiers.md](tiers.md) - Tier definitions
