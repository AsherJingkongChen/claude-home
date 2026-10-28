//! session_end: tells the person how the session went and why it ended.
//!
//! The model never reads this event's output, because the session is over. Claude Code also
//! discards SessionEnd's JSON output, so the message is written straight to the terminal
//! (/dev/tty). Without a terminal (remote control) it falls back to stderr with exit code 2, the
//! one path it shows the person; SessionEnd cannot be blocked, so the 2 blocks nothing.

use std::fs::OpenOptions;
use std::io::Write;
use std::process::ExitCode;

use claude_home::{human_message, read_moment, text};

/// The `reason` values Claude Code documents for SessionEnd, as a sentence each.
fn translate(reason: &str) -> &'static str {
    match reason {
        "clear" => "Cleared manually.",
        "resume" => "Left temporarily.",
        "logout" => "Logged out.",
        "prompt_input_exit" => "Exited manually.",
        _ => "Ended.",
    }
}

fn main() -> ExitCode {
    let moment = read_moment();
    let reason = translate(text(&moment.event, "reason"));
    let message = human_message(&moment, reason);
    let written = OpenOptions::new()
        .write(true)
        .open("/dev/tty")
        .and_then(|mut terminal| writeln!(terminal, "{message}"));
    match written {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => {
            eprintln!("{message}");
            ExitCode::from(2)
        }
    }
}
