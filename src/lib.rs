//! claude-home: hooks for the Claude Code residents of this house.
//!
//! Every hook event gets its own binary in `src/bin/`, named after the event in snake case.
//! This library is the station they share: it reads the event from stdin and sends the
//! reply in the one shape Claude Code accepts. A hook's output can go four ways:
//!
//! | way    | field                                    | who reads it              |
//! |--------|------------------------------------------|---------------------------|
//! | model  | `hookSpecificOutput.additionalContext`   | the model, on its next call |
//! | human  | `systemMessage`                          | the person at the terminal |
//! | system | `decision: "block"` with `reason`        | Claude Code itself        |
//! | air    | nothing on stdout                        | nobody                    |
//!
//! Not every event takes every way; the hooks reference lists which do.

use std::io::Read;
use std::process::ExitCode;

use serde_json::{Map, Value, json};

/// One hook call, as Claude Code sent it on stdin.
pub struct Event {
    pub name: String,
    pub input: Value,
}

impl Event {
    /// Read the event from stdin. Panics with a message when stdin is not a JSON object
    /// with `hook_event_name`, which Claude Code reports as a non-blocking error.
    pub fn read() -> Self {
        let mut raw = String::new();
        std::io::stdin().read_to_string(&mut raw).expect("stdin should be readable");
        let input: Value = serde_json::from_str(&raw).expect("stdin should be one JSON object");
        let name = input["hook_event_name"]
            .as_str()
            .expect("the event should carry hook_event_name")
            .to_owned();
        Self { name, input }
    }

    /// A string field of the input, or "" when it is missing.
    pub fn text(&self, key: &str) -> &str {
        self.input[key].as_str().unwrap_or("")
    }
}

/// What a hook says back. Collected first, sent once, because stdout must hold a single
/// JSON object.
#[derive(Default)]
pub struct Reply {
    model: Vec<String>,
    human: Vec<String>,
    block: Option<String>,
}

impl Reply {
    pub fn to_model(&mut self, text: impl Into<String>) -> &mut Self {
        self.model.push(text.into());
        self
    }

    pub fn to_human(&mut self, text: impl Into<String>) -> &mut Self {
        self.human.push(text.into());
        self
    }

    pub fn block(&mut self, reason: impl Into<String>) -> &mut Self {
        self.block = Some(reason.into());
        self
    }

    /// Print the reply for `event` and return the exit code to end the process with.
    pub fn send(&self, event: &Event) -> ExitCode {
        let mut output = Map::new();
        if !self.model.is_empty() {
            output.insert(
                "hookSpecificOutput".into(),
                json!({"hookEventName": event.name, "additionalContext": self.model.join("\n")}),
            );
        }
        if !self.human.is_empty() {
            output.insert("systemMessage".into(), self.human.join("\n").into());
        }
        if let Some(reason) = &self.block {
            output.insert("decision".into(), "block".into());
            output.insert("reason".into(), reason.clone().into());
        }
        if !output.is_empty() {
            println!("{}", Value::Object(output));
        }
        ExitCode::SUCCESS
    }
}
