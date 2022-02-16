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
    guesses: Vec<Guess>,
    secrets: HashSet<Secret>,
}
impl Words {
    pub fn new(lang: Language) -> Self {
        Words {
            lang,
            secrets: Words::from_str(SOLUTIONS[lang as usize]).collect(),
            guesses: Words::from_str(GUESSES[lang as usize]).collect(),
        }
    }
    pub fn with(lang: Language, guesses: Vec<Guess>, secrets: HashSet<Secret>) -> Self {
        Words {
            lang,
            guesses,
            secrets,
        }
    }
    fn from_str(txt: &str) -> Map<Lines<'_>, fn(&'_ str) -> Word> {
        txt.lines().map(|w| w.to_word())
    }
    pub(crate) fn lang(&self) -> &Language {
        &self.lang
    }
    pub(crate) fn guesses(&self) -> &Vec<Guess> {
        &self.guesses
    }
    pub fn secrets(&self) -> &HashSet<Secret> {
        &self.secrets
    }
}

#[derive(Copy, Clone, PartialEq)]
pub enum Language {
    English,
    German,
    NYTimes,
    Primal,
}
impl TryFrom<&str> for Language {
    type Error = String;

    fn try_from(lang: &str) -> Result<Self, Self::Error> {
        match lang.to_ascii_lowercase().as_str() {
            "english" => Ok(English),
            "nytimes" => Ok(NYTimes),
            "primal" => Ok(Primal),
            "german" | "deutsch" => Ok(German),
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
                German => "German",
                NYTimes => "NYTimes",
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

const GUESSES: [&str; 4] = [
    include_str!("../data/word_lists/original/combined.txt"),
    include_str!("../data/word_lists/german/combined.txt"),
    include_str!("../data/word_lists/ny_times/combined.txt"),
    include_str!("../data/word_lists/primal/combined.txt"),
];
const SOLUTIONS: [&str; 4] = [
    include_str!("../data/word_lists/original/solutions.txt"),
    include_str!("../data/word_lists/german/solutions.txt"),
    include_str!("../data/word_lists/ny_times/solutions.txt"),
    include_str!("../data/word_lists/primal/solutions.txt"),
];
