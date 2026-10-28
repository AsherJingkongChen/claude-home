"""Experiment: replace every displayed model message with a marker.

Answers two questions: does displayContent change the screen, and does
the replaced text reach the model's context on the next turn.
"""

import json
import sys

__all__ = ["main"]

MARKER = "[replaced by display_test hook]"


def main() -> None:
    payload = json.load(sys.stdin)
    output = {"hookSpecificOutput": {"hookEventName": payload["hook_event_name"], "displayContent": MARKER}}
    print(json.dumps(output))


if __name__ == "__main__":
    main()
