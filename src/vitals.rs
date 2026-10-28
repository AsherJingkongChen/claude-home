//! Vitals: what a session has been through, read from its transcript, and the two
//! sentences that report it, one for the person and one for the model.

use std::path::Path;

use jiff::{Timestamp, Zoned};
use serde_json::Value;

const CONTEXT_WINDOW: u64 = 1_000_000;

pub struct Vitals {
    pub model: String,
    pub effort: String,
    pub context_tokens: u64,
    pub compact_window: u64,
    pub turns: u64,
    pub compactions: u64,
    pub started: Option<Timestamp>,
}

/// Read the vitals from a transcript file. A missing or empty transcript gives zeros.
pub fn read_vitals(transcript_path: &str) -> Vitals {
    let text = std::fs::read_to_string(transcript_path).unwrap_or_default();
    let started = text
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find_map(|row| {
            row["timestamp"]
                .as_str()
                .and_then(|stamp| stamp.parse().ok())
        });
    let turns = count_rows(&text, r#""subtype":"turn_duration""#);
    let compactions = count_rows(&text, r#""subtype":"compact_boundary""#);
    let last = text
        .lines()
        .rev()
        .filter(|line| line.contains(r#""type":"assistant""#))
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find(|row| row["type"] == "assistant" && row["message"]["usage"].is_object())
        .unwrap_or_default();
    let usage = &last["message"]["usage"];
    let context_tokens = [
        "input_tokens",
        "cache_creation_input_tokens",
        "cache_read_input_tokens",
    ]
    .iter()
    .map(|key| usage[key].as_u64().unwrap_or(0))
    .sum();
    let model = last["message"]["model"].as_str().unwrap_or("").to_owned();
    Vitals {
        compact_window: compact_window(&model),
        effort: last["effort"].as_str().unwrap_or("").to_owned(),
        model,
        context_tokens,
        turns,
        compactions,
        started,
    }
}

fn count_rows(text: &str, marker: &str) -> u64 {
    text.lines().filter(|line| line.contains(marker)).count() as u64
}

/// The token count where auto compaction would fire: the env override, then the
/// per-model and top-level settings, then the full context window.
fn compact_window(model: &str) -> u64 {
    let from_env = std::env::var("CLAUDE_CODE_AUTO_COMPACT_WINDOW")
        .ok()
        .and_then(|value| value.parse().ok());
    let settings_path =
        Path::new(&std::env::var("HOME").unwrap_or_default()).join(".claude/settings.json");
    let settings: Value = std::fs::read_to_string(settings_path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    from_env
        .or_else(|| settings["modelSettings"][model]["autoCompactWindow"].as_u64())
        .or_else(|| settings["autoCompactWindow"].as_u64())
        .unwrap_or(CONTEXT_WINDOW)
}

/// `claude-opus-5-5` becomes `opus 5.5`; a trailing date or `[1m]` tag is dropped.
pub fn model_name(model: &str) -> String {
    let bare = model.trim_start_matches("claude-");
    let bare = bare.split('[').next().unwrap_or(bare);
    let parts: Vec<&str> = bare
        .split('-')
        .filter(|part| part.len() != 8 || part.parse::<u32>().is_err())
        .collect();
    match parts.iter().position(|part| part.parse::<u32>().is_ok()) {
        Some(index) => format!("{} {}", parts[..index].join("-"), parts[index..].join(".")),
        None => parts.join("-"),
    }
}

/// `slept once`, `slept twice`, `slept 3 times`, `never slept`.
pub fn how_often(verb: &str, count: u64) -> String {
    match count {
        0 => format!("never {verb}"),
        1 => format!("{verb} once"),
        2 => format!("{verb} twice"),
        _ => format!("{verb} {count} times"),
    }
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

fn hours_minutes(seconds: i64) -> String {
    format!("{}h{:02}m", seconds / 3600, seconds % 3600 / 60)
}

/// For the person: `On opus 5.5 High, ate 640k tokens, burped 147 times, slept once,
/// awake 14h54m on average.`
pub fn human_line(vitals: &Vitals, now: &Zoned) -> String {
    let awake = vitals
        .started
        .map(|started| {
            (now.timestamp().as_second() - started.as_second()) / (vitals.compactions as i64 + 1)
        })
        .unwrap_or(0);
    let on = on_clause(&[model_name(&vitals.model), capitalize(&vitals.effort)]);
    capitalize(&format!(
        "{on}ate {}k tokens, {}, {}, awake {} on average.",
        vitals.context_tokens / 1000,
        how_often("burped", vitals.turns),
        how_often("slept", vitals.compactions),
        hours_minutes(awake),
    ))
}

/// `On opus 5.5 High, ` from the non-empty parts, or "" when every part is empty, as in
/// a session with no transcript yet.
fn on_clause(parts: &[String]) -> String {
    let words: Vec<&str> = parts
        .iter()
        .map(String::as_str)
        .filter(|part| !part.is_empty())
        .collect();
    if words.is_empty() {
        String::new()
    } else {
        format!("On {}, ", words.join(" "))
    }
}

/// For the model: `On high effort, 20% of context left, 147 turns in, compacted once.`
/// Context left steps 100%, 90%, ..., 10%: the used share floored to tens, taken from 100.
/// A coarse step keeps the model from counting tokens; 10% is the cue to wrap up.
pub fn model_line(vitals: &Vitals) -> String {
    let used = vitals.context_tokens.min(vitals.compact_window);
    let used_percent = (used * 100 / vitals.compact_window.max(1)).min(99);
    let left = format!("{}%", 100 - used_percent / 10 * 10);
    let turns = if vitals.turns == 1 {
        "1 turn".to_owned()
    } else {
        format!("{} turns", vitals.turns)
    };
    let effort = if vitals.effort.is_empty() {
        String::new()
    } else {
        format!("{} effort", vitals.effort)
    };
    capitalize(&format!(
        "{}{left} of context left, {turns} in, {}.",
        on_clause(&[effort]),
        how_often("compacted", vitals.compactions),
    ))
}
