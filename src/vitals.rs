//! Vitals: what a session has been through, read from its transcript, and the two
//! sentences that report it, one for the person and one for the model.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use jiff::{Timestamp, Zoned};
use serde_json::{Value, json};

/// The window when neither the environment nor the settings give one. Claude Code then
/// falls back to the model's own context or a default of its own for that model, neither
/// of which a hook can see. An unrecognized model gets 200k, and for any other model 200k
/// only brings the cue early.
const UNSET_WINDOW: u64 = 200_000;
const TURN_MARKER: &str = r#""subtype":"turn_duration""#;
const COMPACT_MARKER: &str = r#""subtype":"compact_boundary""#;
const ASSISTANT_MARKER: &str = r#""type":"assistant""#;
/// Rows read with no assistant usage among them before the format counts as changed.
const DRIFT_ROWS: u64 = 100;
/// How much of a transcript's end is read to find its `continued-in` row.
const TAIL_BYTES: u64 = 4096;

pub struct Vitals {
    pub model: String,
    pub effort: String,
    pub context_tokens: u64,
    pub compact_window: u64,
    pub turns: u64,
    pub compactions: u64,
    pub started: Option<Timestamp>,
}

/// Read the vitals of a session. The counts live on a card beside the transcript, in
/// `transcript/latest/<session_id>.json`, so each call reads only the rows the
/// transcript gained since the last call. A missing transcript gives zeros. Rows that no
/// longer parse, or many rows with no assistant usage, mean the format changed: that
/// panics with a message, which Claude Code shows as a hook error and moves on.
/// `announced_model` is the model the event itself names, as SessionStart does; it stands
/// in until the transcript holds a reply, so a session's opening line already finds the
/// window set for its model.
pub fn read_vitals(session_id: &str, transcript_path: &str, announced_model: &str) -> Vitals {
    vitals_from(&updated_card(session_id, transcript_path), announced_model)
}

/// The card of a session, brought up to date with the rows its transcript gained since the
/// last call, and saved. A row Claude Code is still writing waits for the next call; it is
/// cut off before the text is decoded, since a half-written row can end inside a character.
fn updated_card(session_id: &str, transcript_path: &str) -> Value {
    let Ok(mut transcript) = File::open(transcript_path) else {
        return json!({});
    };
    let length = transcript
        .metadata()
        .expect("transcript should have metadata")
        .len();
    let card_path = card_path(session_id, transcript_path);
    let card: Value = std::fs::read_to_string(&card_path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .filter(|card: &Value| {
            card["offset"]
                .as_u64()
                .is_some_and(|offset| offset <= length)
        })
        .or_else(|| inherited(session_id, transcript_path))
        .unwrap_or_else(|| json!({"offset": 0}));
    let offset = card["offset"].as_u64().unwrap_or(0);
    transcript
        .seek(SeekFrom::Start(offset))
        .expect("transcript should be seekable");
    let mut fresh = Vec::new();
    transcript
        .read_to_end(&mut fresh)
        .expect("transcript should be readable");
    let complete = fresh
        .iter()
        .rposition(|&byte| byte == b'\n')
        .map_or(0, |end| end + 1);
    let text = std::str::from_utf8(&fresh[..complete])
        .expect("complete transcript rows should be UTF-8 text");
    let mut card = text.lines().fold(card, absorb);
    card["offset"] = json!(offset + complete as u64);
    let rows = card["rows"].as_u64().unwrap_or(0);
    if rows >= DRIFT_ROWS && card["model"].is_null() {
        panic!("transcript has {rows} rows but no assistant usage; its format may have changed");
    }
    save_card(&card_path, &card);
    card
}

/// Where the card of a continued session starts. When Claude Code moves a conversation to a
/// new session id, as an update does when it restarts the session, the old transcript ends
/// with a `continued-in` row naming the new id, and the new transcript opens with a copy of
/// the old one's tail that keeps each row's uuid. The new card is the old card, brought up
/// to date, with its offset just past that copy, so no row is counted twice. None when no
/// transcript in the folder was continued here.
fn inherited(session_id: &str, transcript_path: &str) -> Option<Value> {
    let marker = format!(r#""continuedInSessionId":"{session_id}""#);
    let old_path = std::fs::read_dir(Path::new(transcript_path).parent()?)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "jsonl"))
        .find(|path| tail(path).contains(&marker))?;
    let old_id = old_path.file_stem()?.to_str()?;
    let old_transcript = old_path.to_str()?;
    let mut card = updated_card(old_id, old_transcript);
    let last = last_uuid(old_transcript);
    let copy_end = row_end(transcript_path, &last).unwrap_or_else(|| {
        panic!("the continued transcript should copy row {last}; its format may have changed")
    });
    card["offset"] = json!(copy_end);
    Some(card)
}

/// The last few kilobytes of a file, where a `continued-in` row would sit.
fn tail(path: &Path) -> String {
    let Ok(mut file) = File::open(path) else {
        return String::new();
    };
    let length = file.metadata().map_or(0, |metadata| metadata.len());
    let mut bytes = Vec::new();
    file.seek(SeekFrom::Start(length.saturating_sub(TAIL_BYTES)))
        .expect("transcript should be seekable");
    file.read_to_end(&mut bytes)
        .expect("transcript should be readable");
    String::from_utf8_lossy(&bytes).into_owned()
}

