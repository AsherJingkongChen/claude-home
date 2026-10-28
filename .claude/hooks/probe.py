"""Record every hook call verbatim, so we can see what a hook receives.

The probe never changes Claude Code's behavior: it prints nothing and
always exits 0. Each call appends one line to two files under log/ at
the repository root, in the same order:

- stdin.jsonl: the event exactly as Claude Code sent it on stdin.
- probe.jsonl: what the probe observed around it (time, argv, cwd, env).
"""

from datetime import datetime, timezone
from pathlib import Path
import json
import os
import sys

__all__ = ["main"]

LOG_DIR = Path(__file__).resolve().parents[2] / "log"
SECRET_MARKERS = ("KEY", "SECRET", "TOKEN")
STRING_LIMIT = 20_000


def redact(key: str, value: str) -> str:
    if any(marker in key for marker in SECRET_MARKERS):
        return f"[redacted {len(value)} chars]"
    return value


def clip(value: object) -> object:
    if isinstance(value, str) and len(value) > STRING_LIMIT:
        return value[:STRING_LIMIT] + f"... [clipped {len(value) - STRING_LIMIT} chars]"
    if isinstance(value, dict):
        return {key: clip(item) for key, item in value.items()}
    if isinstance(value, list):
        return [clip(item) for item in value]
    return value


def main() -> None:
    raw = sys.stdin.read()
    record = {
        "time": datetime.now(timezone.utc).astimezone().isoformat(),
        "argv": sys.argv,
        "cwd": os.getcwd(),
        "env": {key: redact(key, value) for key, value in sorted(os.environ.items()) if key.startswith("CLAUDE")},
        "stdin_bytes": len(raw.encode()),
    }
    LOG_DIR.mkdir(parents=True, exist_ok=True)
    try:
        event = json.loads(raw)
        with (LOG_DIR / "stdin.jsonl").open("a") as log:
            log.write(json.dumps(event, ensure_ascii=False) + "\n")
        record["event"] = event.get("hook_event_name")
    except json.JSONDecodeError as error:
        record["input_error"] = str(error)
        record["input_raw"] = clip(raw)
    with (LOG_DIR / "probe.jsonl").open("a") as log:
        log.write(json.dumps(record, ensure_ascii=False) + "\n")


if __name__ == "__main__":
    try:
        main()
    finally:
        sys.exit(0)
