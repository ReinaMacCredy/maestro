#!/usr/bin/env python3
"""Unit tests for Session Brain scripts.

Tests session_identity.py, preflight.py, and session_cleanup.py.
"""

import json
import sys
from pathlib import Path
import pytest
from datetime import datetime, timezone, timedelta

# Add scripts directory to path for imports
sys.path.insert(0, str(Path(__file__).parent))

from session_identity import (
    generate_session_id,
    format_display_name,
    parse_session_id,
)
from preflight import (
    detect_sessions,
    check_conflicts,
)
from session_cleanup import (
    find_stale_sessions,
    format_takeover_prompt,
)


# ============================================================================
# session_identity.py tests
# ============================================================================

class TestGenerateSessionId:
    """Tests for generate_session_id()."""
    
    def test_generate_session_id(self):
        """Verify format {BaseAgent}-{timestamp}."""
        result = generate_session_id("BlueLake")
        
        # Should have exactly one hyphen before the timestamp
        parts = result.rsplit("-", 1)
        assert len(parts) == 2
        assert parts[0] == "BlueLake"
        
        # Timestamp should be a valid integer
        timestamp = int(parts[1])
        assert timestamp > 0
        
        # Timestamp should be recent (within 5 seconds)
        import time
        now = int(time.time())
        assert abs(now - timestamp) < 5


class TestFormatDisplayName:
    """Tests for format_display_name()."""
    
    def test_format_display_name(self):
        """Verify format {BaseAgent} (session HH:MM)."""
        # Use a known timestamp: 2025-01-01 14:30:00 UTC
        session_id = "BlueLake-1735739400"
        result = format_display_name(session_id)
        
        # Should contain base agent and session format
        assert "BlueLake" in result
        assert "(session" in result
        assert ")" in result
        
        # Should match pattern: "AgentName (session HH:MM)"
        import re
        pattern = r"^BlueLake \(session \d{2}:\d{2}\)$"
        assert re.match(pattern, result)


class TestParseSessionId:
    """Tests for parse_session_id()."""
    
    def test_parse_session_id(self):
        """Verify dict with base_agent, timestamp, display."""
        session_id = "BlueLake-1735689600"
        result = parse_session_id(session_id)
        
        assert isinstance(result, dict)
        assert result["base_agent"] == "BlueLake"
        assert result["timestamp"] == 1735689600
        assert "display" in result
        assert "BlueLake" in result["display"]
        assert "(session" in result["display"]
    
    def test_parse_session_id_with_hyphenated_agent(self):
        """Handle Blue-Lake-1735689600 (agent name with hyphens)."""
        session_id = "Blue-Lake-1735689600"
        result = parse_session_id(session_id)
        
        assert result["base_agent"] == "Blue-Lake"
        assert result["timestamp"] == 1735689600
        assert "Blue-Lake" in result["display"]
    
    def test_parse_session_id_invalid_format(self):
        """Should raise ValueError for invalid format."""
        with pytest.raises(ValueError):
            parse_session_id("InvalidNoHyphen")
    
    def test_parse_session_id_invalid_timestamp(self):
        """Should raise ValueError for non-numeric timestamp."""
        with pytest.raises(ValueError):
            parse_session_id("BlueLake-notanumber")


# ============================================================================
# preflight.py tests
# ============================================================================

class TestDetectSessions:
    """Tests for detect_sessions()."""
    
    def test_detect_sessions_empty(self):
        """Empty inbox returns empty list."""
        result = detect_sessions([])
        assert result == []
    
    def test_detect_sessions_with_start(self):
        """Parses [SESSION START] messages correctly."""
        inbox = [
            {
                "subject": "[SESSION START] BlueLake starting work",
                "from": "BlueLake",
                "created_ts": "2025-01-01T10:00:00Z",
                "body_md": "session_id: BlueLake-1735689600\ntrack: session-brain\nbeads_claimed: bd-101, bd-102"
            }
        ]
        result = detect_sessions(inbox)
        
        assert len(result) == 1
        session = result[0]
        assert session["agent"] == "BlueLake"
        assert session["status"] == "active"
        assert session["track"] == "session-brain"
        assert session["beads_claimed"] == ["bd-101", "bd-102"]
        assert session["started_at"] == "2025-01-01T10:00:00Z"
    
    def test_detect_sessions_with_heartbeat(self):
        """Updates last_seen from heartbeat."""
        inbox = [
            {
                "subject": "[SESSION START] BlueLake starting",
                "from": "BlueLake",
                "created_ts": "2025-01-01T10:00:00Z",
                "body_md": "session_id: BlueLake-1735689600\ntrack: session-brain"
            },
            {
                "subject": "[HEARTBEAT] BlueLake still active",
                "from": "BlueLake",
                "created_ts": "2025-01-01T10:05:00Z",
                "body_md": "session_id: BlueLake-1735689600"
            }
        ]
        result = detect_sessions(inbox)
        
        assert len(result) == 1
        session = result[0]
        # last_heartbeat should be updated to the heartbeat time
        assert session["last_heartbeat"] == "2025-01-01T10:05:00Z"
    
    def test_detect_sessions_with_end(self):
        """Marks session as ended (not returned in active list)."""
        inbox = [
            {
                "subject": "[SESSION START] BlueLake starting",
                "from": "BlueLake",
                "created_ts": "2025-01-01T10:00:00Z",
                "body_md": "session_id: BlueLake-1735689600"
            },
            {
                "subject": "[SESSION END] BlueLake finished",
                "from": "BlueLake",
                "created_ts": "2025-01-01T11:00:00Z",
                "body_md": "session_id: BlueLake-1735689600"
            }
        ]
        result = detect_sessions(inbox)
        
        # Ended sessions should not appear in active list
        assert len(result) == 0


