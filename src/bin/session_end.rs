//! session_end: tells the person how the session went and why it ended.
//!
//! The model never reads this event's output, because the session is over. Claude Code also
//! discards SessionEnd's JSON output, so the message is written straight to a terminal:
//! the hook's own (/dev/tty), else the one Claude Code runs in (`CLAUDE_PID`'s stdout, when it
//! is a /dev/pts device). With neither, as under remote control, it falls back to stderr with
//! exit code 2, the one path Claude Code shows; SessionEnd cannot be blocked, so the 2 blocks
//! nothing.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use claude_home::{for_human, read_moment, text};

/// The `reason` values Claude Code documents for SessionEnd, as a sentence each.
fn explain(reason: &str) -> &'static str {
    match reason {
        "clear" => "Cleared manually.",
        "resume" => "",
        "logout" => "Logged out.",
        "prompt_input_exit" => "Exited manually.",
        _ => "Ended.",
    }
}

fn main() -> ExitCode {
    let moment = read_moment();
    let reason = explain(text(&moment.event, "reason"));
    if reason.is_empty() {
        return ExitCode::SUCCESS;
    }
    let message = for_human(&moment, reason);
    let written = terminals()
        .into_iter()
        .any(|path| write_line(&path, &message));
    if written {
        ExitCode::SUCCESS
    } else {
        eprintln!("\n{message}");
        ExitCode::from(2)
    }
}

/// Terminals to try, in order: the hook's own, then the one Claude Code writes to.
fn terminals() -> Vec<PathBuf> {
    let claude_stdout = std::env::var("CLAUDE_PID")
        .ok()
        .and_then(|pid| std::fs::read_link(format!("/proc/{pid}/fd/1")).ok())
        .filter(|path| path.starts_with("/dev/pts"));
    [Some(PathBuf::from("/dev/tty")), claude_stdout]
        .into_iter()
        .flatten()
        .collect()
}

fn write_line(path: &PathBuf, line: &str) -> bool {
    OpenOptions::new()
        .write(true)
        .open(path)
        .and_then(|mut terminal| writeln!(terminal, "{line}"))
        .is_ok()
}
