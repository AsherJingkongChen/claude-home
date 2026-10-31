//! post_tool_batch: after every tool in a batch resolves, tells the person and the model
//! how many tools ran and how much they brought back. The header's time, set against the
//! previous point's, is how long the batch took.

use std::process::ExitCode;

use humansize::{BINARY, format_size};
use serde_json::Value;

use claude_home::stat::amount;
use claude_home::{for_human, for_model, merge, read_moment, send, to_human, to_model};

/// The bytes of one tool result as the model receives it: the text itself, or the
/// serialized blocks when the result is a content-block array.
fn result_bytes(response: &Value) -> usize {
    match response {
        Value::Null => 0,
        Value::String(text) => text.len(),
        other => other.to_string().len(),
    }
}

fn main() -> ExitCode {
    let moment = read_moment();
    let calls = moment.event.input["tool_calls"].as_array().cloned().unwrap_or_default();
    let bytes: usize = calls.iter().map(|call| result_bytes(&call["tool_response"])).sum();
    let sentence = format!(
        "Ran {} and brought back {}.",
        amount(calls.len(), "tool"),
        format_size(bytes, BINARY.decimal_places(1)),
    );
    send(merge([
        to_human(for_human(&moment, &sentence)),
        to_model(&moment, for_model(&moment, &sentence)),
    ]))
}
