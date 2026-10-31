# claude-home

> Claude's home.

**Features:** Claude Code hooks (one binary per event).

## Install

```bash
cargo install --path . --root ~/.claude/hooks
```

Plus, copy `"hooks"` from `.claude/settings.json` into `~/.claude/settings.json`.

## Uninstall

```bash
cargo uninstall --root ~/.claude/hooks claude-home
rm -r ~/.claude/projects/*/transcript/latest/
```

Also, remove those `"hooks"` entries from `~/.claude/settings.json`.

