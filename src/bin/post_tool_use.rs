//! post_tool_use: after a Read, an Edit, or a Write, tells the person and the model which
//! lines it touched, from the tool's structured result, which the model never sees. Every
//! other tool gets nothing. The sentence carries no header: PostToolBatch marks the time.

use std::process::ExitCode;

use serde_json::Value;

use claude_home::stat::amount;
use claude_home::{Output, merge, read_moment, send, text, to_human, to_model};

/// The file name alone: the model knows which file it just touched, and the person sees
/// the tool call, so the folders only lengthen the line.
fn name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// `lines 29 to 35`, or `line 29` when the range is one line.
fn span(first: u64, last: u64) -> String {
    match first == last {
        true => format!("line {first}"),
        false => format!("lines {first} to {last}"),
    }
}

/// `Read SKILL.md at lines 120 to 239 of 926.` Images, PDFs, and notebooks get nothing.
fn read(response: &Value) -> Option<String> {
    let file = &response["file"];
    let path = name(file["filePath"].as_str()?);
    let start = file["startLine"].as_u64()?;
    let count = file["numLines"].as_u64()?;
    let total = file["totalLines"].as_u64()?;
    (response["type"] == "text").then(|| match count {
        0 => format!("Read nothing from {path}, which has {}.", amount(total as usize, "line")),
        _ => format!("Read {path} at {} of {total}.", span(start, start + count - 1)),
    })
}

/// `Edited SKILL.md at lines 29 to 35, adding 7 lines and removing 6.` The range covers the
/// changed lines only, numbered in the new file, not the context around them.
fn edit(response: &Value) -> Option<String> {
    let path = name(response["filePath"].as_str()?);
    let hunks = response["structuredPatch"].as_array()?;
    let mut added = 0;
    let mut removed = 0;
    let mut changed: Vec<u64> = Vec::new();
    for hunk in hunks {
        let mut line = hunk["newStart"].as_u64()?;
        for row in hunk["lines"].as_array()?.iter().filter_map(Value::as_str) {
            match row.chars().next() {
                Some('+') => {
                    added += 1;
                    changed.push(line);
                    line += 1;
                }
                Some('-') => {
                    removed += 1;
                    changed.push(line);
                }
                _ => line += 1,
            }
        }
    }
    let first = *changed.iter().min()?;
    let last = *changed.iter().max()?;
    let places = match hunks.len() {
        1 => String::new(),
        count => format!(" in {count} places,"),
    };
    Some(format!(
        "Edited {path}{places} at {}, adding {} and removing {removed}.",
        span(first, last),
        amount(added, "line"),
    ))
}

/// `Created README.md with 120 lines.` or `Overwrote README.md with 120 lines.`
fn write(response: &Value) -> Option<String> {
    let path = name(response["filePath"].as_str()?);
    let lines = amount(response["content"].as_str()?.lines().count(), "line");
    match response["type"].as_str()? {
        "create" => Some(format!("Created {path} with {lines}.")),
        _ => Some(format!("Overwrote {path} with {lines}.")),
    }
}

fn main() -> ExitCode {
    let moment = read_moment();
    let event = &moment.event;
    let response = &event.input["tool_response"];
    let sentence = match text(event, "tool_name") {
        "Read" => read(response),
        "Edit" => edit(response),
        "Write" => write(response),
        _ => None,
    };
    send(match sentence {
        Some(sentence) => merge([to_human(sentence.clone()), to_model(&moment, sentence)]),
        None => Output::new(),
    })
}
