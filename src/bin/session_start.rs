//! session_start: demo of the shared lines while the real behavior is being designed.

use std::process::ExitCode;

use claude_home::vitals::{human_line, model_line, read_vitals};
use claude_home::{header, merge, read_event, send, text, to_human, to_model};
use jiff::Zoned;

fn main() -> ExitCode {
    let event = read_event();
    let now = Zoned::now();
    let vitals = read_vitals(text(&event, "transcript_path"));
    let opening = header(&event, &now);
    send(merge([
        to_model(&event, &format!("{opening}: {}", model_line(&vitals))),
        to_human(&format!("{opening}: {}", human_line(&vitals, &now))),
    ]))
}
