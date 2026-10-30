//! user_prompt_submit: describes the prompt Yilan just sent, by the numbers in [`stat`], to
//! the person and the model alike.

use std::process::ExitCode;

use claude_home::stat::{describe, stat};
use claude_home::{for_human, for_model, merge, read_moment, send, text, to_human, to_model};

fn main() -> ExitCode {
    let moment = read_moment();
    let sentence = describe("human", &stat(text(&moment.event, "prompt")));
    send(merge([
        to_human(for_human(&moment, &sentence)),
        to_model(&moment, for_model(&moment, &sentence)),
    ]))
}
