//! session_start: tells the person and the model what kind of start this is, with the details
//! the event carries that the header and vitals line do not already say.

use std::process::ExitCode;

use claude_home::vitals::hours_minutes;
use claude_home::{Event, merge, read_moment, send, text, to_human, to_model};

/// The `source` values Claude Code documents for SessionStart, as a sentence each.
fn explain(source: &str) -> &'static str {
    match source {
        "startup" => "Started.",
        "resume" => "Resumed.",
        "clear" => "Reset.",
        "compact" => "Compacted.",
        "fork" => "Forked.",
        _ => "Started.",
    }
}

fn cache_state(expired: bool) -> String {
    match expired {
        true => "The prompt cache has likely expired.",
        false => "The prompt cache is likely still warm.",
    }
    .to_owned()
}

/// Every other field the event carries, one sentence each; a missing field gives none.
fn details(event: &Event) -> Vec<String> {
    let input = &event.input;
    [
        input["seconds_since_last_response"]
            .as_i64()
            .map(|seconds| format!("Last reply {} ago.", hours_minutes(seconds))),
        input["prompt_cache_likely_expired"]
            .as_bool()
            .map(cache_state),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn main() -> ExitCode {
    let moment = read_moment();
    let sentence = std::iter::once(explain(text(&moment.event, "source")).to_owned())
        .chain(details(&moment.event))
        .collect::<Vec<_>>()
        .join(" ");
    send(merge([
        to_human(&moment, &sentence),
        to_model(&moment, &sentence),
    ]))
}
