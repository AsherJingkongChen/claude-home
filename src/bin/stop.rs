//! stop: describes the model's last message, and what it is still waiting on, in one
//! `stopReason`. The person reads it now; it stays in the conversation, so the model reads
//! it at its next call. `continue: false` only unlocks `stopReason`: the turn is ending
//! anyway, and `additionalContext` or a block would cost one more model call.

use std::process::ExitCode;

use croner::Cron;
use jiff::Zoned;

use claude_home::stat::{amount, describe, stat};
use claude_home::{Event, for_model, merge, read_moment, send, stamp, text, top};

/// `Waiting on 3 jobs.`, or nothing when no background task is in flight.
fn waiting(event: &Event) -> Option<String> {
    let jobs = event.input["background_tasks"].as_array().map_or(0, Vec::len);
    (jobs > 0).then(|| format!("Waiting on {}.", amount(jobs, "job")))
}

/// `Waking at 11-01 09:00:00 +08.` for the nearest scheduled wake, or nothing when none is
/// scheduled. A schedule that does not parse as cron is left out.
fn waking(event: &Event, now: &Zoned) -> Option<String> {
    event.input["session_crons"]
        .as_array()?
        .iter()
        .filter_map(|cron| cron["schedule"].as_str()?.parse::<Cron>().ok())
        .filter_map(|cron| cron.find_next_occurrence(now, false).ok())
        .min()
        .map(|next| format!("Waking at {}.", stamp(&next)))
}

fn main() -> ExitCode {
    let moment = read_moment();
    let event = &moment.event;
    let sentence = [
        Some(describe("model", &stat(text(event, "last_assistant_message")))),
        waiting(event),
        waking(event, &moment.now),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join("\n");
    send(merge([
        top("continue", false),
        top("stopReason", for_model(&moment, &sentence)),
    ]))
}
