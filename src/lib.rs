use std::cmp::{Ordering, Reverse};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::{Display, Formatter};
use std::io;

use rayon::prelude::*;

use Hint::*;

use crate::cache::{Cache, SecretIndices, WordIndex};
use crate::words::Language::*;
use crate::words::{Language, ToWord, Word, Words};

pub const MAX_ATTEMPTS: usize = 6;
const AUTOPLAY_MAX_ATTEMPTS: usize = 10;
const ALL_POS: [usize; 5] = [0, 1, 2, 3, 4];

type Feedback = Word;
type HintValue = u8;
pub type Attempt = usize;
pub type Count = usize;
type Solutions<'a> = BTreeSet<&'a Word>;

pub mod cache;
pub mod words;

pub struct Wordle {
    words: Words,
    solutions: SecretIndices,
    cache: Cache,
    pub guessed: Vec<WordIndex>,
    print_output: bool,
}
impl Wordle {
    pub fn with(lang: Language) -> Self {
        let words = Words::new(lang);
        let solutions = words.secret_indices();
        let cache = Cache::new(&words);
        Wordle {
            words,
            solutions,
            cache,
            guessed: Vec::new(),
            print_output: true,
        }
    }

    pub fn play(&mut self) {
        let mut strategy = ChainedStrategies::new(
            vec![
                &FirstOfTwoOrFewerRemainingSolutions,
                &WordWithMostNewCharsFromRemainingSolutions,
                &WordThatResultsInFewestRemainingSolutions,
                &WordThatResultsInShortestGameApproximation,
            ],
            PickFirstSolution,
        );

        while self.solutions.len() != 1 {
            self.print_remaining_word_count();

            let suggestion = strategy.pick(self);
            let guess = self.ask_for_guess(suggestion);
            let feedback = self.ask_for_feedback(&guess);
            let hint = Hints::from_feedback(feedback).value();

            let guess_idx = self.words.index_of(&guess);
            self.update_remaining_solutions(guess_idx, hint);
            self.guessed.push(guess_idx);
        }
        self.print_result();
    }

    fn update_remaining_solutions(&mut self, guess: WordIndex, hint: HintValue) {
        let solutions = self.cache.solutions_by(guess, hint);
        self.solutions = self.solutions.intersect(solutions);
    }

    #[allow(clippy::ptr_arg)] // to_string is implemented for Word but not &[char]
    pub fn autoplay(&mut self, secret: &Word, mut strategy: impl PickWord) {
        self.print_output = false;

        while self.guessed.len() < AUTOPLAY_MAX_ATTEMPTS {
            let guess: Word = strategy.pick(self);

            let guess = self.words.index_of(&guess);
            self.guessed.push(guess);

            let secret = self.words.index_of(secret);
            let hint = self.cache.hint(guess, secret);

            self.print_state(guess, secret, hint);
            if guess == secret {
                self.solutions = [secret].into_iter().collect();
                break;
            } else {
                self.update_remaining_solutions(guess, hint);
            }
        }

        if self.solutions.len() != 1 {
            println!(
                "{:4} solutions left, after {} guesses to find secret {}. :(",
                self.solutions.len(),
                self.guessed.len(),
                secret.to_string()
            );
        }
    }

    fn print_state(&self, guess: WordIndex, secret: WordIndex, hint: HintValue) {
        println!(
            "{:4} solutions left, {}. guess {}, hint {}, secret {}",
            self.solutions.len(),
            self.guessed.len() + 1,
            self.words.get_string(guess),
            Hints::from(hint),
            self.words.get_string(secret),
        );
    }

    fn print_remaining_word_count(&self) {
        let len = self.solutions.len();
        if len > 10 {
            println!("\n{} words left", len);
        } else {
            println!("\n{} words left: {}", len, self.solutions().sorted_string());
        }
    }

    fn ask_for_feedback(&mut self, guess: &[char]) -> Word {
        loop {
            println!(
                "Enter feedback using upper-case for correct and lower-case for wrong positions,\n\
            or any non-alphabetic for illegal:"
            );
            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();
            let feedback = input.to_word();

            if feedback.len() > 5 {
                println!("Enter at most 5 characters!")
            } else if self.words.lang() == &Primal {
                return feedback;
            } else if let Some(illegal) = feedback
                .iter()
                .filter(|c| c.is_ascii_alphabetic())
                .find(|c| !guess.contains(&c.to_ascii_lowercase()))
            {
                println!("Character '{}' was not part of the guess!", illegal);
            } else {
                return feedback;
            }
        }
    }

    fn ask_for_guess(&self, suggestion: Word) -> Word {
        println!(
            "Enter your guess, or press enter to use the suggestion {}:",
            suggestion.to_string()
        );
        loop {
            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();
            if input.trim().is_empty() {
                return suggestion;
            }
            let guess = input
                .trim()
                .chars()
                .map(|c| c.to_ascii_lowercase())
                .collect::<Word>();
            if !self.allowed().iter().any(|word| word == &&guess) {
                println!("This word is not allowed, please enter another one, or nothing to use the suggestion:")
            } else {
                return guess;
            }
        }
    }

    fn open_positions(&self) -> Vec<usize> {
        (0..5)
            .into_iter()
            .filter(|i| {
                // If there's only one char in a position it's no longer open
                1 != self
                    .solutions
                    .iter()
                    .map(|&i| self.words.get(i).to_vec())
                    .map(|w| w[*i])
                    .collect::<HashSet<char>>()
                    .len()
            })
            .collect()
    }

