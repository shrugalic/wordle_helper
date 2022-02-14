use std::cmp::{Ordering, Reverse};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::env::args;
use std::fmt::{Display, Formatter};
use std::io;
use std::iter::Map;
use std::str::Lines;

use rayon::prelude::*;

use Hint::*;
use Language::*;

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

const MAX_ATTEMPTS: usize = 6;
const AUTOPLAY_MAX_ATTEMPTS: usize = 10;
const ALL_POS: [usize; 5] = [0, 1, 2, 3, 4];

type Word = Vec<char>;
type Guess = Word;
type Secret = Word;
type Feedback = Guess;
type HintValue = u8;
type Solutions<'a> = BTreeSet<&'a Secret>;
type Attempt = usize;
type Count = usize;

#[derive(Clone)]
struct Words {
    lang: Language,
    guesses: Vec<Guess>,
    secrets: HashSet<Secret>,
}
impl Words {
    fn new(lang: Language) -> Self {
        Words {
            lang,
            secrets: Words::from_str(SOLUTIONS[lang as usize]).collect(),
            guesses: Words::from_str(GUESSES[lang as usize]).collect(),
        }
    }
    fn from_str(txt: &str) -> Map<Lines<'_>, fn(&'_ str) -> Word> {
        txt.lines().map(|w| w.to_word())
    }
}

struct Wordle<'a> {
    words: &'a Words,
    solutions: Solutions<'a>,
    cache: &'a Cache<'a>,
    guessed: Vec<Guess>,
    print_output: bool,
}
impl<'a> Wordle<'a> {
    fn with(words: &'a Words, cache: &'a Cache) -> Self {
        let solutions = words.secrets.iter().collect();
        Wordle {
            words,
            solutions,
            cache,
            guessed: Vec::new(),
            print_output: true,
        }
    }

    fn play(&mut self) {
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

            self.update_remaining_solutions(&guess, &hint);
            self.guessed.push(guess);
        }
        self.print_result();
    }

    fn update_remaining_solutions(&mut self, guess: &Guess, hint: &HintValue) {
        let solutions = &self.cache.hint_solutions.by_hint_by_guess[guess][hint];
        self.solutions = self.solutions.intersect(solutions);
    }

    #[allow(clippy::ptr_arg)] // to_string is implemented for Word but not &[char]
    fn autoplay(&mut self, secret: &Secret, mut strategy: impl PickWord) {
        self.print_output = false;

        while self.guessed.len() < AUTOPLAY_MAX_ATTEMPTS {
            let guess: Guess = strategy.pick(self);
            let hint = self.cache.hints.by_secret_by_guess[&guess][secret];

            self.print_state(&guess, secret, hint);
            self.guessed.push(guess.clone());
            if guess.eq(secret) {
                self.solutions = self
                    .solutions
                    .iter()
                    .filter(|&&s| guess.eq(s))
                    .cloned()
                    .collect();
                break;
            }
            self.update_remaining_solutions(&guess, &hint);
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

    fn print_state(&self, guess: &Guess, secret: &Secret, hint: HintValue) {
        println!(
            "{:4} solutions left, {}. guess {}, hint {}, secret {}",
            self.solutions.len(),
            self.guessed.len() + 1,
            guess.to_string(),
            Hints::from(hint),
            secret.to_string(),
        );
    }

    fn print_remaining_word_count(&self) {
        let len = self.solutions.len();
        if len > 10 {
            println!("\n{} words left", len);
        } else {
            println!("\n{} words left: {}", len, self.solutions.sorted_string());
        }
    }

    fn ask_for_feedback(&mut self, guess: &[char]) -> Guess {
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
            } else if self.words.lang == Primal {
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
                .collect::<Guess>();
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
                1 == self
                    .solutions
                    .iter()
                    .map(|w| w[*i])
                    .collect::<HashSet<char>>()
                    .len()
            })
            .collect()
    }

    fn allowed(&self) -> Vec<&Guess> {
        self.words
            .guesses
            .iter()
            .filter(|guess| !self.guessed.contains(guess))
            .collect()
    }

    fn print_result(&self) {
        if self.solutions.len() == 1 {
            println!(
                "\nThe word is {}",
                self.solutions.iter().next().unwrap().to_string()
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
        self.solutions.iter().cloned().flatten().copied().collect()
    }

    fn guessed_chars(&self) -> HashSet<char> {
        self.guessed.iter().flatten().copied().collect()
    }
}

struct HintsBySecretByGuess<'a> {
    by_secret_by_guess: HashMap<&'a Guess, HashMap<&'a Secret, HintValue>>,
}
impl<'a> HintsBySecretByGuess<'a> {
    fn of(words: &'a Words) -> Self {
        HintsBySecretByGuess {
            by_secret_by_guess: words
                .guesses
                .par_iter()
                .map(|guess| {
                    let hint_value_by_secret = words
                        .secrets
                        .iter()
                        .map(|secret| (secret, guess.calculate_hint(secret).value()))
                        .collect::<HashMap<&Secret, HintValue>>();
                    (guess, hint_value_by_secret)
                })
                .collect(),
        }
    }
}

