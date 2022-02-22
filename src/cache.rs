use std::collections::BTreeSet;

use rayon::prelude::*;

use crate::{CalcHintValue, GetHint, Words};

pub type WordIndex = u16; // max 65'565 words
pub type SecretIndices = BTreeSet<WordIndex>;
pub type HintValue = u8;

pub struct Cache {
    hint_by_secret_by_guess: Vec<Vec<HintValue>>,
    solutions_by_hint_by_guess: Vec<Vec<BTreeSet<WordIndex>>>,
}
impl Cache {
    pub fn new(words: &Words) -> Self {
        let hints_by_secret_by_guess = hint_by_secret_idx_by_guess_idx(words);
        let solutions_by_hint_by_guess =
            solution_indices_by_hint_by_guess_idx(words, &hints_by_secret_by_guess);
        Cache {
            hint_by_secret_by_guess: hints_by_secret_by_guess,
            solutions_by_hint_by_guess,
        }
    }
    pub fn hint(&self, g: WordIndex, s: WordIndex) -> HintValue {
        self.hint_by_secret_by_guess[g as usize][s as usize]
    }
    pub fn solutions(&self, g: WordIndex, s: WordIndex) -> &BTreeSet<WordIndex> {
        let hint = self.hint_by_secret_by_guess[g as usize][s as usize];
        self.solutions_by(g, hint)
    }
    pub fn solutions_by(&self, g: WordIndex, hint: HintValue) -> &BTreeSet<WordIndex> {
        &self.solutions_by_hint_by_guess[g as usize][hint as usize]
    }
    pub fn solutions_by_hint_for(&self, g: WordIndex) -> &Vec<BTreeSet<WordIndex>> {
        &self.solutions_by_hint_by_guess[g as usize]
    }
}

fn hint_by_secret_idx_by_guess_idx(words: &Words) -> Vec<Vec<HintValue>> {
    words
        .guesses()
        .into_par_iter()
        .map(|guess| {
            words
                .secrets()
                .map(|secret| guess.calculate_hint(secret).value())
                .collect()
        })
        .collect()
}

fn solution_indices_by_hint_by_guess_idx(
    words: &Words,
    hint_by_secret_by_guess: &[Vec<HintValue>],
) -> Vec<Vec<BTreeSet<WordIndex>>> {
    words
        .guess_indices()
        .into_par_iter()
        .map(|guess_idx| {
            let mut solutions_by_hint: Vec<BTreeSet<WordIndex>> = vec![BTreeSet::new(); 243];
            for secret_idx in words.secret_indices() {
                let hint = hint_by_secret_by_guess[guess_idx as usize][secret_idx as usize];
                solutions_by_hint[hint as usize].insert(secret_idx as WordIndex);
            }
            solutions_by_hint
        })
        .collect()
}
