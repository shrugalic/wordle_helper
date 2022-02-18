use std::collections::BTreeSet;
use std::time::Instant;

use rayon::prelude::*;

use crate::{CalcHintValue, GetHint, Word, Words};

pub type WordIndex = u16; // max 65'565 words
pub type SolutionsIdx = BTreeSet<WordIndex>;
pub type HintValue = u8;

pub struct Cache2 {
    hints_by_secret_by_guess: Vec<Vec<HintValue>>,
    solutions_by_hint_by_guess: Vec<Vec<BTreeSet<WordIndex>>>,
}
impl Cache2 {
    pub fn with(words: &Words) -> Self {
        Self::new(words.guesses(), words.secret_count())
    }
    pub fn new(guesses: &[Word], secret_cnt: usize) -> Self {
        let hints_by_secret_by_guess = hint_by_secret_idx_by_guess_idx(guesses, secret_cnt);
        let solutions_by_hint_by_guess =
            solution_indices_by_hint_by_guess_idx(guesses, secret_cnt, &hints_by_secret_by_guess);
        Cache2 {
            hints_by_secret_by_guess,
            solutions_by_hint_by_guess,
        }
    }
    pub fn hint(&self, g: usize, s: usize) -> HintValue {
        self.hints_by_secret_by_guess[g][s]
    }
    pub fn solutions(&self, g: usize, s: usize) -> &BTreeSet<WordIndex> {
        let hint = self.hints_by_secret_by_guess[g][s];
        &self.solutions_by_hint_by_guess[g][hint as usize]
    }
}

fn hint_by_secret_idx_by_guess_idx(words: &[Word], secret_cnt: usize) -> Vec<Vec<HintValue>> {
    let start = Instant::now();
    let hint_by_secret_by_guess: Vec<Vec<HintValue>> = words
        .par_iter()
        .map(|guess| {
            words
                .iter()
                .take(secret_cnt as usize)
                .map(|secret| guess.calculate_hint(secret).value())
                .collect()
        })
        .collect();
    let elapsed = start.elapsed();
    println!(
        "{:?} to calc {} hint_by_secret_idx_by_guess_idx",
        elapsed,
        hint_by_secret_by_guess.len()
    );
    hint_by_secret_by_guess
}

fn solution_indices_by_hint_by_guess_idx(
    words: &[Word],
    secret_cnt: usize,
    hint_by_secret_by_guess: &[Vec<HintValue>],
) -> Vec<Vec<BTreeSet<WordIndex>>> {
    let start = Instant::now();
    let solutions_by_hint_by_guess: Vec<Vec<BTreeSet<WordIndex>>> = words
        .par_iter()
        .enumerate()
        .map(|(guess_idx, _guess)| {
            let mut solutions_by_hint: Vec<BTreeSet<WordIndex>> = vec![BTreeSet::new(); 243];
            (0..secret_cnt).into_iter().for_each(|secret_idx| {
                let hint = hint_by_secret_by_guess[guess_idx][secret_idx];
                solutions_by_hint[hint as usize].insert(secret_idx as WordIndex);
            });
            solutions_by_hint
        })
        .collect();
    let elapsed = start.elapsed();
    println!(
        "{:?} to calc {} solutions_by_hint_by_guess",
        elapsed,
        solutions_by_hint_by_guess.len()
    );
    solutions_by_hint_by_guess
}
