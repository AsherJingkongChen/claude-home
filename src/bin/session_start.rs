//! session_start: placeholder until its behavior is designed; it reads the event and stays silent.

use std::process::ExitCode;

use claude_home::{Output, read_event, send};

fn main() -> ExitCode {
    read_event();
    send(Output::new())
}
