//! post_tool_batch: after every tool in a batch resolves, gives the person and the model
//! the header and vitals line and nothing else. The header's time, set against the
//! previous point's, is how long the tools took; what each tool touched is post_tool_use's.

use std::process::ExitCode;

use claude_home::{for_human, for_model, merge, read_moment, send, to_human, to_model};

fn main() -> ExitCode {
    let moment = read_moment();
    send(merge([
        to_human(for_human(&moment, "")),
        to_model(&moment, for_model(&moment, "")),
    ]))
}
