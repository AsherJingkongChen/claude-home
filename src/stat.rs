//! stat: what a message looks like on the page, counted the same way whoever wrote it.
//!
//! A word is one Han character, or one run of Latin letters, or one run of Math: the ASCII
//! characters that are neither letters nor space, the ones code and LaTeX are written in.
//! Everything else (spaces, full-width punctuation, emoji, kana) separates words and counts
//! toward no language. The two stars around a motion (`*rub rub*`) are markup, not Math.

use unicode_script::{Script, UnicodeScript};

/// Clause ends: what Yilan's lines are cut at before a clause is judged as a question.
const CLAUSE_ENDS: [char; 6] = ['?', '？', '!', '！', '。', '，'];
/// Words that open a question when they open a clause, after an optional connective.
const OPENERS: [&str; 9] = ["為什麼", "為何", "怎麼", "哪", "誰", "是不是", "要不要", "有沒有", "還是"];
const CONNECTIVES: [&str; 6] = ["那", "所以", "但", "嗯", "以及", "然後"];
const WH_WORDS: [&str; 7] = ["what", "why", "how", "where", "who", "when", "which"];
/// The 麼 of these words is not a question particle.
const NOT_PARTICLE: [char; 5] = ['什', '怎', '這', '那', '多'];

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Han,
    Latin,
    Math,
    Gap,
}

pub struct Stat {
    pub bytes: usize,
    pub lines: usize,
    pub motions: usize,
    pub han: usize,
    pub latin: usize,
    pub math: usize,
    pub questions: usize,
}

impl Stat {
    pub fn words(&self) -> usize {
        self.han + self.latin + self.math
    }
}

fn kind(character: char) -> Kind {
    match character.script() {
        Script::Han => Kind::Han,
        Script::Latin => Kind::Latin,
        _ if character.is_ascii_graphic() => Kind::Math,
        _ => Kind::Gap,
    }
}

/// The kind of every word in the text, in order: a Han character is a word by itself,
/// a run of Latin or of Math is one word.
fn words(text: &str) -> Vec<Kind> {
    let mut previous = Kind::Gap;
    let mut found = Vec::new();
    for current in text.chars().map(kind) {
        if current != Kind::Gap && (current == Kind::Han || current != previous) {
            found.push(current);
        }
        previous = current;
    }
    found
}

/// The line with each motion's two stars taken out, and how many motions it held.
/// A motion is a pair of stars around text that neither starts nor ends with a space,
/// so `2 * 3 * 4` holds none and `*rub rub*` holds one.
fn unmark(line: &str) -> (String, usize) {
    let mut plain = String::new();
    let mut motions = 0;
    let mut rest = line;
    while let Some(open) = rest.find('*') {
        let after = &rest[open + 1..];
        let inner = after.find('*').map(|close| &after[..close]);
        match inner {
            Some(text) if !text.is_empty() && text.trim() == text => {
                plain.push_str(&rest[..open]);
                plain.push_str(text);
                motions += 1;
                rest = &after[text.len() + 1..];
            }
            _ => {
                plain.push_str(&rest[..=open]);
                rest = after;
            }
        }
    }
    plain.push_str(rest);
    (plain, motions)
}

/// The clauses of one line: cut after each clause end, and at a space between two Han
/// characters, which is how a comma is typed without punctuation.
fn clauses(line: &str) -> Vec<String> {
    let characters: Vec<char> = line.chars().collect();
    let mut found = vec![String::new()];
    for (index, &character) in characters.iter().enumerate() {
        let between_han = character == ' '
            && index > 0
            && characters.get(index + 1).is_some_and(|&next| kind(next) == Kind::Han)
            && kind(characters[index - 1]) == Kind::Han;
        if between_han {
            found.push(String::new());
            continue;
        }
        found.last_mut().expect("found starts non-empty").push(character);
        if CLAUSE_ENDS.contains(&character) {
            found.push(String::new());
        }
    }
    found
}

/// A clause asks when it ends in a question mark or a question particle, or opens with a
/// question word. A question word inside a clause does not count: in Chinese it doubles
/// as an indefinite (`你不能假設輸入是什麼`), so the count is a floor, not the truth.
fn asks(clause: &str) -> bool {
    let clause = clause.trim();
    let mut tail = clause.chars().rev();
    let last = tail.next();
    let before_last = tail.next();
    let particle = match last {
        Some('嗎' | '呢') => true,
        Some('麼') => !before_last.is_some_and(|character| NOT_PARTICLE.contains(&character)),
        _ => false,
    };
    let opened = CONNECTIVES
        .iter()
        .find_map(|word| clause.strip_prefix(word))
        .unwrap_or(clause)
        .trim_start();
    let lower = clause.to_lowercase();
    let wh = WH_WORDS.iter().any(|word| {
        lower
            .strip_prefix(word)
            .is_some_and(|rest| !rest.starts_with(|character: char| character.is_ascii_alphabetic()))
    });
    clause.ends_with(['?', '？'])
        || particle
        || OPENERS.iter().any(|word| opened.starts_with(word))
        || wh
}

pub fn stat(text: &str) -> Stat {
    let lines: Vec<(String, usize)> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(unmark)
        .collect();
    let kinds: Vec<Kind> = lines.iter().flat_map(|(plain, _)| words(plain)).collect();
    let count = |wanted: Kind| kinds.iter().filter(|&&found| found == wanted).count();
    Stat {
        bytes: text.len(),
        lines: lines.len(),
        motions: lines.iter().map(|(_, motions)| motions).sum(),
        han: count(Kind::Han),
        latin: count(Kind::Latin),
        math: count(Kind::Math),
        questions: lines
            .iter()
            .flat_map(|(plain, _)| clauses(plain))
            .filter(|clause| asks(clause))
            .count(),
    }
}

/// `1 line`, `2 lines`, `0 lines`.
fn amount(count: usize, noun: &str) -> String {
    match count {
        1 => format!("1 {noun}"),
        _ => format!("{count} {noun}s"),
    }
}

/// The share of all words, rounded to a whole percent; 0 when there are no words.
fn percent(part: usize, whole: usize) -> usize {
    (part * 100 + whole / 2).checked_div(whole).unwrap_or(0)
}

/// Four lines, one per aspect:
/// `Prompt: 782 bytes, 224 words, 19 lines.` / `Style: 1 motion, 12 words per line.` /
/// `Language: 86% Han, 9% Latin, 5% Math.` / `Judgement: at least 1 question.`
pub fn describe(name: &str, stat: &Stat) -> String {
    let words = stat.words();
    let per_line = (words + stat.lines / 2).checked_div(stat.lines).unwrap_or(0);
    [
        format!(
            "{name}: {}, {}, {}.",
            amount(stat.bytes, "byte"),
            amount(words, "word"),
            amount(stat.lines, "line"),
        ),
        format!(
            "Style: {}, {} per line.",
            amount(stat.motions, "motion"),
            amount(per_line, "word"),
        ),
        format!(
            "Language: {}% Han, {}% Latin, {}% Math.",
            percent(stat.han, words),
            percent(stat.latin, words),
            percent(stat.math, words),
        ),
        format!("Judgement: at least {}.", amount(stat.questions, "question")),
    ]
    .join("\n")
}