    fn allowed(&self) -> Vec<&Word> {
        let guessed_words = self.guessed_words();
        self.words
            .guesses()
            .iter()
            .filter(|guess| !guessed_words.contains(guess))
            .collect()
    }

    fn guessed_words(&self) -> Vec<Word> {
        self.words.indices_to_words(self.guessed.iter())
    }

    fn print_result(&self) {
        if self.solutions.len() == 1 {
            println!(
                "\nThe word is {}",
                self.words
                    .get(*self.solutions.iter().next().unwrap())
                    .to_string()
            );
        } else {
            println!("No matching word in the solutions!");
        }
    }

    fn wanted_chars(&self) -> HashSet<char> {
        self.chars_in_possible_solutions()
            .difference(&self.guessed_chars())
            .cloned()
            .collect()
    }

    fn chars_in_possible_solutions(&self) -> HashSet<char> {
        self.solutions().into_iter().flatten().copied().collect()
    }

    fn guessed_chars(&self) -> HashSet<char> {
        self.guessed_words().into_iter().flatten().collect()
    }

    fn solutions(&self) -> Solutions {
        self.solutions.iter().map(|s| self.words.get(*s)).collect()
    }
}

trait WordAsCharVec {
    fn to_string(&self) -> String;
    fn unique_chars_in(&self, positions: &[usize]) -> HashSet<char>;
    fn chars_in(&self, positions: &[usize]) -> Vec<char>;
    fn char_position_set(&self, positions: &[usize]) -> HashSet<(usize, char)>;
    fn total_char_count(&self, positions: &[usize], ch: &char) -> usize;
}
impl WordAsCharVec for Word {
    fn to_string(&self) -> String {
        format!("'{}'", self.iter().collect::<String>())
    }
    fn unique_chars_in(&self, positions: &[usize]) -> HashSet<char> {
        positions.iter().map(|&pos| self[pos]).collect()
    }
    fn chars_in(&self, positions: &[usize]) -> Vec<char> {
        positions.iter().map(|&pos| self[pos]).collect()
    }
    fn char_position_set(&self, positions: &[usize]) -> HashSet<(usize, char)> {
        positions.iter().map(|&pos| (pos, self[pos])).collect()
    }
    fn total_char_count(&self, positions: &[usize], ch: &char) -> usize {
        positions.iter().filter(|&&pos| &self[pos] == ch).count()
    }
}

