//! session_end: tells the person how the session went and why it ended.
//!
//! The model never reads this event's output, because the session is over; the line goes
//! to the terminal only.

use std::process::ExitCode;

use claude_home::vitals::{human_line, read_vitals};
use claude_home::{header, read_event, send, text, to_human};
use jiff::Zoned;

/// The `reason` values Claude Code documents for SessionEnd, as a sentence each.
fn label(reason: &str) -> String {
    match reason {
        "clear" => "Cleared by the user.".to_owned(),
        "resume" => "Left for another session.".to_owned(),
        "logout" => "Logged out.".to_owned(),
        "prompt_input_exit" => "Exited at the prompt.".to_owned(),
        "other" => "Ended for a reason Claude Code did not name.".to_owned(),
        unknown => format!("Ended: {unknown}."),
    }
}

fn main() -> ExitCode {
    let event = read_event();
    let now = Zoned::now();
    let vitals = read_vitals(text(&event, "session_id"), text(&event, "transcript_path"));
    send(to_human(&format!(
        "{}: {}\n{}",
        header(&event, &now),
        human_line(&vitals, &now),
        label(text(&event, "reason")),
    )))
}
