//! session_end: tells the person how the session went and why it ended.
//!
//! The model never reads this event's output, because the session is over; the line goes
//! to the terminal only.

use std::process::ExitCode;

use claude_home::{read_moment, send, text, to_human};

/// The `reason` values Claude Code documents for SessionEnd, as a sentence each.
fn label(reason: &str) -> &'static str {
    match reason {
        "clear" => "Cleared manually.",
        "resume" => "Left temporarily...",
        "logout" => "Logged out.",
        "prompt_input_exit" => "Exited manually.",
        _ => "Ended.",
    }
}

fn main() -> ExitCode {
    let moment = read_moment();
    let reason = label(text(&moment.event, "reason"));
    send(to_human(&moment, reason))
}
