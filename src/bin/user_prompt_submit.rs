//! user_prompt_submit: describes the prompt Yilan just sent, by the numbers in [`stat`], to
//! the person and the model alike. A prompt that did not come from him gets the header alone.

use std::process::ExitCode;

use claude_home::stat::{describe, stat};
use claude_home::{for_human, for_model, merge, read_moment, send, text, to_human, to_model};

/// How the prompts that come from another session, a subagent, the harness, or a background
/// task open. The hook input carries no origin, and the transcript row that does is written
/// only after the hook runs, so the opening is the only sign.
const NOT_HUMAN: [&str; 4] = [
    "Another Claude session sent a message:",
    "<agent-message from=",
    "[Cross-session idle notice]",
    "<task-notification>",
];

fn main() -> ExitCode {
    let moment = read_moment();
    let prompt = text(&moment.event, "prompt");
    let sentence = match NOT_HUMAN.iter().any(|opening| prompt.starts_with(opening)) {
        true => String::new(),
        false => describe("human", &stat(prompt)),
    };
    send(merge([
        to_human(for_human(&moment, &sentence)),
        to_model(&moment.event, for_model(&moment, &sentence)),
    ]))
}
