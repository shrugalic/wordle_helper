use std::collections::{BTreeSet, HashMap};

use rayon::prelude::*;

use crate::{CalcHintValue, GetHint, HintValue, Word, Words};

pub type Solutions<'a> = BTreeSet<&'a Word>;

pub struct Cache<'a> {
    hint_solutions: &'a SolutionsByHintByGuess<'a>,
    hints: &'a HintsBySecretByGuess<'a>,
    secret_solutions: SolutionsBySecretByGuess<'a>,
}
impl<'a> Cache<'a> {
    pub fn new(
        words: &'a Words,
        hints: &'a HintsBySecretByGuess,
        hint_solutions: &'a SolutionsByHintByGuess,
    ) -> Self {
        let secret_solutions = SolutionsBySecretByGuess::of(words, hints, hint_solutions);
        Cache {
            hint_solutions,
            hints,
            secret_solutions,
        }
    }
    pub(crate) fn solutions_by_hint_by_guess(
        &self,
        guess: &Word,
        hint: &HintValue,
    ) -> &Solutions<'a> {
        &self.hint_solutions.by_hint_by_guess[guess][hint]
    }

    pub(crate) fn solutions_by_hint_for(&self, guess: &Word) -> &HashMap<HintValue, Solutions<'a>> {
        &self.hint_solutions.by_hint_by_guess[guess]
    }
    pub(crate) fn hint_by_secret_by_guess(&self, guess: &Word, secret: &Word) -> HintValue {
        self.hints.by_secret_by_guess[guess][secret]
    }
    #[cfg(test)]
    pub(crate) fn solutions_by_secret_by_guess(
        &self,
    ) -> &HashMap<&'a Word, HashMap<&'a Word, &'a Solutions<'a>>> {
        &self.secret_solutions.by_secret_by_guess
    }
    pub(crate) fn solutions_by(&self, guess: &Word, secret: &Word) -> &'a Solutions<'a> {
        self.secret_solutions.by_secret_by_guess[guess][secret]
    }
}

pub struct HintsBySecretByGuess<'a> {
    by_secret_by_guess: HashMap<&'a Word, HashMap<&'a Word, HintValue>>,
}
impl<'a> HintsBySecretByGuess<'a> {
    pub fn of(words: &'a Words) -> Self {
        HintsBySecretByGuess {
            by_secret_by_guess: words
                .guesses()
                .par_iter()
                .map(|guess| {
                    let hint_value_by_secret = words
                        .secrets()
                        .map(|secret| (secret, guess.calculate_hint(secret).value()))
                        .collect::<HashMap<&Word, HintValue>>();
                    (guess, hint_value_by_secret)
                })
                .collect(),
        }
    }
    #[cfg(test)]
    pub fn by_secret_by_guess(&self) -> &HashMap<&'a Word, HashMap<&'a Word, HintValue>> {
        &self.by_secret_by_guess
    }
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.by_secret_by_guess.len()
    }
}

pub struct SolutionsByHintByGuess<'a> {
    by_hint_by_guess: HashMap<&'a Word, HashMap<HintValue, Solutions<'a>>>,
}
impl<'a> SolutionsByHintByGuess<'a> {
    pub fn of(words: &'a Words, hsg: &'a HintsBySecretByGuess) -> Self {
        SolutionsByHintByGuess {
            by_hint_by_guess: words
                .guesses()
                .par_iter()
                .map(|guess| {
                    let mut solutions_by_hint: HashMap<HintValue, Solutions> = HashMap::new();
                    for secret in words.secrets() {
                        solutions_by_hint
                            .entry(hsg.by_secret_by_guess[guess][secret])
                            .or_default()
                            .insert(secret);
                    }
                    (guess, solutions_by_hint)
                })
                .collect(),
        }
    }
    #[cfg(test)]
    pub fn by_hint_by_guess(&self) -> &HashMap<&'a Word, HashMap<HintValue, Solutions<'a>>> {
        &self.by_hint_by_guess
    }
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.by_hint_by_guess.len()
    }
}

pub struct SolutionsBySecretByGuess<'a> {
    by_secret_by_guess: HashMap<&'a Word, HashMap<&'a Word, &'a Solutions<'a>>>,
}
impl<'a> SolutionsBySecretByGuess<'a> {
    pub(crate) fn of(
        words: &'a Words,
        hsg: &'a HintsBySecretByGuess,
        shg: &'a SolutionsByHintByGuess,
    ) -> Self {
        SolutionsBySecretByGuess {
            by_secret_by_guess: words
                .guesses()
                .par_iter()
                .map(|guess| {
                    let mut solutions_by_secret: HashMap<&Word, &Solutions> = HashMap::new();
                    for secret in words.secrets() {
                        let hint = &hsg.by_secret_by_guess[guess][secret];
                        let solutions = &shg.by_hint_by_guess[guess][hint];
                        solutions_by_secret.insert(secret, solutions);
                    }
                    (guess, solutions_by_secret)
                })
                .collect(),
        }
    }
}
