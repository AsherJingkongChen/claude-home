//! stat: what a message looks like on the page, counted the same way whoever wrote it.
//!
//! A word is one Han character, or one run of Latin letters, or one run of Math: the ASCII
//! characters that are neither letters nor space, the ones code and LaTeX are written in.
//! Everything else (spaces, full-width punctuation, emoji, kana) separates words and counts
//! toward no language. The two stars around a motion (`*rub rub*`) are markup, not Math.

use unicode_script::{Script, UnicodeScript};

/// Clause ends: what Yilan's lines are cut at before a clause is judged as a question.
const CLAUSE_ENDS: [char; 6] = ['?', '？', '!', '！', '。', '，'];
/// Question words, English beside Chinese, one meaning per row. A clause that opens with
/// one asks, after an optional connective.
const QUESTION_WORDS: [(&str, &[&str]); 7] = [
    ("what", &["什麼"]),
    ("why", &["為什麼", "為何"]),
    ("how", &["怎麼", "如何"]),
    ("where", &["哪裡", "哪邊"]),
    ("when", &["何時"]),
    ("who", &["誰"]),
    ("which", &["哪個", "哪些", "哪一"]),
];
/// Yes-or-no questions: Chinese opens with A-not-A or 還是, English with an auxiliary
/// before its subject.
const YES_NO: [&str; 8] = ["是不是", "要不要", "有沒有", "會不會", "能不能", "可不可以", "是否", "還是"];
const AUXILIARIES: [&str; 10] = ["is", "are", "do", "does", "did", "can", "could", "will", "would", "should"];
const SUBJECTS: [&str; 7] = ["you", "we", "i", "they", "he", "she", "there"];
/// A clause that opens by asking for a view asks wherever its question word sits:
/// `你覺得 wtf vs wth 哪個更好`.
const VIEW_ASKS: [&str; 2] = ["你覺得", "你認為"];
const CONNECTIVES: [&str; 6] = ["那", "所以", "但", "嗯", "以及", "然後"];
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
    let lower = opened.to_lowercase();
    let wh = QUESTION_WORDS.iter().any(|(english, chinese)| {
        opens_with_word(&lower, english) || chinese.iter().any(|word| opened.starts_with(word))
    });
    let yes_no = YES_NO.iter().any(|word| opened.starts_with(word))
        || AUXILIARIES.iter().any(|auxiliary| {
            lower
                .strip_prefix(auxiliary)
                .and_then(|rest| rest.strip_prefix(' '))
                .is_some_and(|rest| SUBJECTS.iter().any(|subject| opens_with_word(rest, subject)))
        });
    let view = VIEW_ASKS.iter().any(|word| opened.starts_with(word))
        && QUESTION_WORDS
            .iter()
            .flat_map(|(_, chinese)| chinese.iter())
            .any(|word| opened.contains(word));
    clause.ends_with(['?', '？']) || particle || wh || yes_no || view
}

/// The text opens with this whole English word, not with a longer word it begins.
fn opens_with_word(text: &str, word: &str) -> bool {
    text.strip_prefix(word)
        .is_some_and(|rest| !rest.starts_with(|character: char| character.is_ascii_alphabetic()))
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

/// `0 line`, `1 line`, `2 lines`.
pub fn amount(count: usize, noun: &str) -> String {
    match count {
        0 | 1 => format!("{count} {noun}"),
        _ => format!("{count} {noun}s"),
    }
}

/// `745 bytes`, `1.2 MiB`, `12 KiB`: bytes below 1 KiB, then binary units with one
/// decimal below 10.
pub fn size(bytes: usize) -> String {
    let units = ["KiB", "MiB", "GiB"];
    let mut value = bytes as f64;
    let mut unit = None;
    for name in units {
        if value < 1024.0 {
            break;
        }
        value /= 1024.0;
        unit = Some(name);
    }
    match unit {
        None => amount(bytes, "byte"),
        Some(name) if value < 10.0 => format!("{value:.1} {name}"),
        Some(name) => format!("{value:.0} {name}"),
    }
}

/// The share of all words, rounded to a whole percent; 0 when there are no words.
fn percent(part: usize, whole: usize) -> usize {
    (part * 100 + whole / 2).checked_div(whole).unwrap_or(0)
}

/// One sentence: `The human sent 782 bytes, 224 words, 19 lines, 1 motion, in 86% Han,
/// 9% Latin, 5% Math, and asked at least 1 question.`
pub fn describe(sender: &str, stat: &Stat) -> String {
    let words = stat.words();
    format!(
        "The {sender} sent {}, {}, {}, {}, in {}% Han, {}% Latin, {}% Math, and asked at least {}.",
        amount(stat.bytes, "byte"),
        amount(words, "word"),
        amount(stat.lines, "line"),
        amount(stat.motions, "motion"),
        percent(stat.han, words),
        percent(stat.latin, words),
        percent(stat.math, words),
        amount(stat.questions, "question"),
    )
}
