//! post_tool_batch: after every tool in a batch resolves, right before the model's next
//! call, tells the person and the model how many tools the batch ran. The header's time,
//! set against the previous point's, is how long they took; a run of `Ran 1 tool.` where
//! one batch would do shows calls made one by one instead of in parallel.

use std::process::ExitCode;

use claude_home::stat::amount;
use claude_home::{for_human, for_model, merge, read_moment, send, to_human, to_model};

fn main() -> ExitCode {
    let moment = read_moment();
    let tools = moment.event.input["tool_calls"].as_array().map_or(0, Vec::len);
    let sentence = format!("Ran {}.", amount(tools, "tool"));
    send(merge([
        to_human(for_human(&moment, &sentence)),
        to_model(&moment, for_model(&moment, &sentence)),
    ]))
}
