//! session_start: placeholder until its behavior is designed; it reads the event and stays silent.

use std::process::ExitCode;

use claude_home::{Event, Reply};

fn main() -> ExitCode {
    let event = Event::read();
    Reply::default().send(&event)
}