trait CharFrequencyToString {
    fn to_string(&self) -> String;
}
impl CharFrequencyToString for HashMap<char, usize> {
    fn to_string(&self) -> String {
        let mut freq: Vec<_> = self.iter().collect();
        freq.sort_unstable_by_key(|(_, i)| Reverse(*i));
        freq.iter()
            .map(|(c, i)| format!("{} {}", i, c))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

trait WordsToString {
    fn to_string(&self) -> String;
}
impl<W: AsRef<Word>> WordsToString for Vec<W> {
    fn to_string(&self) -> String {
        to_string(self.iter())
    }
}
impl<W: AsRef<Word>> WordsToString for &[W] {
    fn to_string(&self) -> String {
        to_string(self.iter())
    }
}
impl<W: AsRef<Word>> WordsToString for HashSet<W> {
    fn to_string(&self) -> String {
        to_string(self.iter())
    }
}
impl<W: AsRef<Word>> WordsToString for BTreeSet<W> {
    fn to_string(&self) -> String {
        to_string(self.iter())
    }
}
fn to_string<W: AsRef<Word>>(words: impl Iterator<Item = W>) -> String {
    words
        .map(|w| w.as_ref().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

trait WordsToSortedString {
    fn sorted_string(&self) -> String;
}
fn to_sorted_string<W: AsRef<Word>>(words: impl Iterator<Item = W>) -> String {
    let mut words: Vec<_> = words.map(|w| w.as_ref().to_string()).collect();
    words.sort_unstable();
    words.join(", ")
}
impl<W: AsRef<Word>> WordsToSortedString for Vec<W> {
    fn sorted_string(&self) -> String {
        to_sorted_string(self.iter())
    }
}
impl<W: AsRef<Word>> WordsToSortedString for &[W] {
    fn sorted_string(&self) -> String {
        to_sorted_string(self.iter())
    }
}
impl<W: AsRef<Word>> WordsToSortedString for HashSet<W> {
    fn sorted_string(&self) -> String {
        to_sorted_string(self.iter())
    }
}
impl<W: AsRef<Word>> WordsToSortedString for BTreeSet<W> {
    fn sorted_string(&self) -> String {
        to_sorted_string(self.iter())
    }
}

trait CharacterCounts {
    /// Global character frequency
    fn global_character_counts_in(&self, positions: &[usize]) -> HashMap<char, usize>;

    /// Character counts per position
    fn character_counts_per_position_in(&self, positions: &[usize]) -> [HashMap<char, usize>; 5];
}
impl<W: AsRef<Word>> CharacterCounts for Vec<W> {
    fn global_character_counts_in(&self, positions: &[usize]) -> HashMap<char, usize> {
        let mut counts: HashMap<char, usize> = HashMap::new();
        for word in self.iter() {
            for i in positions {
                *counts.entry(word.as_ref()[*i]).or_default() += 1;
            }
        }
        counts
    }
    fn character_counts_per_position_in(&self, positions: &[usize]) -> [HashMap<char, usize>; 5] {
        let empty = || ('a'..='z').into_iter().map(|c| (c, 0)).collect();
        let mut counts: [HashMap<char, usize>; 5] = [empty(), empty(), empty(), empty(), empty()];
        for word in self.iter() {
            for i in positions {
                *counts[*i].get_mut(&word.as_ref()[*i]).unwrap() += 1;
            }
        }
        counts
    }
}
impl<W: AsRef<Word>> CharacterCounts for HashSet<W> {
    fn global_character_counts_in(&self, positions: &[usize]) -> HashMap<char, usize> {
        let mut counts: HashMap<char, usize> = HashMap::new();
        for word in self.iter() {
            for i in positions {
                *counts.entry(word.as_ref()[*i]).or_default() += 1;
            }
        }
        counts
    }
    fn character_counts_per_position_in(&self, positions: &[usize]) -> [HashMap<char, usize>; 5] {
        let empty = || ('a'..='z').into_iter().map(|c| (c, 0)).collect();
        let mut counts: [HashMap<char, usize>; 5] = [empty(), empty(), empty(), empty(), empty()];
        for word in self.iter() {
            for i in positions {
                *counts[*i].get_mut(&word.as_ref()[*i]).unwrap() += 1;
            }
        }
        counts
    }
}

impl<W: AsRef<Word>> CharacterCounts for BTreeSet<W> {
    fn global_character_counts_in(&self, positions: &[usize]) -> HashMap<char, usize> {
        let mut counts: HashMap<char, usize> = HashMap::new();
        for word in self.iter() {
            for i in positions {
                *counts.entry(word.as_ref()[*i]).or_default() += 1;
            }
        }
        counts
    }
    fn character_counts_per_position_in(&self, positions: &[usize]) -> [HashMap<char, usize>; 5] {
        let empty = || ('a'..='z').into_iter().map(|c| (c, 0)).collect();
        let mut counts: [HashMap<char, usize>; 5] = [empty(), empty(), empty(), empty(), empty()];
        for word in self.iter() {
            for i in positions {
                *counts[*i].get_mut(&word.as_ref()[*i]).unwrap() += 1;
            }
        }
        counts
    }
}

trait WordScoresToString<T: PartialOrd + Copy> {
    fn to_string(&self, count: usize) -> String;
}
impl<T: PartialOrd + Copy + Display> WordScoresToString<T> for Vec<(&Word, T)> {
    fn to_string(&self, count: usize) -> String {
        self.iter()
            .take(count)
            .map(|(word, value)| format!("{:.3} {}", value, word.to_string()))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

trait WordIndexScoresToString<T: PartialOrd + Copy> {
    fn to_string(&self, count: usize, words: &Words) -> String;
}
impl<T: PartialOrd + Copy + Display> WordIndexScoresToString<T> for Vec<(WordIndex, T)> {
    fn to_string(&self, count: usize, words: &Words) -> String {
        self.iter()
            .take(count)
            .map(|(idx, value)| format!("{:.3} {}", value, words.get_string(*idx)))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

trait ScoreTrait<T: PartialOrd + Copy, W: Ord + Clone> {
    fn sort_asc(&mut self);
    fn sort_desc(&mut self);
    fn lowest_pair(&self) -> Option<(T, W)>;
    fn highest_pair(&self) -> Option<(T, W)>;

    fn lowest(&self) -> Option<W> {
        self.lowest_pair().map(|(_, word)| word)
    }
    fn highest(&self) -> Option<W> {
        self.highest_pair().map(|(_, word)| word)
    }

    fn lowest_score(&self) -> Option<T> {
        self.lowest_pair().map(|(score, _)| score)
    }
    fn highest_score(&self) -> Option<T> {
        self.highest_pair().map(|(score, _)| score)
    }
}
impl<T: PartialOrd + Copy + Display, W: Ord + Clone> ScoreTrait<T, W> for Vec<(W, T)> {
    fn sort_asc(&mut self) {
        self.sort_unstable_by(|(a_word, a_value), (b_word, b_value)| {
            match a_value.partial_cmp(b_value) {
                Some(Ordering::Equal) | None => a_word.cmp(b_word),
                Some(by_value) => by_value,
            }
        });
    }
    fn sort_desc(&mut self) {
        self.sort_unstable_by(|(a_word, a_value), (b_word, b_value)| {
            match b_value.partial_cmp(a_value) {
                Some(Ordering::Equal) | None => a_word.cmp(b_word),
                Some(by_value) => by_value,
            }
        });
    }
    fn lowest_pair(&self) -> Option<(T, W)> {
        self.iter()
            .min_by(
                |(a_word, a_value), (b_word, b_value)| match a_value.partial_cmp(b_value) {
                    Some(Ordering::Equal) | None => a_word.cmp(b_word),
                    Some(by_value) => by_value,
                },
            )
            .map(|(word, score)| (*score, word.clone()))
    }
    fn highest_pair(&self) -> Option<(T, W)> {
        self.iter()
            .max_by(
                |(a_word, a_value), (b_word, b_value)| match a_value.partial_cmp(b_value) {
                    Some(Ordering::Equal) | None => b_word.cmp(a_word),
                    Some(by_value) => by_value,
                },
            )
            .map(|(word, score)| (*score, word.clone()))
    }
}

pub trait PickWord {
    fn pick(&mut self, game: &Wordle) -> Word;
}

pub struct PickFirstSolution;
impl PickWord for PickFirstSolution {
    fn pick(&mut self, game: &Wordle) -> Word {
        game.solutions
            .iter()
            .next()
            .map(|s| game.words.get(*s).to_vec())
            .unwrap()
    }
}

pub struct ChainedStrategies<'a, F: PickWord> {
    strategies: Vec<&'a dyn TryToPickWord>,
    fallback: F,
}
impl<'a, F: PickWord> ChainedStrategies<'a, F> {
    pub fn new(strategies: Vec<&'a dyn TryToPickWord>, fallback: F) -> Self {
        ChainedStrategies {
            strategies,
            fallback,
        }
    }
}
impl<'a, F: PickWord> PickWord for ChainedStrategies<'a, F> {
    fn pick(&mut self, game: &Wordle) -> Word {
        for strategy in &self.strategies {
            if let Some(word) = strategy.pick(game) {
                return word;
            }
        }
        if game.print_output {
            println!("Using fallback");
        }
        self.fallback.pick(game)
    }
}

pub trait TryToPickWord {
    fn pick(&self, game: &Wordle) -> Option<Word>;
}

struct WordThatResultsInShortestGameApproximation;
impl TryToPickWord for WordThatResultsInShortestGameApproximation {
    fn pick(&self, game: &Wordle) -> Option<Word> {
        let picks = 20;
        let mut scores: Vec<_> = turn_sums(
            &game.words,
            &game.solutions,
            &game.guessed,
            &game.cache,
            picks,
            false,
        );
        if game.print_output {
            scores.sort_asc();
            println!(
                "Best (fewest remaining solutions): {}",
                game.words.scores_to_string(&scores, picks)
            );
        }
        scores.lowest().map(|i| game.words.get(i).to_vec())
    }
}
/// Returns a turn sum for each guess. The expected number of turns
/// for a guess is this sum divided by number of solutions left
fn turn_sums(
    words: &Words,
    remaining_solutions: &SecretIndices,
    guessed: &[WordIndex],
    cache: &Cache,
    picks: usize,
    log: bool,
) -> Vec<(WordIndex, usize)> {
    if remaining_solutions.len() <= 2 {
        return trivial_turn_sum(words, remaining_solutions, guessed, log);
    } else if guessed.len() >= 6 {
        return vec![(0, usize::MAX)];
    }
    let best: Vec<_> = fewest_remaining_solutions(words, remaining_solutions, guessed, cache);
    // let mut sum_by_solutions_cache: HashMap<SolutionsIndex, usize> = HashMap::new();

    let mut scores: Vec<(WordIndex, usize)> = best
        .into_par_iter()
        .enumerate()
        .take(picks) // only the best ${picks} guesses
        .inspect(|&(i, (guess, score))| {
            if log {
                println!(
                    "{}pre  {}/{picks} ({}.) guess {} reduces {} solutions to {score:.3}",
                    "\t".repeat(guessed.len()),
                    i + 1,
                    guessed.len() + 1,
                    words.get_string(guess),
                    remaining_solutions.len(),
                );
            }
        })
        .map(|(i, (guess, score))| {
            let mut guessed = guessed.to_vec();
            guessed.push(guess);

            let sum: usize = if true {
                let mut solutions_by_hint: HashMap<HintValue, SecretIndices> = HashMap::new();
                for &secret in remaining_solutions {
                    let hint = cache.hint(guess, secret);
                    solutions_by_hint.entry(hint).or_default().insert(secret);
                }
                solutions_by_hint
                    .into_par_iter()
                    .map(|(_k, v)| v)
                    .map(|rem_solutions: SecretIndices| {
                        let guess_is_a_solution = rem_solutions.contains(&guess);
                        if rem_solutions.len() == 1 {
                            if guess_is_a_solution {
                                guessed.len()
                            } else {
                                guessed.len() + 1
                            }
                        } else if rem_solutions.len() == 2 {
                            if guess_is_a_solution {
                                2 * guessed.len() + 1
                            } else {
                                2 * (guessed.len() + 1) + 1
                            }
                        } else {
                            turn_sums(words, &rem_solutions, &guessed, cache, picks, log)
                                .lowest_score()
                                .unwrap()
                        }
                    })
                    .sum()
            } else {
                cache
                    .solutions_by_hint_for(guess)
                    .iter()
                    .filter(|solutions| !solutions.is_empty())
                    .map(|solutions| solutions.intersect(remaining_solutions))
                    .filter(|intersection| !intersection.is_empty())
                    .map(|intersection| {
                        if intersection.len() == 1 {
                            guessed.len() + 1
                        } else if intersection.len() == 2 {
                            2 * (guessed.len() + 1) + 1
                        // } else if intersection.len() > 3
                        //     && sum_by_solutions_cache.contains_key(&intersection)
                        // {
                        //     *sum_by_solutions_cache.get(&intersection).unwrap()
                        } else {
                            let min_score =
                                turn_sums(words, &intersection, &guessed, cache, picks, log)
                                    .lowest_score()
                                    .unwrap();
                            // sum_by_solutions_cache.insert(intersection, min_score);
                            min_score
                        }
                    })
                    .sum()
            };
            if log {
                println!(
                    "{}post {}/{} ({}.) guess {} reduces {} solutions to {:.3}, sub-sum {}",
                    "\t".repeat(guessed.len() - 1),
                    i + 1,
                    picks,
                    guessed.len(),
                    words.get_string(guess),
                    remaining_solutions.len(),
                    score,
                    sum
                );
            }
            (guess, sum)
        })
        .collect();
    scores.sort_asc();
    scores
}

fn trivial_turn_sum(
    words: &Words,
    secrets: &SecretIndices,
    guessed: &[WordIndex],
    log: bool,
) -> Vec<(WordIndex, usize)> {
    let secrets_count = secrets.len();
    assert!(secrets_count <= 2);
    let mut secrets = secrets.iter();
    let first = *secrets.next().unwrap();

    let this_turn = guessed.len() + 1;
    return if secrets_count == 1 {
        // With only one solution left, the optimal "strategy" picks it
        if log {
            println!(
                "{}{}. guess is the solution: {}",
                "\t".repeat(this_turn - 1),
                this_turn,
                words.get_string(first)
            );
        }
        vec![(first, this_turn)]
    } else {
        let second = *secrets.next().unwrap();
        let next_turn = this_turn + 1;
        let sum = this_turn + next_turn;
        // With two remaining solutions there is no better strategy than choosing either,
        // which will be right 50% of the time. The optimal sum of turns is 3, one turn if picked
        // correctly, and 2 turns if not. The average would be 1.5.
        if log {
            println!(
                "{}{}. guess or {}. guess is the solution: {} or {}",
                "\t".repeat(this_turn - 1),
                next_turn,
                this_turn,
                words.get_string(first),
                words.get_string(second),
            );
        }
        vec![(first, sum), (second, sum)]
    };
}

struct WordThatResultsInFewestRemainingSolutions;
impl TryToPickWord for WordThatResultsInFewestRemainingSolutions {
    fn pick(&self, game: &Wordle) -> Option<Word> {
        // Use this for first guess, because the more complicated
        // methods are too slow with all possible solutions
        if !game.guessed.is_empty() {
            return None;
        }
        let mut scores =
            fewest_remaining_solutions(&game.words, &game.solutions, &game.guessed, &game.cache);
        if game.print_output {
            scores.sort_asc();
            println!(
                "Best (fewest remaining solutions): {}",
                scores.to_string(5, &game.words)
            );
        }
        scores.lowest().map(|i| game.words.get(i).clone())
    }
}

fn fewest_remaining_solutions(
    words: &Words,
    solutions: &SecretIndices,
    guessed: &[WordIndex],
    cache: &Cache,
) -> Vec<(WordIndex, f64)> {
    let is_first_turn = solutions.len() as WordIndex == words.secret_count();
    let solution_count = solutions.len() as f64;
    let mut scores: Vec<(WordIndex, f64)> = words
        .guess_indices()
        .into_par_iter()
        .filter(|guess_idx| !guessed.contains(guess_idx))
        .map(|guess| {
            let sum: usize = solutions
                .iter()
                .map(|&secret| {
                    if guess == secret {
                        0
                    } else if is_first_turn {
                        cache.solutions(guess, secret).len()
                    } else {
                        cache
                            .solutions(guess, secret)
                            .intersection(solutions)
                            .count()
                    }
                })
                .sum();
            (guess as WordIndex, sum as f64 / solution_count)
        })
        .collect();
    scores.sort_unstable_by(|(a_idx, a_score), (b_idx, b_score)| {
        match a_score.partial_cmp(b_score) {
            Some(Ordering::Equal) | None => a_idx.cmp(b_idx),
            Some(by_value) => by_value,
        }
    });
    scores
}

struct MostFrequentGlobalCharacter;
impl TryToPickWord for MostFrequentGlobalCharacter {
    fn pick(&self, game: &Wordle) -> Option<Word> {
        let positions = &game.open_positions();
        let freq = game
            .solutions
            .iter()
            .map(|i| game.words.get(*i))
            .collect::<Solutions>()
            .global_character_counts_in(positions);

        let scores: Vec<_> = game
            .solutions
            .iter()
            .map(|i| game.words.get(*i))
            .map(|secret| {
                let count = secret
                    .unique_chars_in(positions)
                    .iter()
                    .map(|c| freq[c])
                    .sum::<usize>();
                (secret, count)
            })
            .collect();
        // scores.sort_desc();
        // println!("{}", scores.to_string());
        // ScoreTrait::highest(&scores)
        scores.highest().cloned()
    }
}
struct MostFrequentGlobalCharacterHighVarietyWord;
impl TryToPickWord for MostFrequentGlobalCharacterHighVarietyWord {
    fn pick(&self, game: &Wordle) -> Option<Word> {
        let positions = &game.open_positions();
        let solutions = game.solutions();
        let high_variety_words = solutions.high_variety_words(positions);
        if high_variety_words.is_empty() {
            return None;
        }
        let freq = high_variety_words.global_character_counts_in(positions);
        // println!("Overall character counts: {}", freq.to_string());

        let scores: Vec<(&Word, usize)> = high_variety_words
            .into_iter()
            .map(|word| {
                let count = word
                    .unique_chars_in(positions)
                    .iter()
                    .map(|c| freq[c])
                    .sum::<usize>();
                (word, count)
            })
            .collect();
        if scores.is_empty() {
            return None;
        }
        scores.highest().cloned()
    }
}

pub struct FixedGuessList {
    guesses: Vec<Word>,
}
impl FixedGuessList {
    pub fn new<S: AsRef<str>>(guesses: Vec<S>) -> Self {
        let guesses: Vec<Word> = guesses.into_iter().map(|w| w.as_ref().to_word()).collect();
        println!("Fixed guesses {}\n", guesses.to_string());
        FixedGuessList { guesses }
    }
}

impl TryToPickWord for FixedGuessList {
    fn pick(&self, game: &Wordle) -> Option<Word> {
        self.guesses.get(game.guessed.len()).cloned()
    }
}

struct MostFrequentCharactersOfRemainingWords;
impl TryToPickWord for MostFrequentCharactersOfRemainingWords {
    fn pick(&self, game: &Wordle) -> Option<Word> {
        let remaining_words_with_unique_new_chars: Vec<_> = game
            .allowed()
            .iter()
            .filter(|&word| {
                word.unique_chars_in(&ALL_POS).len() == 5
                    && !word.iter().any(|c| game.guessed_chars().contains(c))
            })
            .cloned()
            .collect();

        let positions = &ALL_POS;
        let freq = remaining_words_with_unique_new_chars.global_character_counts_in(positions);
        let scores: Vec<_> = remaining_words_with_unique_new_chars
            .into_iter()
            .map(|word| {
                let count = word
                    .unique_chars_in(positions)
                    .iter()
                    .map(|c| freq[c])
                    .sum::<usize>();
                (word, count)
            })
            .collect();
        scores.highest().cloned()
    }
}

pub struct FirstOfTwoOrFewerRemainingSolutions;
impl TryToPickWord for FirstOfTwoOrFewerRemainingSolutions {
    fn pick(&self, game: &Wordle) -> Option<Word> {
        if game.solutions.len() <= 2 {
            game.solutions
                .iter()
                .next()
                .map(|i| game.words.get(*i).to_vec())
        } else {
            None
        }
    }
}

pub struct WordWithMostNewCharsFromRemainingSolutions;
impl TryToPickWord for WordWithMostNewCharsFromRemainingSolutions {
    fn pick(&self, game: &Wordle) -> Option<Word> {
        let wanted_char_count = game.wanted_chars().len();
        if !(1..=5).contains(&wanted_char_count) {
            return None;
        }
        let scores: Vec<(&Word, usize)> =
            words_with_most_wanted_chars(&game.wanted_chars(), game.allowed());
        if game.print_output {
            println!(
                "Words with most of wanted chars {:?} are:\n  {}",
                game.wanted_chars(),
                scores.to_string(5)
            );
        }
        scores.highest().cloned()
    }
}

struct MostFrequentCharacterPerPos;
impl TryToPickWord for MostFrequentCharacterPerPos {
    fn pick(&self, game: &Wordle) -> Option<Word> {
        let positions = &game.open_positions();
        let counts = game.solutions().character_counts_per_position_in(positions);

        let scores: Vec<_> = game
            .solutions
            .iter()
            .map(|i| game.words.get(*i))
            .map(|secret| {
                let count = positions
                    .iter()
                    .map(|&i| counts[i][&secret[i]])
                    .sum::<usize>();
                (secret, count)
            })
            .collect();
        scores.highest().cloned()
    }
}

struct MostFrequentCharacterPerPosHighVarietyWord;
impl TryToPickWord for MostFrequentCharacterPerPosHighVarietyWord {
    fn pick(&self, game: &Wordle) -> Option<Word> {
        let positions = &game.open_positions();
        let solutions = game.solutions();
        let high_variety_words = solutions.high_variety_words(positions);
        if high_variety_words.is_empty() {
            return None;
        }
        let counts = high_variety_words.character_counts_per_position_in(positions);

        let scores: Vec<_> = high_variety_words
            .into_iter()
            .map(|word| {
                let count = positions
                    .iter()
                    .map(|&i| counts[i][&word[i]])
                    .sum::<usize>();
                (word, count)
            })
            .collect();
        scores.highest().cloned()
    }
}

struct MatchingMostOtherWordsInAtLeastOneOpenPosition;
impl TryToPickWord for MatchingMostOtherWordsInAtLeastOneOpenPosition {
    fn pick(&self, game: &Wordle) -> Option<Word> {
        let positions = &game.open_positions();
        // for each word, find out how many other words it matches in any open position
        let mut scores: Vec<_> = game
            .solutions
            .iter()
            .map(|i| game.words.get(*i))
            .map(|secret| (secret, 0_usize))
            .collect();
        let open_chars: Vec<_> = game
            .solutions
            .iter()
            .map(|i| game.words.get(*i))
            .map(|word| word.chars_in(positions))
            .collect();
        for (i, chars_a) in open_chars.iter().enumerate().take(open_chars.len() - 1) {
            for (j, chars_b) in open_chars.iter().enumerate().skip(i + 1) {
                assert_ne!(i, j);
                assert_ne!(chars_a, chars_b);
                let any_open_position_matches_exactly =
                    chars_a.iter().zip(chars_b.iter()).any(|(a, b)| a == b);
                if any_open_position_matches_exactly {
                    scores[i].1 += 1;
                    scores[j].1 += 1;
                }
            }
        }

        // scores.sort_asc();
        // println!("Matching fewest words: {}", scores.to_string());
        //
        // scores.sort_desc();
        // println!("Matching most words:   {}", scores.to_string());

        // Worst 10
        // 320 lymph, 314 igloo, 313 umbra, 310 unzip, 308 affix,
        // 304 ethos, 301 jumbo, 298 ethic, 282 nymph, 279 inbox

        // Best 10
        // 1077 slate, 1095 sooty, 1097 scree, 1098 gooey, 1099 spree,
        // 1100 sense, 1104 saute, 1114 soapy, 1115 saucy, 1122 sauce

        scores.highest().cloned()
    }
}
struct MatchingMostOtherWordsInAtLeastOneOpenPositionHighVarietyWord;
impl TryToPickWord for MatchingMostOtherWordsInAtLeastOneOpenPositionHighVarietyWord {
    fn pick(&self, game: &Wordle) -> Option<Word> {
        let positions = &game.open_positions();
        let solutions = game.solutions();
        let high_variety_words = solutions.high_variety_words(positions);
        if high_variety_words.is_empty() {
            return None;
        }
        // for each word, find out how many other words it matches in any open position
        let mut scores: Vec<_> = high_variety_words
            .iter()
            .map(|&word| (word, 0_usize))
            .collect();
        let open_chars: Vec<_> = high_variety_words
            .iter()
            .map(|word| word.chars_in(positions))
            .collect();
        for (i, chars_a) in open_chars.iter().enumerate().take(open_chars.len() - 1) {
            for (j, chars_b) in open_chars.iter().enumerate().skip(i + 1) {
                assert_ne!(i, j);
                assert_ne!(chars_a, chars_b);
                let any_open_position_matches_exactly =
                    chars_a.iter().zip(chars_b.iter()).any(|(a, b)| a == b);
                if any_open_position_matches_exactly {
                    scores[i].1 += 1;
                    scores[j].1 += 1;
                }
            }
        }
        scores.highest().cloned()
    }
}

trait HighVarietyWords {
    fn high_variety_words(&self, open_positions: &[usize]) -> Vec<&Word>;
}
impl<W: AsRef<Word> + Clone> HighVarietyWords for Vec<W> {
    fn high_variety_words(&self, open_positions: &[usize]) -> Vec<&Word> {
        high_variety_words(self.iter().map(|w| w.as_ref()), open_positions)
    }
}
impl<W: AsRef<Word>> HighVarietyWords for HashSet<W> {
    fn high_variety_words(&self, open_positions: &[usize]) -> Vec<&Word> {
        high_variety_words(self.iter().map(|w| w.as_ref()), open_positions)
    }
}
impl<W: AsRef<Word>> HighVarietyWords for BTreeSet<W> {
    fn high_variety_words(&self, open_positions: &[usize]) -> Vec<&Word> {
        high_variety_words(self.iter().map(|w| w.as_ref()), open_positions)
    }
}
fn high_variety_words<'a>(
    it: impl Iterator<Item = &'a Word>,
    open_positions: &[usize],
) -> Vec<&'a Word> {
    it.filter(|&word| word.unique_chars_in(open_positions).len() == open_positions.len())
        .collect()
}

trait CalcHintValue {
    fn value(&self) -> HintValue;
}

#[derive(Copy, Clone)]
enum Hint {
    Illegal,  // Not in the word at all
    WrongPos, // In word but at other position
    Correct,  // Correct at this position
}
impl CalcHintValue for Hint {
    fn value(&self) -> HintValue {
        match self {
            Illegal => 0,
            WrongPos => 1,
            Correct => 2,
        }
    }
}
impl Hint {
    fn from_feedback(feedback: char) -> Self {
        if feedback.is_ascii_lowercase() {
            WrongPos
        } else if feedback.is_ascii_uppercase() {
            Correct
        } else {
            Illegal
        }
    }
}
impl From<char> for Hint {
    fn from(c: char) -> Self {
        match c {
            '⬛' => Illegal,
            '🟨' => WrongPos,
            '🟩' => Correct,
            _ => unreachable!("Illegal hint {}", c),
        }
    }
}
impl From<HintValue> for Hint {
    fn from(v: u8) -> Self {
        match v {
            0 => Illegal,
            1 => WrongPos,
            2 => Correct,
            _ => unreachable!("Illegal hint value {}", v),
        }
    }
}
impl Display for Hint {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Illegal => "⬛",
                WrongPos => "🟨",
                Correct => "🟩",
            }
        )
    }
}

struct Hints {
    hints: Vec<Hint>,
}
impl Default for Hints {
    fn default() -> Self {
        Hints {
            hints: vec![Illegal; 5],
        }
    }
}
impl Hints {
    fn set_correct(&mut self, i: usize) {
        self.hints[i] = Correct;
    }
    fn set_wrong_pos(&mut self, i: usize) {
        self.hints[i] = WrongPos;
    }
    fn from_feedback(feedback: Feedback) -> Self {
        let hints = feedback.into_iter().map(Hint::from_feedback).collect();
        Hints { hints }
    }
}
impl CalcHintValue for Hints {
    fn value(&self) -> HintValue {
        [81, 27, 9, 3, 1]
            .into_iter()
            .zip(&self.hints)
            .map(|(multiplier, h)| multiplier * h.value())
            .sum::<HintValue>()
    }
}
impl Display for Hints {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            self.hints
                .iter()
                .map(|ch| ch.to_string())
                .collect::<String>()
        )
    }
}
impl From<&str> for Hints {
    fn from(hint: &str) -> Self {
        Hints {
            hints: hint.chars().map(Hint::from).collect(),
        }
    }
}
impl From<u8> for Hints {
    fn from(mut hint_value: u8) -> Self {
        const MULTIPLIERS: [HintValue; 5] = [81, 27, 9, 3, 1];
        let mut hints = vec![];
        for multi in MULTIPLIERS.into_iter() {
            let hint = hint_value / multi;
            hint_value %= multi;
            hints.push(Hint::from(hint));
        }
        Hints { hints }
    }
}

