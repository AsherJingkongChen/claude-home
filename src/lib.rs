//! claude-home: hooks for the Claude Code residents of this house.
//!
//! Every hook event gets its own binary in `src/bin/`, named after the event in snake case.
//! A binary knows its event, so it knows who reads each field it returns; event-specific
//! fields (a Stop block, a PreToolUse deny) are written in that binary.
//!
//! This library is the station every binary shares. [`read_moment`] and [`send`] are its
//! two ends, the only functions that touch the outside world; everything between them is a
//! plain function from values to values.
//!
//! Output goes four ways. Every piece is placed by [`top`] or [`specific`], and [`merge`]
//! joins the pieces into the one object Claude Code reads.
//!
//! | direction | piece                         | field                                       |
//! |-----------|-------------------------------|---------------------------------------------|
//! | person    | [`to_human`]                  | `systemMessage`                             |
//! | model     | [`to_model`]                  | `hookSpecificOutput.additionalContext`      |
//! | system    | [`top`], [`specific`]         | `decision`, `continue`, `watchPaths`, ...   |
//! | air       | none                          | nothing on stdout                           |
//!
//! [`for_human`] and [`for_model`] write the text: the same [`header`], a line from
//! [`vitals`] for that reader, then the binary's own sentence. Output that is not JSON
//! (exit code 2, a terminal write at SessionEnd) stays in the binary that needs it.

pub mod vitals;

use std::io::Read;
use std::path::Path;
use std::process::ExitCode;

use jiff::Zoned;
use serde_json::{Map, Value};

use vitals::{Vitals, human_line, model_line, read_vitals};

/// One JSON object of hook output, or a piece of one.
pub type Output = Map<String, Value>;

/// One hook call, as Claude Code sent it on stdin.
pub struct Event {
    pub name: String,
    pub input: Value,
}

/// Everything a hook reads from outside, gathered once: the event, the clock, the vitals.
pub struct Moment {
    pub event: Event,
    pub now: Zoned,
    pub vitals: Vitals,
}

/// Read the event from stdin, the clock, and the session's vitals. Panics with a message
/// when stdin is not a JSON object with `hook_event_name`, which Claude Code reports as a
/// non-blocking error.
pub fn read_moment() -> Moment {
    let mut raw = String::new();
    std::io::stdin()
        .read_to_string(&mut raw)
        .expect("stdin should be readable");
    let input: Value = serde_json::from_str(&raw).expect("stdin should be one JSON object");
    let name = input["hook_event_name"]
        .as_str()
        .expect("the event should carry hook_event_name");
    let event = Event {
        name: name.into(),
        input,
    };
    let vitals = read_vitals(text(&event, "session_id"), text(&event, "transcript_path"));
    Moment {
        event,
        now: Zoned::now(),
        vitals,
    }
}

/// A string field of the event input, or "" when it is missing.
pub fn text<'a>(event: &'a Event, key: &str) -> &'a str {
    event.input[key].as_str().unwrap_or("")
}

/// The shared opening of every message: `claude-home-bf6f8f2a (10-29 01:50:09 +08):`.
/// The project is where the session started (`CLAUDE_PROJECT_DIR`), so a `cd` does not move it.
/// The event name stays out: Claude Code already prefixes what the model reads with it.
pub fn header(moment: &Moment) -> String {
    let event = &moment.event;
    let root = std::env::var("CLAUDE_PROJECT_DIR").unwrap_or_else(|_| text(event, "cwd").into());
    let project = Path::new(&root)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let id: String = text(event, "session_id").chars().take(8).collect();
    let offset = moment.now.offset().seconds();
    let zone = match offset % 3600 {
        0 => format!("{:+03}", offset / 3600),
        _ => moment.now.strftime("%:z").to_string(),
    };
    format!(
        "{project}-{id} ({} {zone}):",
        moment.now.strftime("%m-%d %H:%M:%S")
    )
}

/// `header line`, then the binary's own sentence on the next line when it has one.
/// Claude Code prefixes both the model's context and the person's message with the event
/// name (`SessionStart:startup says:`), so the event stays out.
fn compose(moment: &Moment, line: &str, sentence: &str) -> String {
    match sentence {
        "" => format!("{} {line}", header(moment)),
        _ => format!("{} {line}\n{sentence}", header(moment)),
    }
}

/// Text for the model: header, the model's vitals line, then the sentence.
pub fn for_model(moment: &Moment, sentence: &str) -> String {
    compose(moment, &model_line(&moment.vitals), sentence)
}

/// Text for the person: header, the person's vitals line, then the sentence.
pub fn for_human(moment: &Moment, sentence: &str) -> String {
    compose(moment, &human_line(&moment.vitals, &moment.now), sentence)
}

/// A top-level output field, such as `systemMessage`, `decision`, or `continue`.
pub fn top(field: &str, value: impl Into<Value>) -> Output {
    Output::from_iter([(field.to_owned(), value.into())])
}

/// A field inside `hookSpecificOutput`, such as `additionalContext`, `watchPaths`, or
/// `permissionDecision`, tagged with the event name Claude Code expects beside it.
pub fn specific(moment: &Moment, field: &str, value: impl Into<Value>) -> Output {
    let inner = Output::from_iter([
        ("hookEventName".to_owned(), moment.event.name.clone().into()),
        (field.to_owned(), value.into()),
    ]);
    top("hookSpecificOutput", inner)
}

/// A message for the person at the terminal; the model does not see it.
pub fn to_human(text: impl Into<String>) -> Output {
    top("systemMessage", text.into())
}

/// Context for the model, attached to its next call.
pub fn to_model(moment: &Moment, text: impl Into<String>) -> Output {
    specific(moment, "additionalContext", text.into())
}

/// Join output pieces into one object. Objects under the same key, such as two
/// `hookSpecificOutput` pieces, are joined field by field; otherwise a later piece wins.
pub fn merge(pieces: impl IntoIterator<Item = Output>) -> Output {
    pieces.into_iter().fold(Output::new(), join)
}

fn join(mut whole: Output, piece: Output) -> Output {
    for (key, value) in piece {
        match (whole.get_mut(&key), value) {
            (Some(Value::Object(inner)), Value::Object(more)) => inner.extend(more),
            (_, value) => {
                whole.insert(key, value);
            }
        }
    }
    whole
}

/// Print the output, if any, and return the exit code to end the process with.
pub fn send(output: Output) -> ExitCode {
    if !output.is_empty() {
        println!("{}", Value::Object(output));
    }
    ExitCode::SUCCESS
}