class TestCheckConflicts:
    """Tests for check_conflicts()."""
    
    def test_check_conflicts_no_conflict(self):
        """Different track returns no conflicts."""
        my_session = {
            "track": "my-track",
            "beads_claimed": ["bd-301"],
            "files_reserved": ["docs/**"]
        }
        active_sessions = [
            {
                "session_id": "GreenCastle-1735689600",
                "track": "other-track",
                "beads_claimed": ["bd-101"],
                "files_reserved": ["src/**"]
            }
        ]
        result = check_conflicts(my_session, active_sessions)
        
        assert result["has_conflicts"] is False
        assert result["track_conflicts"] == []
        assert result["file_conflicts"] == []
        assert result["bead_conflicts"] == []
    
    def test_check_conflicts_track_conflict(self):
        """Same track returns conflict."""
        my_session = {
            "track": "session-brain",
            "beads_claimed": [],
            "files_reserved": []
        }
        active_sessions = [
            {
                "session_id": "GreenCastle-1735689600",
                "track": "session-brain",
                "beads_claimed": [],
                "files_reserved": []
            }
        ]
        result = check_conflicts(my_session, active_sessions)
        
        assert result["has_conflicts"] is True
        assert len(result["track_conflicts"]) == 1
        assert result["track_conflicts"][0]["track"] == "session-brain"
        assert result["track_conflicts"][0]["session_id"] == "GreenCastle-1735689600"
    
    def test_check_conflicts_file_overlap(self):
        """Overlapping globs detected."""
        my_session = {
            "track": "my-track",
            "beads_claimed": [],
            "files_reserved": ["src/api/**"]
        }
        active_sessions = [
            {
                "session_id": "GreenCastle-1735689600",
                "track": "other-track",
                "beads_claimed": [],
                "files_reserved": ["src/**"]
            }
        ]
        result = check_conflicts(my_session, active_sessions)
        
        assert result["has_conflicts"] is True
        assert len(result["file_conflicts"]) == 1
        assert result["file_conflicts"][0]["pattern"] == "src/**"
        assert result["file_conflicts"][0]["overlap"] == "src/api/**"
    
    def test_check_conflicts_bead_conflict(self):
        """Same bead claimed by both sessions."""
        my_session = {
            "track": "my-track",
            "beads_claimed": ["bd-101", "bd-102"],
            "files_reserved": []
        }
        active_sessions = [
            {
                "session_id": "GreenCastle-1735689600",
                "track": "other-track",
                "beads_claimed": ["bd-101"],
                "files_reserved": []
            }
        ]
        result = check_conflicts(my_session, active_sessions)
        
        assert result["has_conflicts"] is True
        assert len(result["bead_conflicts"]) == 1
        assert result["bead_conflicts"][0]["bead_id"] == "bd-101"


# ============================================================================
# session_cleanup.py tests
# ============================================================================

class TestFindStaleSessions:
    """Tests for find_stale_sessions()."""
    
    def test_find_stale_sessions(self):
        """Sessions >10 min since last_seen are stale."""
        # Create a timestamp 15 minutes ago
        old_time = (datetime.now(timezone.utc) - timedelta(minutes=15)).isoformat()
        
        sessions = [
            {
                "session_id": "BlueLake-1735689600",
                "agent": "BlueLake",
                "track": "session-brain",
                "last_heartbeat": old_time,
                "beads_claimed": ["bd-101"]
            }
        ]
        result = find_stale_sessions(sessions, threshold_min=10)
        
        assert len(result) == 1
        assert result[0]["stale_reason"] == "heartbeat_timeout"
        assert result[0]["stale_minutes"] >= 14  # Allow small timing variance
    
    def test_find_stale_sessions_none_stale(self):
        """Recent sessions not marked stale."""
        # Create a timestamp 2 minutes ago
        recent_time = (datetime.now(timezone.utc) - timedelta(minutes=2)).isoformat()
        
        sessions = [
            {
                "session_id": "BlueLake-1735689600",
                "agent": "BlueLake",
                "track": "session-brain",
                "last_heartbeat": recent_time,
                "beads_claimed": ["bd-101"]
            }
        ]
        result = find_stale_sessions(sessions, threshold_min=10)
        
        assert len(result) == 0
    
    def test_find_stale_sessions_no_heartbeat(self):
        """Sessions without heartbeat are marked stale."""
        sessions = [
            {
                "session_id": "BlueLake-1735689600",
                "agent": "BlueLake",
                "track": "session-brain",
                "last_heartbeat": None,
                "beads_claimed": []
            }
        ]
        result = find_stale_sessions(sessions, threshold_min=10)
        
        assert len(result) == 1
        assert result[0]["stale_reason"] == "no_heartbeat"


class TestFormatTakeoverPrompt:
    """Tests for format_takeover_prompt()."""
    
    def test_format_takeover_prompt(self):
        """Contains [T]ake, [W]ait, [I]gnore options."""
        session = {
            "session_id": "BlueLake-1735689600",
            "agent": "BlueLake",
            "track": "session-brain",
            "stale_minutes": 15,
            "beads_claimed": ["bd-101", "bd-102"]
        }
        result = format_takeover_prompt(session)
        
        # Check for required options
        assert "[T]" in result
        assert "[W]" in result
        assert "[I]" in result
        assert "Take over" in result
        assert "Wait" in result
        assert "Ignore" in result
        
        # Check for session info
        assert "BlueLake" in result
        assert "session-brain" in result
        assert "STALE" in result
        assert "15m" in result


# ============================================================================
# Run tests
# ============================================================================

if __name__ == "__main__":
    pytest.main([__file__, "-v"])
