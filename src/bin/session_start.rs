//! session_start: tells the person and the model what kind of start this is, with the details
//! the event carries that the header and vitals line do not already say.

use std::process::ExitCode;

use claude_home::vitals::{Vitals, hours_minutes};
use claude_home::{
    Event, Moment, for_human, for_model, merge, read_moment, send, text, to_human, to_model,
};

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

/// SessionStart with source `compact` runs before Claude Code writes this compaction's
/// boundary row, so the transcript counts one compaction short at that moment, and its
/// last usage is still the full context from before. The extra compaction and the
/// emptied context live only in this output; the card keeps counting rows.
fn count_pending(moment: Moment) -> Moment {
    let compacted = text(&moment.event, "source") == "compact";
    Moment {
        vitals: Vitals {
            compactions: moment.vitals.compactions + u64::from(compacted),
            context_tokens: if compacted { 0 } else { moment.vitals.context_tokens },
            ..moment.vitals
        },
        ..moment
    }
}

fn main() -> ExitCode {
    let moment = count_pending(read_moment());
    let sentence = std::iter::once(explain(text(&moment.event, "source")).to_owned())
        .chain(details(&moment.event))
        .collect::<Vec<_>>()
        .join(" ");
    send(merge([
        to_human(for_human(&moment, &sentence)),
        to_model(&moment.event, for_model(&moment, &sentence)),
    ]))
}
