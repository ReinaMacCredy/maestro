#!/usr/bin/env python3
"""Session identity generation and parsing for orchestrator sessions.

Generates session IDs in format: {BaseAgent}-{unix_timestamp}
Display format: {BaseAgent} (session HH:MM)
"""

import argparse
import json
import sys
import time
from datetime import datetime


def generate_session_id(base_agent: str) -> str:
    """Generate a session ID with current timestamp.
    
    Args:
        base_agent: The base agent name (e.g., "BlueLake")
        
    Returns:
        Session ID in format "{BaseAgent}-{unix_timestamp}"
    """
    timestamp = int(time.time())
    return f"{base_agent}-{timestamp}"


def format_display_name(session_id: str) -> str:
    """Format a session ID for human-readable display.
    
    Args:
        session_id: Session ID in format "{BaseAgent}-{timestamp}"
        
    Returns:
        Display string in format "{BaseAgent} (session HH:MM)"
    """
    parsed = parse_session_id(session_id)
    return parsed["display"]


def parse_session_id(session_id: str) -> dict:
    """Parse a session ID into its components.
    
    Args:
        session_id: Session ID in format "{BaseAgent}-{timestamp}"
        
    Returns:
        Dictionary with keys: base_agent, timestamp, display
        
    Raises:
        ValueError: If session_id format is invalid
    """
    if "-" not in session_id:
        raise ValueError(f"Invalid session ID format: {session_id}")
    
    # Split from the right to handle agent names with hyphens
    parts = session_id.rsplit("-", 1)
    if len(parts) != 2:
        raise ValueError(f"Invalid session ID format: {session_id}")
    
    base_agent, ts_str = parts
    
    try:
        timestamp = int(ts_str)
    except ValueError:
        raise ValueError(f"Invalid timestamp in session ID: {ts_str}")
    
    # Format display time
    dt = datetime.fromtimestamp(timestamp)
    time_str = dt.strftime("%H:%M")
    display = f"{base_agent} (session {time_str})"
    
    return {
        "base_agent": base_agent,
        "timestamp": timestamp,
        "display": display
    }


def cmd_generate(args) -> dict:
    """Handle 'generate' subcommand."""
    session_id = generate_session_id(args.base_agent)
    return {
        "session_id": session_id,
        "base_agent": args.base_agent,
        "timestamp": int(time.time()),
        "display": format_display_name(session_id)
    }


def cmd_format(args) -> dict:
    """Handle 'format' subcommand."""
    display = format_display_name(args.session_id)
    return {
        "session_id": args.session_id,
        "display": display
    }


def cmd_parse(args) -> dict:
    """Handle 'parse' subcommand."""
    return parse_session_id(args.session_id)


def main():
    parser = argparse.ArgumentParser(
        description="Session identity generation and parsing"
    )
    subparsers = parser.add_subparsers(dest="command", required=True)
    
    # generate subcommand
    gen_parser = subparsers.add_parser(
        "generate",
        help="Generate a new session ID"
    )
    gen_parser.add_argument(
        "base_agent",
        help="Base agent name (e.g., BlueLake)"
    )
    gen_parser.set_defaults(func=cmd_generate)
    
    # format subcommand
    fmt_parser = subparsers.add_parser(
        "format",
        help="Format a session ID for display"
    )
    fmt_parser.add_argument(
        "session_id",
        help="Session ID to format"
    )
    fmt_parser.set_defaults(func=cmd_format)
    
    # parse subcommand
    parse_parser = subparsers.add_parser(
        "parse",
        help="Parse a session ID into components"
    )
    parse_parser.add_argument(
        "session_id",
        help="Session ID to parse"
    )
    parse_parser.set_defaults(func=cmd_parse)
    
    args = parser.parse_args()
    
    try:
        result = args.func(args)
        print(json.dumps(result, indent=2))
        return 0
    except ValueError as e:
        error = {"error": str(e)}
        print(json.dumps(error, indent=2), file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
