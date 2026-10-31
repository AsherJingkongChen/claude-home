# claude-home

Hooks for the Claude Code residents of this house, one binary per hook event.
Each message carries the same header, `claude-home-bf6f8f2a (10-29 01:50:09 +08):`, so the model and the person both know when it was said.

## Install

```
git clone <repo-url> claude-home && cd claude-home
cargo install --path . --root ~/.claude/hooks
```

The binaries land in `~/.claude/hooks/bin/`.
Then copy the `hooks` object from this repo's `.claude/settings.json` into `~/.claude/settings.json`.
A running session picks it up after its ConfigChange event; there is no need to restart.

To update, pull and run the same `cargo install`.
To uninstall, remove those entries from `~/.claude/settings.json` and run `cargo uninstall --root ~/.claude/hooks claude-home`.

## Develop

`.claude/settings.json` is both this repo's own hook setup and the form to copy, so a change is tested by installing it.
The probe that logs every raw event to `log/stdin.jsonl` is registered in `.claude/settings.local.json`, which stays out of git.
