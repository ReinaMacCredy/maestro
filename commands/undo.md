# Undo Last Operation

I'll help you rollback the last destructive operation performed by CCPlugins commands.

First, let me check for available backups and recent operations:

```bash
# Check for CCPlugins backup directory
BACKUP_DIR="$HOME/.claude/.ccplugins_backups"
if [ -d "$BACKUP_DIR" ]; then
    echo "Found backup directory. Recent backups:"
    ls -la "$BACKUP_DIR" | tail -10
else
    echo "No backup directory found."
fi

# Check git status for recent changes
if git rev-parse --git-dir > /dev/null 2>&1; then
    echo -e "\nGit status:"
    git status --short
fi
```

Based on what I find, I can:

1. **Restore from CCPlugins backup** - If a backup exists from /cleanproject or other commands
2. **Use git to restore** - If changes haven't been committed yet
3. **Identify what was changed** - Show you what was modified so you can decide

I'll analyze the situation and suggest the safest recovery method.

If multiple restore options exist, I'll:
- Show you what each option would restore
- Explain the implications
- Let you choose the best approach

This ensures you can confidently undo operations without losing important work.