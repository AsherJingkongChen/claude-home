# claude-home

Claude Code hooks, one binary per event.

## Install

```
cargo install --path . --root ~/.claude/hooks
```

Copy `"hooks"` from `.claude/settings.json` into `~/.claude/settings.json`.

## Uninstall

```
cargo uninstall --root ~/.claude/hooks claude-home
rm -r ~/.claude/hooks/cache
```

Remove those `"hooks"` entries from `~/.claude/settings.json`.
