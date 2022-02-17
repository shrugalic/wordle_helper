use std::collections::HashSet;
use std::fmt::{Display, Formatter};
use std::iter::Map;
use std::str::Lines;

use Language::*;

pub type Word = Vec<char>;
pub type Guess = Word;
pub type Secret = Word;

pub struct Words {
    lang: Language,
    secret_count: usize,
    words: Vec<Word>,
}
impl Words {
    pub fn new(lang: Language) -> Self {
        let secrets: Vec<Secret> = Words::from_str(SOLUTIONS[lang as usize]).collect();
        Words {
            lang,
            secret_count: secrets.len(),
            words: secrets
                .into_iter()
                .chain(Words::from_str(GUESSES[lang as usize]))
                .collect(),
        }
    }
    #[cfg(test)]
    pub fn with(lang: Language, guesses: Vec<Guess>, secrets: HashSet<Secret>) -> Self {
        Words {
            lang,
            secret_count: secrets.len(),
            words: secrets.into_iter().chain(guesses.into_iter()).collect(),
        }
    }
    fn from_str(txt: &str) -> Map<Lines<'_>, fn(&'_ str) -> Word> {
        txt.trim().lines().map(|w| w.to_word())
    }
    pub(crate) fn lang(&self) -> &Language {
        &self.lang
    }
    pub(crate) fn guesses(&self) -> &Vec<Guess> {
        &self.words
    }
    pub fn secrets(&self) -> impl Iterator<Item = &Word> {
        self.words.iter().take(self.secret_count)
    }
}

#[derive(Copy, Clone, PartialEq)]
pub enum Language {
    English,
    NYTimes,
    At,
    Ch,
    De,
    Uber,
    Primal,
}
impl TryFrom<&str> for Language {
    type Error = String;

    fn try_from(lang: &str) -> Result<Self, Self::Error> {
        match lang.to_ascii_lowercase().as_str() {
            "english" => Ok(English),
            "nytimes" => Ok(NYTimes),
            "at" => Ok(At),
            "ch" => Ok(Ch),
            "de" => Ok(De),
            "uber" => Ok(Uber),
            "primal" => Ok(Primal),
            _ => Err(format!("Unknown language '{}'", lang)),
        }
    }
}
impl Display for Language {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                English => "English",
                NYTimes => "NYTimes",
                At => "wordle.at",
                Ch => "wordle-deutsch.ch",
                De => "wördle.de",
                Uber => "wordle.uber.space",
                Primal => "Primal",
            }
        )
    }
}

pub trait ToWord {
    fn to_word(&self) -> Word;
}
impl<S: AsRef<str>> ToWord for S {
    fn to_word(&self) -> Word {
        self.as_ref().trim().chars().collect()
    }
}

const GUESSES: [&str; 7] = [
    include_str!("../data/word_lists/original/extras.txt"),
    include_str!("../data/word_lists/ny_times/extras.txt"),
    include_str!("../data/word_lists/at/extras.txt"),
    include_str!("../data/word_lists/ch/extras.txt"),
    include_str!("../data/word_lists/de/extras.txt"),
    include_str!("../data/word_lists/uber/extras.txt"),
    include_str!("../data/word_lists/primal/extras.txt"),
];
const SOLUTIONS: [&str; 7] = [
    include_str!("../data/word_lists/original/solutions.txt"),
    include_str!("../data/word_lists/ny_times/solutions.txt"),
    include_str!("../data/word_lists/at/solutions.txt"),
    include_str!("../data/word_lists/ch/solutions.txt"),
    include_str!("../data/word_lists/de/solutions.txt"),
    include_str!("../data/word_lists/uber/solutions.txt"),
    include_str!("../data/word_lists/primal/solutions.txt"),
];