struct SolutionsByHintByGuess<'a> {
    by_hint_by_guess: HashMap<&'a Guess, HashMap<HintValue, Solutions<'a>>>,
}
impl<'a> SolutionsByHintByGuess<'a> {
    fn of(words: &'a Words, hsg: &'a HintsBySecretByGuess) -> Self {
        SolutionsByHintByGuess {
            by_hint_by_guess: words
                .guesses
                .par_iter()
                .map(|guess| {
                    let mut solutions_by_hint: HashMap<HintValue, Solutions> = HashMap::new();
                    for secret in &words.secrets {
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
}

struct SolutionsBySecretByGuess<'a> {
    by_secret_by_guess: HashMap<&'a Guess, HashMap<&'a Secret, &'a Solutions<'a>>>,
}
impl<'a> SolutionsBySecretByGuess<'a> {
    fn of(
        words: &'a Words,
        hsg: &'a HintsBySecretByGuess,
        shg: &'a SolutionsByHintByGuess,
    ) -> Self {
        SolutionsBySecretByGuess {
            by_secret_by_guess: words
                .guesses
                .par_iter()
                .map(|guess| {
                    let mut solutions_by_secret: HashMap<&Secret, &Solutions> = HashMap::new();
                    for secret in words.secrets.iter() {
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

trait ScoreTrait<T: PartialOrd + Copy> {
    fn sort_asc(&mut self);
    fn sort_desc(&mut self);
    fn to_string(&self, count: usize) -> String;
    fn lowest_pair(&self) -> Option<(T, Word)>;
    fn highest_pair(&self) -> Option<(T, Word)>;

    fn lowest(&self) -> Option<Word> {
        self.lowest_pair().map(|(_, word)| word)
    }
    fn highest(&self) -> Option<Word> {
        self.highest_pair().map(|(_, word)| word)
    }

    fn lowest_score(&self) -> Option<T> {
        self.lowest_pair().map(|(score, _)| score)
    }
    fn highest_score(&self) -> Option<T> {
        self.highest_pair().map(|(score, _)| score)
    }
}
impl<T: PartialOrd + Copy + Display> ScoreTrait<T> for Vec<(&Word, T)> {
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
    fn to_string(&self, count: usize) -> String {
        self.iter()
            .take(count)
            .map(|(word, value)| format!("{:.3} {}", value, word.to_string()))
            .collect::<Vec<_>>()
            .join(", ")
    }
    fn lowest_pair(&self) -> Option<(T, Word)> {
        self.iter()
            .min_by(
                |(a_word, a_value), (b_word, b_value)| match a_value.partial_cmp(b_value) {
                    Some(Ordering::Equal) | None => a_word.cmp(b_word),
                    Some(by_value) => by_value,
                },
            )
            .map(|&(word, v)| (v, word.clone()))
    }
    fn highest_pair(&self) -> Option<(T, Word)> {
        self.iter()
            .max_by(
                |(a_word, a_value), (b_word, b_value)| match a_value.partial_cmp(b_value) {
                    Some(Ordering::Equal) | None => b_word.cmp(a_word),
                    Some(by_value) => by_value,
                },
            )
            .map(|&(word, v)| (v, word.clone()))
    }
}

trait PickWord {
    fn pick(&mut self, game: &Wordle) -> Word;
}

struct PickFirstSolution;
impl PickWord for PickFirstSolution {
    fn pick(&mut self, game: &Wordle) -> Word {
        game.solutions.iter().next().unwrap().to_vec()
    }
}

struct ChainedStrategies<'a, F: PickWord> {
    strategies: Vec<&'a dyn TryToPickWord>,
    fallback: F,
}
impl<'a, F: PickWord> ChainedStrategies<'a, F> {
    fn new(strategies: Vec<&'a dyn TryToPickWord>, fallback: F) -> Self {
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

trait TryToPickWord {
    fn pick(&self, game: &Wordle) -> Option<Guess>;
}

struct WordThatResultsInShortestGameApproximation;
impl TryToPickWord for WordThatResultsInShortestGameApproximation {
    fn pick(&self, game: &Wordle) -> Option<Guess> {
        let guessed: Vec<_> = game.guessed.iter().collect();
        let picks = 10;
        let mut scores = turn_sums(
            game.words,
            &game.solutions,
            &guessed,
            game.cache,
            picks,
            false,
        );
        if game.print_output {
            scores.sort_asc();
            println!(
                "Best (fewest remaining solutions): {}",
                scores.to_string(picks)
            );
        }
        scores.lowest()
    }
}
/// Returns a turn sum for each guess. The expected number of turns
/// for a guess is this sum divided by number of solutions left
fn turn_sums<'a>(
    words: &'a Words,
    secrets: &'a Solutions,
    guessed: &'a [&Guess],
    cache: &'a Cache,
    picks: usize,
    log: bool,
) -> Vec<(&'a Guess, usize)> {
    if secrets.len() <= 2 {
        return trivial_turn_sum(words, secrets, guessed, log);
    } else if guessed.len() >= 6 {
        panic!();
    }
    let best = fewest_remaining_solutions(words, secrets, guessed, cache);
    let mut sum_by_solutions_cache: HashMap<Solutions, usize> = HashMap::new();
    let mut scores: Vec<(&Guess, usize)> = best
        .into_iter()
        .enumerate()
        .take(picks) // only the best ${picks} guesses
        .inspect(|&(i, (guess, score))| {
            if log {
                println!(
                    "{} {}/{} ({}.) guess {} reduces {} solutions to {:.3}",
                    "\t".repeat(guessed.len()),
                    i + 1,
                    picks,
                    guessed.len() + 1,
                    guess.to_string(),
                    secrets.len(),
                    score,
                );
            }
        })
        .map(|(i, (guess, score))| {
            let mut guessed = guessed.to_vec();
            guessed.push(guess);
            let solutions = &cache.hint_solutions.by_hint_by_guess[guess];
            let sum: usize = solutions
                .iter()
                .map(|(_hint, solutions)| solutions.intersect(secrets))
                .filter(|intersection| !intersection.is_empty())
                .map(|intersection| {
                    if intersection.len() > 3 && sum_by_solutions_cache.contains_key(&intersection)
                    {
                        *sum_by_solutions_cache.get(&intersection).unwrap()
                    } else {
                        let min_score =
                            turn_sums(words, &intersection, &guessed, cache, picks, log)
                                .lowest_score()
                                .unwrap();
                        sum_by_solutions_cache.insert(intersection, min_score);
                        min_score
                    }
                })
                .sum();
            (guess, sum)
        })
        .collect();
    scores.sort_asc();
    scores.into_iter().take(picks).collect()
}

fn trivial_turn_sum<'a>(
    words: &'a Words,
    secrets: &Solutions,
    guessed: &[&Guess],
    log: bool,
) -> Vec<(&'a Guess, usize)> {
    assert!(secrets.len() <= 2);
    let first = secrets.iter().next().unwrap();
    let first = words.guesses.iter().find(|g| g == first).unwrap();

    let this_turn = guessed.len() + 1;
    return if secrets.len() == 1 {
        // With only one solution left, the optimal "strategy" picks it
        if log {
            println!(
                "{}{}. guess is the solution: {}",
                "\t".repeat(this_turn - 1),
                this_turn,
                first.to_string()
            );
        }
        vec![(first, this_turn)]
    } else {
        let second = secrets.iter().nth(1).unwrap();
        let second = words.guesses.iter().find(|g| g == second).unwrap();
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
                first.to_string(),
                second.to_string()
            );
        }
        vec![(first, sum), (second, sum)]
    };
}

struct WordThatResultsInFewestRemainingSolutions;
impl TryToPickWord for WordThatResultsInFewestRemainingSolutions {
    fn pick(&self, game: &Wordle) -> Option<Guess> {
        // Use this for first guess, because the more complicated
        // methods are too slow with all possible solutions
        if !game.guessed.is_empty() {
            return None;
        }
        let mut scores = fewest_remaining_solutions_for_game(game);
        if game.print_output {
            scores.sort_asc();
            println!("Best (fewest remaining solutions): {}", scores.to_string(5));
        }
        scores.lowest()
    }
}

fn fewest_remaining_solutions_for_game<'a>(game: &'a Wordle) -> Vec<(&'a Guess, f64)> {
    let guessed: Vec<_> = game.guessed.iter().collect();
    fewest_remaining_solutions(game.words, &game.solutions, &guessed, game.cache)
}

fn fewest_remaining_solutions<'a>(
    words: &'a Words,
    secrets: &Solutions,
    guessed: &[&Guess],
    cache: &Cache,
) -> Vec<(&'a Guess, f64)> {
    let is_first_turn = secrets.len() == words.secrets.len();
    let len = secrets.len() as f64;
    let mut scores: Vec<(&Guess, f64)> = words
        .guesses
        .par_iter()
        .filter(|guess| !guessed.contains(guess))
        .map(|guess| {
            let count: usize = secrets
                .iter()
                .map(|&secret| {
                    if is_first_turn {
                        cache.secret_solutions.by_secret_by_guess[guess][secret].len()
                    } else {
                        cache.secret_solutions.by_secret_by_guess[guess][secret]
                            .intersection(secrets)
                            .count()
                    }
                })
                .sum();
            (guess, count as f64 / len)
        })
        .collect();
    scores.sort_asc();
    scores
}

struct MostFrequentGlobalCharacter;
impl TryToPickWord for MostFrequentGlobalCharacter {
    fn pick(&self, game: &Wordle) -> Option<Guess> {
        let positions = &game.open_positions();
        let freq = game.solutions.global_character_counts_in(positions);

        let scores: Vec<_> = game
            .solutions
            .iter()
            .map(|&secret| {
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
        scores.highest()
    }
}
struct MostFrequentGlobalCharacterHighVarietyWord;
impl TryToPickWord for MostFrequentGlobalCharacterHighVarietyWord {
    fn pick(&self, game: &Wordle) -> Option<Guess> {
        let positions = &game.open_positions();
        let high_variety_words = game.solutions.high_variety_words(positions);
        if high_variety_words.is_empty() {
            return None;
        }
        let freq = high_variety_words.global_character_counts_in(positions);
        // println!("Overall character counts: {}", freq.to_string());

        let scores: Vec<_> = high_variety_words
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
        scores.highest()
    }
}

struct FixedGuessList {
    guesses: Vec<Guess>,
}
impl FixedGuessList {
    fn new<S: AsRef<str>>(guesses: Vec<S>) -> Self {
        let guesses: Vec<Guess> = guesses.into_iter().map(|w| w.as_ref().to_word()).collect();
        println!("Fixed guesses {}\n", guesses.to_string());
        FixedGuessList { guesses }
    }
}

impl TryToPickWord for FixedGuessList {
    fn pick(&self, game: &Wordle) -> Option<Guess> {
        self.guesses.get(game.guessed.len()).cloned()
    }
}

struct MostFrequentCharactersOfRemainingWords;
impl TryToPickWord for MostFrequentCharactersOfRemainingWords {
    fn pick(&self, game: &Wordle) -> Option<Guess> {
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
        scores.highest()
    }
}

struct FirstOfTwoOrFewerRemainingSolutions;
impl TryToPickWord for FirstOfTwoOrFewerRemainingSolutions {
    fn pick(&self, game: &Wordle) -> Option<Guess> {
        if game.solutions.len() <= 2 {
            game.solutions.iter().next().copied().cloned()
        } else {
            None
        }
    }
}

struct WordWithMostNewCharsFromRemainingSolutions;
impl TryToPickWord for WordWithMostNewCharsFromRemainingSolutions {
    fn pick(&self, game: &Wordle) -> Option<Guess> {
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
        scores.highest()
    }
}

struct MostFrequentCharacterPerPos;
impl TryToPickWord for MostFrequentCharacterPerPos {
    fn pick(&self, game: &Wordle) -> Option<Guess> {
        let positions = &game.open_positions();
        let counts = game.solutions.character_counts_per_position_in(positions);

        let scores: Vec<_> = game
            .solutions
            .iter()
            .map(|&secret| {
                let count = positions
                    .iter()
                    .map(|&i| counts[i][&secret[i]])
                    .sum::<usize>();
                (secret, count)
            })
            .collect();
        scores.highest()
    }
}

struct MostFrequentCharacterPerPosHighVarietyWord;
impl TryToPickWord for MostFrequentCharacterPerPosHighVarietyWord {
    fn pick(&self, game: &Wordle) -> Option<Guess> {
        let positions = &game.open_positions();
        let high_variety_words = game.solutions.high_variety_words(positions);
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
        scores.highest()
    }
}

struct MatchingMostOtherWordsInAtLeastOneOpenPosition;
impl TryToPickWord for MatchingMostOtherWordsInAtLeastOneOpenPosition {
    fn pick(&self, game: &Wordle) -> Option<Guess> {
        let positions = &game.open_positions();
        // for each word, find out how many other words it matches in any open position
        let mut scores: Vec<_> = game
            .solutions
            .iter()
            .map(|&secret| (secret, 0_usize))
            .collect();
        let open_chars: Vec<_> = game
            .solutions
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

        scores.highest()
    }
}
struct MatchingMostOtherWordsInAtLeastOneOpenPositionHighVarietyWord;
impl TryToPickWord for MatchingMostOtherWordsInAtLeastOneOpenPositionHighVarietyWord {
    fn pick(&self, game: &Wordle) -> Option<Guess> {
        let positions = &game.open_positions();
        let high_variety_words = game.solutions.high_variety_words(positions);
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
        scores.highest()
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

trait ToWord {
    fn to_word(&self) -> Word;
}
impl<S: AsRef<str>> ToWord for S {
    fn to_word(&self) -> Word {
        self.as_ref().trim().chars().collect()
    }
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
    fn calculate_hint(&self, secret: &Word) -> Hints;
}
impl GetHint for Word {
    fn calculate_hint(&self, secret: &Word) -> Hints {
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

fn main() {
    let args: Vec<String> = args().collect();
    let mut lang: Option<Language> = None;
    let mut consumed_args = 1;
    if args.len() > 1 {
        if let Ok(parsed_lang) = Language::try_from(args[1].as_str()) {
            println!("Parsed language '{}'", parsed_lang);
            lang = Some(parsed_lang);
            consumed_args = 2;
        }
    }
    let lang = lang.unwrap_or(English);
    println!(
        "Language: {}. Choices: English, NYTimes, German, Primal.",
        lang
    );
    if args.len() > consumed_args {
        let guesses = args
            .iter()
            .skip(consumed_args)
            .map(|s| s.to_ascii_lowercase())
            .collect();
        let strategy = FixedGuessList::new(guesses);
        autoplay_and_print_stats_with_language(strategy, lang);
    } else {
        let words = Words::new(lang);
        let hsg = HintsBySecretByGuess::of(&words);
        let shg = SolutionsByHintByGuess::of(&words, &hsg);
        let cache = Cache::new(&words, &hsg, &shg);
        let mut game = Wordle::with(&words, &cache);
        game.play();
    }
}

fn autoplay_and_print_stats_with_language<S: TryToPickWord + Sync>(strategy: S, lang: Language) {
    let words = Words::new(lang);
    let hsg = HintsBySecretByGuess::of(&words);
    let shg = SolutionsByHintByGuess::of(&words, &hsg);
    let cache = Cache::new(&words, &hsg, &shg);

    let mut secrets: Vec<_> = words
        .secrets
        .iter()
        // .filter(|w| w.to_string().eq("'rowdy'"))
        .collect();
    secrets.sort_unstable();
    let attempts: Vec<usize> = secrets
        .iter()
        .map(|secret| {
            let mut game = Wordle::with(&words, &cache);
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

struct Cache<'a> {
    hint_solutions: &'a SolutionsByHintByGuess<'a>,
    hints: &'a HintsBySecretByGuess<'a>,
    secret_solutions: SolutionsBySecretByGuess<'a>,
}
impl<'a> Cache<'a> {
    fn new(
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
}

#[derive(Copy, Clone, PartialEq)]
enum Language {
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

#[cfg(test)]
mod tests;
