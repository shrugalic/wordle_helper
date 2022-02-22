use std::fmt::{Display, Formatter};

use Language::*;

use crate::cache::WordIndex;
use crate::{SecretIndices, WordAsCharVec};

pub type Word = Vec<char>;

pub struct Words {
    lang: Language,
    secret_cnt: usize,
    words: Vec<Word>,
}
impl Words {
    pub fn new(lang: Language) -> Self {
        const USE_FULL_WORD_LIST: usize = 1;
        let guesses = Words::from_str(GUESSES[lang as usize], USE_FULL_WORD_LIST);
        let secrets = Words::from_str(SOLUTIONS[lang as usize], USE_FULL_WORD_LIST).collect();
        Self::of(guesses, secrets, lang)
    }
    #[cfg(test)] // Partial dictionary for exhaustive testing
    pub fn fractional(lang: Language, step_by: usize) -> Self {
        let guesses = Words::from_str(GUESSES[lang as usize], step_by);
        let secrets = Words::from_str(SOLUTIONS[lang as usize], step_by).collect();
        Self::of(guesses, secrets, lang)
    }
    pub fn of(guesses: impl Iterator<Item = Word>, secrets: Vec<Word>, lang: Language) -> Self {
        let secret_cnt = secrets.len();
        let words: Vec<Word> = secrets.into_iter().chain(guesses).collect();
        println!("{lang}: {secret_cnt} secrets in {} words", words.len());
        Words {
            lang,
            secret_cnt,
            words,
        }
    }
    fn from_str(txt: &str, step_by: usize) -> impl Iterator<Item = Word> + '_ {
        txt.trim().lines().step_by(step_by).map(|w| w.to_word())
    }
    pub(crate) fn lang(&self) -> &Language {
        &self.lang
    }
    pub(crate) fn guesses(&self) -> &Vec<Word> {
        &self.words
    }
    pub(crate) fn guess_indices(&self) -> Vec<WordIndex> {
        (0..self.words.len() as WordIndex).into_iter().collect()
    }
    pub fn secrets(&self) -> impl Iterator<Item = &Word> {
        self.words.iter().take(self.secret_cnt as usize)
    }
    pub fn get(&self, idx: WordIndex) -> &Word {
        &self.words[idx as usize]
    }
    pub fn get_string(&self, idx: WordIndex) -> String {
        self.words[idx as usize].to_string()
    }
    pub fn index_of(&self, wanted: &Word) -> WordIndex {
        self.words.iter().position(|word| wanted.eq(word)).unwrap() as WordIndex
    }
    pub fn secret_count(&self) -> WordIndex {
        self.secret_cnt as WordIndex
    }
    pub fn secret_indices(&self) -> SecretIndices {
        (0..self.secret_cnt as WordIndex).into_iter().collect()
    }
    pub fn scores_to_string<Score: PartialOrd + Display>(
        &self,
        scores: &[(WordIndex, Score)],
        picks: usize,
    ) -> String {
        scores
            .iter()
            .take(picks)
            .map(|(idx, score)| format!("{:.3} {}", score, self.get(*idx).to_string()))
            .collect::<Vec<_>>()
            .join(", ")
    }
    pub fn indices_to_words<'a>(&self, indices: impl Iterator<Item = &'a WordIndex>) -> Vec<Word> {
        indices.map(|i| self.get(*i).to_vec()).collect()
    }

    pub fn indices_to_string<'a>(&self, indices: impl Iterator<Item = &'a WordIndex>) -> String {
        let mut words: Vec<_> = self
            .indices_to_words(indices)
            .into_iter()
            .map(|w| w.to_string())
            .collect();
        words.sort_unstable();
        words.join(", ")
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