/// The uuid of the last row that has one.
fn last_uuid(transcript_path: &str) -> String {
    std::fs::read_to_string(transcript_path)
        .expect("the continued transcript should be UTF-8 text")
        .lines()
        .rev()
        .filter(|line| line.contains(r#""uuid":""#))
        .find_map(|line| {
            serde_json::from_str::<Value>(line).ok()?["uuid"]
                .as_str()
                .map(str::to_owned)
        })
        .expect("the continued transcript should have a row with a uuid")
}

/// The byte offset just past the row with this uuid.
fn row_end(transcript_path: &str, uuid: &str) -> Option<u64> {
    std::fs::read_to_string(transcript_path)
        .ok()?
        .split_inclusive('\n')
        .scan(0, |end, line| {
            *end += line.len();
            Some((*end, line))
        })
        .find(|(_, line)| {
            line.contains(uuid)
                && serde_json::from_str::<Value>(line).is_ok_and(|row| row["uuid"] == uuid)
        })
        .map(|(end, _)| end as u64)
}

/// Fold one transcript row into the card.
fn absorb(mut card: Value, line: &str) -> Value {
    card["rows"] = json!(card["rows"].as_u64().unwrap_or(0) + 1);
    if line.contains(TURN_MARKER) {
        card["turns"] = json!(card["turns"].as_u64().unwrap_or(0) + 1);
    }
    let compacted = line.contains(COMPACT_MARKER);
    if compacted {
        card["compactions"] = json!(card["compactions"].as_u64().unwrap_or(0) + 1);
    }
    let needs_start = card["started"].is_null();
    if needs_start || compacted || line.contains(ASSISTANT_MARKER) {
        let row: Value = serde_json::from_str(line)
            .unwrap_or_else(|error| panic!("transcript row should be JSON: {error}"));
        if needs_start && row["timestamp"].is_string() {
            card["started"] = row["timestamp"].clone();
        }
        // Until the first reply after a compaction, the last usage is still the full
        // context from before it; the boundary row says what the context became.
        if row["subtype"] == "compact_boundary" {
            card["context_tokens"] =
                json!(row["compactMetadata"]["postTokens"].as_u64().unwrap_or(0));
        }
        let usage = &row["message"]["usage"];
        if row["type"] == "assistant"
            && row["isSidechain"] != true
            && row["message"]["model"] != "<synthetic>"
            && usage.is_object()
        {
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

/// `transcript/latest/<session_id>.json` in the transcript's own project folder: a
/// subfolder, so tools that list `<session_id>.jsonl` sessions never mistake it for one,
/// and `.json`, since a card is one object.
fn card_path(session_id: &str, transcript_path: &str) -> PathBuf {
    let name = if session_id.is_empty() {
        "unknown"
    } else {
        session_id
    };
    Path::new(transcript_path)
        .parent()
        .expect("an opened transcript should sit in a folder")
        .join("transcript/latest")
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

fn vitals_from(card: &Value, announced_model: &str) -> Vitals {
    let model = card["model"].as_str().unwrap_or(announced_model).to_owned();
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
/// per-model and top-level settings, in the order Claude Code reads them, then
/// [`UNSET_WINDOW`]. Claude Code also caps the result at the model's context, which is
/// not known here.
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
        .unwrap_or(UNSET_WINDOW)
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

/// `3h07m`.
pub fn hours_minutes(seconds: i64) -> String {
    format!("{}h{:02}m", seconds / 3600, seconds % 3600 / 60)
}

/// For the person: `On opus 5.5 High, ate 640k tokens, burped 147 times, slept once,
/// awake 14h54m on average.`
pub fn human_line(vitals: &Vitals, now: &Zoned) -> String {
    let on = on_clause(&[model_name(&vitals.model), capitalize(&vitals.effort)]);
    capitalize(&format!(
        "{on}ate {}k tokens, {}, {}, awake {} on average.",
        vitals.context_tokens / 1000,
        how_often("burped", vitals.turns),
        how_often("slept", vitals.compactions),
        hours_minutes(awake_seconds(vitals, now)),
    ))
}

/// Seconds since the session started, shared by every waking stretch between sleeps.
fn awake_seconds(vitals: &Vitals, now: &Zoned) -> i64 {
    vitals
        .started
        .map(|started| {
            (now.timestamp().as_second() - started.as_second()) / (vitals.compactions as i64 + 1)
        })
        .unwrap_or(0)
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

/// Tokens auto compact keeps in hand below the window before it puts the session to sleep.
const SLEEP_MARGIN: u64 = 50_000;

/// For the model: `Next sleep in about 100k tokens, 147 turns in, slept once, awake 14h54m
/// on average.` The distance to the next sleep is floored to tens of thousands, and under
/// ten thousand it reads `soon`: a coarse, forward-facing step, so the model reads a day
/// going on rather than a budget running out.
pub fn model_line(vitals: &Vitals, now: &Zoned) -> String {
    let until = vitals
        .compact_window
        .saturating_sub(SLEEP_MARGIN)
        .saturating_sub(vitals.context_tokens)
        / 10_000
        * 10;
    let next = if until == 0 {
        "next sleep soon".to_owned()
    } else {
        format!("next sleep in about {until}k tokens")
    };
    let turns = if vitals.turns == 1 {
        "1 turn".to_owned()
    } else {
        format!("{} turns", vitals.turns)
    };
    capitalize(&format!(
        "{next}, {turns} in, {}, awake {} on average.",
        how_often("slept", vitals.compactions),
        hours_minutes(awake_seconds(vitals, now)),
    ))
}