trait GetHint {
    /// Each guessed word results in a hint that depends on the secret word.
    /// For each character in a guess there are 3 options:
    /// - The solution contains it in exactly this position: 🟩, given value 2
    /// - The solution contains it, but a different position: 🟨, given value 1
    /// - The solution does not contain this character anywhere: ⬛️, given value 0
    ///
    /// As each of the 5 positions can result in one of 3 states, there are 3^5 = 243 possible hints.
    /// Let's assign a number to each one. We multiply the values 0, 1 or 2 with a multiplier 3 ^ i,
    /// which depends on its index `i` within the word (the first index being 0).
    ///
    /// Returns a hints for the given guess and solution
    fn calculate_hint(&self, secret: &Self) -> Hints;
}
impl GetHint for Word {
    fn calculate_hint(&self, secret: &Self) -> Hints {
        // Initialize as every position incorrect
        let mut hint = Hints::default();

        // Fill in exact matches
        let mut open_positions = vec![];
        for i in 0..5 {
            if self[i] == secret[i] {
                hint.set_correct(i);
            } else {
                open_positions.push(i);
            }
        }

        // For characters at another position, consider only characters not previously matched
        // For example:
        // Guessing "geese" for solution "eject"  matches exactly in the middle 'e', which leaves
        // "ge_se" and "ej_ct". The 'e' at pos 1 of "ge_se" will count as a present char, but the
        // last 'e' in "ge_se" is illegal, because all the 'e's in "ej_ct" were already matched.
        for &i in &open_positions {
            let considered_char_count = |guess: &[char], ch: &char| {
                guess
                    .iter()
                    .take(i + 1) // include current pos
                    .enumerate()
                    .filter(|(i, g)| g == &ch && open_positions.contains(i))
                    .count()
            };
            let char = &self[i];
            if considered_char_count(self, char) <= secret.total_char_count(&open_positions, char) {
                hint.set_wrong_pos(i);
            }
        }
        hint
    }
}
impl GetHint for &str {
    fn calculate_hint(&self, secret: &Self) -> Hints {
        self.to_word().calculate_hint(&secret.to_word())
    }
}

