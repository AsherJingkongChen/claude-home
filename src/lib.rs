//! claude-home: hooks for the Claude Code residents of this house.
//!
//! Every hook event gets its own binary in `src/bin/`, named after the event in snake case.
//! A binary knows its event, so it knows who reads each field it returns; event-specific
//! fields (a Stop block, a PreToolUse deny) are written in that binary.
//!
//! This library holds only what every binary shares, as plain functions:
//! [`read_event`] and [`send`] are the only ones that touch stdin or stdout, and the rest
//! build output pieces that [`merge`] joins into the one JSON object Claude Code accepts.
//!
//! | piece        | field                                  | reader                       |
//! |--------------|----------------------------------------|------------------------------|
//! | [`to_model`] | `hookSpecificOutput.additionalContext` | the model, on its next call  |
//! | [`to_human`] | `systemMessage`                        | the person at the terminal   |
//! | none         | nothing on stdout                      | nobody                       |
//!
//! Lines in either direction open with [`header`]; [`vitals`] writes what follows it.

pub mod vitals;

use std::io::Read;
use std::path::Path;
use std::process::ExitCode;

use jiff::Zoned;
use serde_json::{Map, Value, json};

/// One JSON object of hook output, or a piece of one.
pub type Output = Map<String, Value>;

/// One hook call, as Claude Code sent it on stdin.
pub struct Event {
    pub name: String,
    pub input: Value,
}

/// Read the event from stdin. Panics with a message when stdin is not a JSON object with
/// `hook_event_name`, which Claude Code reports as a non-blocking error.
pub fn read_event() -> Event {
    let mut raw = String::new();
    std::io::stdin()
        .read_to_string(&mut raw)
        .expect("stdin should be readable");
    let input: Value = serde_json::from_str(&raw).expect("stdin should be one JSON object");
    let name = input["hook_event_name"]
        .as_str()
        .expect("the event should carry hook_event_name")
        .to_owned();
    Event { name, input }
}

/// A string field of the event input, or "" when it is missing.
pub fn text<'a>(event: &'a Event, key: &str) -> &'a str {
    event.input[key].as_str().unwrap_or("")
}

/// The shared opening of every line: `[Stop]@claude-home-bf6f8f (10-29 01:50:09 +08)`.
/// The project is where the session started (`CLAUDE_PROJECT_DIR`), so a `cd` does not move it.
pub fn header(event: &Event, now: &Zoned) -> String {
    let root =
        std::env::var("CLAUDE_PROJECT_DIR").unwrap_or_else(|_| text(event, "cwd").to_owned());
    let project = Path::new(&root)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let id: String = text(event, "session_id").chars().take(6).collect();
    let offset = now.offset().seconds();
    let zone = if offset % 3600 == 0 {
        format!("{:+03}", offset / 3600)
    } else {
        now.strftime("%:z").to_string()
    };
    format!(
        "[{}]@{project}-{id} ({} {zone})",
        event.name,
        now.strftime("%m-%d %H:%M:%S")
    )
}

/// Context for the model, attached to its next call.
pub fn to_model(event: &Event, text: &str) -> Output {
    let piece =
        json!({"hookSpecificOutput": {"hookEventName": event.name, "additionalContext": text}});
    piece.as_object().cloned().unwrap_or_default()
}

/// A line for the person at the terminal; the model does not see it.
pub fn to_human(text: &str) -> Output {
    Output::from_iter([("systemMessage".to_owned(), Value::from(text))])
}

/// Join output pieces into one object. A later piece wins on a repeated key.
pub fn merge(pieces: impl IntoIterator<Item = Output>) -> Output {
    pieces.into_iter().flatten().collect()
}

/// Print the output, if any, and return the exit code to end the process with.
pub fn send(output: Output) -> ExitCode {
    if !output.is_empty() {
        println!("{}", Value::Object(output));
    }
    ExitCode::SUCCESS
}
