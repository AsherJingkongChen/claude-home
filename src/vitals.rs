//! Vitals: what a session has been through, read from its transcript, and the two
//! sentences that report it, one for the person and one for the model.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use jiff::{Timestamp, Zoned};
use serde_json::{Value, json};

const CONTEXT_WINDOW: u64 = 1_000_000;
const TURN_MARKER: &str = r#""subtype":"turn_duration""#;
const COMPACT_MARKER: &str = r#""subtype":"compact_boundary""#;
const ASSISTANT_MARKER: &str = r#""type":"assistant""#;
/// Rows read with no assistant usage among them before the format counts as changed.
const DRIFT_ROWS: u64 = 100;

pub struct Vitals {
    pub model: String,
    pub effort: String,
    pub context_tokens: u64,
    pub compact_window: u64,
    pub turns: u64,
    pub compactions: u64,
    pub started: Option<Timestamp>,
}

/// Read the vitals of a session. The counts live on a card in
/// `~/.claude/cache/claude-home/vitals/<session_id>.json`, so each call reads only the rows the
/// transcript gained since the last call. A missing transcript gives zeros. Rows that no
/// longer parse, or many rows with no assistant usage, mean the format changed: that
/// panics with a message, which Claude Code shows as a hook error and moves on.
pub fn read_vitals(session_id: &str, transcript_path: &str) -> Vitals {
    let Ok(mut transcript) = File::open(transcript_path) else {
        return vitals_from(&json!({}));
    };
    let length = transcript
        .metadata()
        .expect("transcript should have metadata")
        .len();
    let card_path = card_path(session_id);
    let card: Value = std::fs::read_to_string(&card_path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .filter(|card: &Value| {
            card["offset"]
                .as_u64()
                .is_some_and(|offset| offset <= length)
        })
        .unwrap_or_else(|| json!({"offset": 0}));
    let offset = card["offset"].as_u64().unwrap_or(0);
    transcript
        .seek(SeekFrom::Start(offset))
        .expect("transcript should be seekable");
    let mut fresh = String::new();
    transcript
        .read_to_string(&mut fresh)
        .expect("transcript should be UTF-8 text");
    let complete = fresh.rfind('\n').map_or(0, |end| end + 1);
    let mut card = fresh[..complete].lines().fold(card, absorb);
    card["offset"] = json!(offset + complete as u64);
    let rows = card["rows"].as_u64().unwrap_or(0);
    if rows >= DRIFT_ROWS && card["model"].is_null() {
        panic!("transcript has {rows} rows but no assistant usage; its format may have changed");
    }
    save_card(&card_path, &card);
    vitals_from(&card)
}

/// Fold one transcript row into the card.
fn absorb(mut card: Value, line: &str) -> Value {
    card["rows"] = json!(card["rows"].as_u64().unwrap_or(0) + 1);
    if line.contains(TURN_MARKER) {
        card["turns"] = json!(card["turns"].as_u64().unwrap_or(0) + 1);
    }
    if line.contains(COMPACT_MARKER) {
        card["compactions"] = json!(card["compactions"].as_u64().unwrap_or(0) + 1);
    }
    let needs_start = card["started"].is_null();
    if needs_start || line.contains(ASSISTANT_MARKER) {
        let row: Value = serde_json::from_str(line)
            .unwrap_or_else(|error| panic!("transcript row should be JSON: {error}"));
        if needs_start && row["timestamp"].is_string() {
            card["started"] = row["timestamp"].clone();
        }
        let usage = &row["message"]["usage"];
        if row["type"] == "assistant" && row["isSidechain"] != true && usage.is_object() {
            card["model"] = row["message"]["model"].clone();
            card["effort"] = row["effort"].clone();
            card["context_tokens"] = json!(
                [
                    "input_tokens",
                    "cache_creation_input_tokens",
                    "cache_read_input_tokens"
                ]
                .iter()
                .map(|key| usage[key].as_u64().unwrap_or(0))
                .sum::<u64>()
            );
        }
    }
    card
}

fn card_path(session_id: &str) -> PathBuf {
    let name = if session_id.is_empty() {
        "unknown"
    } else {
        session_id
    };
    Path::new(&std::env::var("HOME").unwrap_or_default())
        .join(".claude/cache/claude-home/vitals")
        .join(format!("{name}.json"))
}

/// Write the card through a temporary file and a rename, so a parallel hook never reads
/// half a card.
fn save_card(path: &Path, card: &Value) {
    let directory = path
        .parent()
        .expect("the card path should have a directory");
    std::fs::create_dir_all(directory).expect("the card directory should be creatable");
    let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
    std::fs::write(&temporary, card.to_string()).expect("the card should be writable");
    std::fs::rename(&temporary, path).expect("the card should be renamable into place");
}

fn vitals_from(card: &Value) -> Vitals {
    let model = card["model"].as_str().unwrap_or("").to_owned();
    Vitals {
        compact_window: compact_window(&model),
        effort: card["effort"].as_str().unwrap_or("").to_owned(),
        model,
        context_tokens: card["context_tokens"].as_u64().unwrap_or(0),
        turns: card["turns"].as_u64().unwrap_or(0),
        compactions: card["compactions"].as_u64().unwrap_or(0),
        started: card["started"]
            .as_str()
            .and_then(|stamp| stamp.parse().ok()),
    }
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