fn words_with_most_wanted_chars<'w>(
    wanted_chars: &HashSet<char>,
    guesses: Vec<&'w Word>,
) -> Vec<(&'w Word, usize)> {
    let mut scores: Vec<_> = guesses
        .into_iter()
        .map(|word| {
            let unique_wanted_char_count = word
                .unique_chars_in(&ALL_POS)
                .into_iter()
                .filter(|c| wanted_chars.contains(c))
                .count();
            (word, unique_wanted_char_count)
        })
        .collect();
    scores.sort_desc();
    scores
}

trait Intersect {
    fn intersect(&self, other: &Self) -> Self;
}
impl Intersect for Solutions<'_> {
    fn intersect(&self, other: &Self) -> Self {
        self.intersection(other).into_iter().cloned().collect()
    }
}
impl Intersect for SecretIndices {
    fn intersect(&self, other: &Self) -> Self {
        self.intersection(other).into_iter().cloned().collect()
    }
}

pub fn autoplay_and_print_stats_with_language<S: TryToPickWord + Sync>(
    strategy: S,
    lang: Language,
) {
    let words = Words::new(lang);
    let mut secrets: Vec<_> = words
        .secrets()
        // .filter(|w| w.to_string().eq("'rowdy'"))
        .collect();
    secrets.sort_unstable();
    let attempts: Vec<usize> = secrets
        .iter()
        .map(|secret| {
            let mut game = Wordle::with(lang);
            let strategy = ChainedStrategies::new(
                vec![
                    &FirstOfTwoOrFewerRemainingSolutions,
                    &WordWithMostNewCharsFromRemainingSolutions,
                    &strategy,
                ],
                PickFirstSolution,
            );
            game.autoplay(secret, strategy);
            game.guessed.len()
        })
        .collect();
    let mut count_by_attempts: BTreeMap<Attempt, Count> = BTreeMap::new();
    for attempt in attempts {
        *count_by_attempts.entry(attempt).or_default() += 1;
    }
    print_stats(count_by_attempts.iter());
}

fn print_stats<'a>(count_by_attempts: impl Iterator<Item = (&'a Attempt, &'a Count)>) {
    let mut games = 0;
    let mut attempts_sum = 0;
    let mut failures = 0;
    let mut descs = vec![];
    for (attempts, count) in count_by_attempts {
        games += count;
        attempts_sum += attempts * count;
        if attempts > &MAX_ATTEMPTS {
            failures += count;
        }
        descs.push(format!("{}: {}", attempts, count));
    }
    let average = attempts_sum as f64 / games as f64;

    print!("\n{:.3} average attempts; {}", average, descs.join(", "));
    if failures > 0 {
        let percent_failed = 100.0 * failures as f64 / games as f64;
        println!("; {} ({:.2}%) failures", failures, percent_failed)
    } else {
        println!();
    }
}

#[cfg(test)]
mod tests;
