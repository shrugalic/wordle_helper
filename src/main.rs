use std::cmp::{Ordering, Reverse};
use std::collections::{HashMap, HashSet};
use std::env::args;
use std::fmt::{Display, Formatter};
use std::io;
use std::iter::Map;
use std::str::Lines;

use rayon::prelude::*;

use Hint::*;
use Language::*;

const GUESSES: [&str; 2] = [
    include_str!("../data/word_lists/original/combined.txt"),
    include_str!("../data/word_lists/german/combined.txt"),
];
const SOLUTIONS: [&str; 2] = [
    include_str!("../data/word_lists/original/solutions.txt"),
    include_str!("../data/word_lists/german/solutions.txt"),
];

const ALL_POS: [usize; 5] = [0, 1, 2, 3, 4];

type Word = Vec<char>;
type Guess = Word;
type Secret = Word;
type Feedback = Guess;
type HintValue = u8;

#[derive(Clone)]
struct Words {
    guesses: Vec<Guess>,
    secrets: HashSet<Secret>,
}
impl Words {
    fn new(lang: Language) -> Self {
        Words {
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
    solutions: HashSet<&'a Secret>,
    cache: &'a SolutionsByHintByGuess<'a>,
    guessed: Vec<Guess>,
    print_output: bool,
}
impl<'a> Wordle<'a> {
    fn with(words: &'a Words, cache: &'a SolutionsByHintByGuess<'a>) -> Self {
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
            ],
            PickFirstSolution,
        );

        while self.solutions.len() != 1 {
            self.print_remaining_word_count();

            let suggestion = strategy.pick(self);
            let guess = self.ask_for_guess(suggestion);
            let feedback = self.ask_for_feedback(&guess);
            let hint = Hints::from_feedback(feedback);

            self.update_remaining_solutions(&guess, &hint);
            self.guessed.push(guess);
        }
        self.print_result();
    }

    fn update_remaining_solutions(&mut self, guess: &Guess, hint: &Hints) {
        let solutions = &self.cache.by_hint_by_guess[guess][&hint.value()];
        self.solutions = self
            .solutions
            .intersection(solutions)
            .into_iter()
            .cloned()
            .collect();
    }

    #[cfg(test)]
    #[allow(clippy::ptr_arg)] // to_string is implemented for Word but not &[char]
    fn autoplay(&mut self, secret: &Secret, mut strategy: impl PickWord) {
        self.print_output = false;

        const AUTOPLAY_MAX_ATTEMPTS: usize = 10;
        while self.guessed.len() < AUTOPLAY_MAX_ATTEMPTS {
            let guess: Guess = strategy.pick(self);
            let hint = guess.get_hint(secret);
            self.print_state(&guess, secret, &hint);
            self.guessed.push(guess.clone());
            if guess.eq(secret) {
                self.solutions = self.solutions.drain().filter(|&s| guess.eq(s)).collect();
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

    #[cfg(test)] // autoplay
    fn print_state(&self, guess: &Guess, secret: &Secret, hint: &Hints) {
        println!(
            "{:4} solutions left, {}. guess {}, hint {}, secret {}",
            self.solutions.len(),
            self.guessed.len() + 1,
            guess.to_string(),
            hint,
            secret.to_string(),
        );
    }

    fn print_remaining_word_count(&self) {
        let len = self.solutions.len();
        if len > 10 {
            println!("\n{} words left", len);
        } else {
            println!("\n{} words left: {}", len, self.solutions.to_string());
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

struct SolutionsByHintByGuess<'a> {
    by_hint_by_guess: HashMap<&'a Guess, HashMap<HintValue, HashSet<&'a Secret>>>,
}
impl<'a> SolutionsByHintByGuess<'a> {
    fn of(words: &'a Words) -> Self {
        SolutionsByHintByGuess::new(&words.guesses, &words.secrets)
    }
    fn new(guesses: &'a [Guess], secrets: &'a HashSet<Secret>) -> Self {
        SolutionsByHintByGuess {
            by_hint_by_guess: guesses
                .par_iter()
                .map(|guess| {
                    let mut solutions_by_hint: HashMap<HintValue, HashSet<&Guess>> = HashMap::new();
                    for solution in secrets {
                        solutions_by_hint
                            .entry(guess.get_hint(solution).value())
                            .or_default()
                            .insert(solution);
                    }
                    (guess, solutions_by_hint)
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
fn to_sorted_string<W: AsRef<Word>>(words: impl Iterator<Item = W>) -> String {
    let mut words: Vec<_> = words.map(|w| w.as_ref().to_string()).collect();
    words.sort_unstable();
    words.join(", ")
}
impl<W: AsRef<Word>> WordsToString for Vec<W> {
    fn to_string(&self) -> String {
        to_sorted_string(self.iter())
    }
}
impl<W: AsRef<Word>> WordsToString for &[W] {
    fn to_string(&self) -> String {
        to_sorted_string(self.iter())
    }
}
impl<W: AsRef<Word>> WordsToString for HashSet<W> {
    fn to_string(&self) -> String {
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

trait ScoreTrait {
    fn sort_asc(&mut self);
    fn sort_desc(&mut self);
    fn to_string(&self, count: usize) -> String;
    fn lowest(&self) -> Option<Word>;
    fn highest(&self) -> Option<Word>;
}
impl<T: PartialOrd + Display> ScoreTrait for Vec<(&Word, T)> {
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
            .map(|(word, value)| format!("{:.2} {}", value, word.to_string()))
            .collect::<Vec<_>>()
            .join(", ")
    }
    fn lowest(&self) -> Option<Word> {
        self.iter()
            .min_by(
                |(a_word, a_value), (b_word, b_value)| match a_value.partial_cmp(b_value) {
                    Some(Ordering::Equal) | None => a_word.cmp(b_word),
                    Some(by_value) => by_value,
                },
            )
            .map(|&(word, _)| word.clone())
    }
    fn highest(&self) -> Option<Word> {
        self.iter()
            .max_by(
                |(a_word, a_value), (b_word, b_value)| match a_value.partial_cmp(b_value) {
                    Some(Ordering::Equal) | None => b_word.cmp(a_word),
                    Some(by_value) => by_value,
                },
            )
            .map(|&(word, _)| word.clone())
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
        println!("Using fallback");
        self.fallback.pick(game)
    }
}

trait TryToPickWord {
    fn pick(&self, game: &Wordle) -> Option<Guess>;
}

struct WordThatResultsInFewestRemainingSolutions;
impl TryToPickWord for WordThatResultsInFewestRemainingSolutions {
    fn pick(&self, game: &Wordle) -> Option<Guess> {
        let mut scores = fewest_remaining_solutions_for_game(game);
        if game.print_output {
            scores.sort_asc();
            println!("Best (fewest remaining solutions): {}", scores.to_string(5));
        }
        scores.lowest()
    }
}

fn fewest_remaining_solutions_for_game<'a>(game: &'a Wordle) -> Vec<(&'a Guess, usize)> {
    let guessed: Vec<_> = game.guessed.iter().collect();
    fewest_remaining_solutions(game.words, &game.solutions, &guessed, game.cache)
}

fn fewest_remaining_solutions<'a>(
    words: &'a Words,
    secrets: &HashSet<&Secret>,
    guessed: &[&Guess],
    solutions: &SolutionsByHintByGuess,
) -> Vec<(&'a Guess, usize)> {
    let is_first_turn = secrets.len() == words.secrets.len();
    let mut scores: Vec<(&Guess, usize)> = words
        .guesses
        .par_iter()
        .filter(|guess| !guessed.contains(guess))
        .map(|guess| {
            let count: usize = secrets
                .iter()
                .map(|secret| {
                    let hint = guess.get_hint(secret);
                    if is_first_turn {
                        solutions.by_hint_by_guess[guess][&hint.value()].len()
                    } else {
                        solutions.by_hint_by_guess[guess][&hint.value()]
                            .intersection(secrets)
                            .count()
                    }
                })
                .sum();
            (guess, count)
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
    #[cfg(test)]
    fn new(guesses: Vec<&str>) -> Self {
        println!("Fixed guesses {:?}\n", guesses);
        let guesses = guesses.into_iter().map(|w| w.to_word()).collect();
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
        if game.wanted_chars().len() >= 5 {
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
    fn get_hint(&self, secret: &Word) -> Hints;
}
impl GetHint for Word {
    fn get_hint(&self, secret: &Word) -> Hints {
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
    let lang = if args.len() > 1 {
        Language::try_from(args[1].as_str()).unwrap_or(English)
    } else {
        English
    };
    // print_word_combinations();
    let words = Words::new(lang);
    let cache = SolutionsByHintByGuess::of(&words);
    Wordle::with(&words, &cache).play()
}

#[derive(Copy, Clone)]
enum Language {
    English,
    German,
}
impl TryFrom<&str> for Language {
    type Error = String;

    fn try_from(lang: &str) -> Result<Self, Self::Error> {
        match lang.to_ascii_lowercase().as_str() {
            "english" => Ok(English),
            "german" | "deutsch" => Ok(German),
            _ => Err(format!("Unknown language '{}'", lang)),
        }
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

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;

    const MAX_ATTEMPTS: usize = 6;

    #[ignore] // ~30s for all guesses, ~6s for solutions only
    #[test]
    fn compare_best_word_strategies() {
        let words = Words::new(English);

        // Solutions only:
        // 484.89 roate, 490.38 raise, 493.53 raile, 502.77 soare, 516.34 arise,
        // 516.85 irate, 517.91 orate, 531.22 ariel, 538.21 arose, 548.07 raine
        // Guesses:
        // 12563.92 lares, 12743.70 rales, 13298.11 tares, 13369.60 soare, 13419.26 reais
        // 13461.31 nares, 13684.70 aeros, 13771.28 rates, 13951.64 arles, 13972.96 serai
        let variances = variance_of_remaining_words(&words);
        println!("Best (lowest) variance:\n{}", variances.to_string(10));
        assert_eq!(variances.len(), words.guesses.len());

        // panic!(); // ~2s

        // Solutions only:
        // 60.42 roate, 61.00 raise, 61.33 raile, 62.30 soare, 63.73 arise,
        // 63.78 irate, 63.89 orate, 65.29 ariel, 66.02 arose, 67.06 raine
        // Guesses:
        // 288.74 lares, 292.11 rales, 302.49 tares, 303.83 soare, 304.76 reais
        // 305.55 nares, 309.73 aeros, 311.36 rates, 314.73 arles, 315.13 serai
        let remaining = expected_remaining_solution_counts(&words);
        println!(
            "Best (lowest) expected remaining solution count:\n{}",
            remaining.to_string(10)
        );
        assert_eq!(remaining.len(), words.guesses.len());

        // panic!(); // ~5s

        // Solutions only:
        // 139883 roate, 141217 raise, 141981 raile, 144227 soare, 147525 arise
        // All guesses:
        // Best (lowest) average remaining solution count:
        // 3745512 lares, 3789200 rales, 3923922 tares, 3941294 soare, 3953360 reais
        // 3963578 nares, 4017862 aeros, 4038902 rates, 4082728 arles, 4087910 serai
        let words = Words::new(English);
        let cache = SolutionsByHintByGuess::of(&words);
        let game = Wordle::with(&words, &cache);
        let totals = fewest_remaining_solutions_for_game(&game);
        println!(
            "Best (lowest) average remaining solution count:\n{}",
            totals.to_string(10)
        );
        assert_eq!(totals.len(), words.guesses.len());

        // panic!(); // ~8s

        let (mut different, mut same) = (0, 0);
        for (i, ((low_w, count), (rem_w, remaining))) in
            totals.iter().zip(remaining.iter()).enumerate()
        {
            if low_w != rem_w {
                different += 1;
                println!(
                    "{} ({}, {}) ({}, {})",
                    i,
                    low_w.to_string(),
                    count,
                    rem_w.to_string(),
                    remaining
                );
            } else {
                same += 1;
            }
        }
        println!(
            "{} are same, {} are different\n\n-------------------\n",
            same, different
        );
        // 4193 (beefs, 16913576) (mixte, 1303.852605612086)
        // 4194 (mixte, 16913576) (beefs, 1303.8526056120852)
        // 5028 (plume, 15566118) (sewed, 1199.9782608695646)
        // 5029 (sewed, 15566118) (plume, 1199.9782608695637)
        // 6072 (nicol, 14081082) (perdy, 1085.4981498612387)
        // 6073 (perdy, 14081082) (nicol, 1085.4981498612383)
        // 7913 (slots, 11871272) (tuber, 915.1458526056141)
        // 7914 (tuber, 11871272) (slots, 915.145852605613)
        // 8091 (geyan, 11675600) (tatus, 900.0616712920146)
        // 8092 (tatus, 11675600) (geyan, 900.0616712920142)
        //
        // All guesses:    12962 are same,  10 are different
        // Solutions only: 12724 are same, 248 are different -> variance is slightly better than expected-remaining
        //
        // -------------------
        //
        // Solutions only: 12762 are same, 210 are different
        // All guesses:    12964 are same,   8 are different -> variance is slightly better than expected-remaining
        //
        // 4193 (beefs, 16913576) (mixte, 66753.47904282859)
        // 4194 (mixte, 16913576) (beefs, 66753.47904282843)
        // 6354 (banns, 13699230) (bonks, 53525.717725956216)
        // 6355 (bonks, 13699230) (banns, 53525.717725956)
        // 7913 (slots, 11871272) (tuber, 46003.256820606526)
        // 7914 (tuber, 11871272) (slots, 46003.2568206063)
        // 8091 (geyan, 11675600) (tatus, 45198.022252705174)
        // 8092 (tatus, 11675600) (geyan, 45198.02225270514)

        let (mut different, mut same) = (0, 0);
        for (i, ((low_w, count), (var_w, variance))) in
            totals.iter().zip(variances.iter()).enumerate()
        {
            if low_w != var_w {
                different += 1;
                println!(
                    "{} ({}, {}) ({}, {})",
                    i,
                    low_w.to_string(),
                    count,
                    var_w.to_string(),
                    variance
                );
            } else {
                same += 1;
            }
        }
        println!("{} are same, {} are different", same, different);
    }

    // Previously used method, slightly less stable than lowest_total_number_of_remaining_solutions
    fn variance_of_remaining_words(words: &Words) -> Vec<(&Guess, f64)> {
        let average = words.secrets.len() as f64 / 243.0;
        let mut scores: Vec<_> = words
            .guesses
            .par_iter()
            .map(|guess| {
                let buckets = hint_buckets(guess, &words.secrets);
                let variance = buckets
                    .into_iter()
                    .map(|indices| (indices.len() as f64 - average).powf(2.0))
                    .sum::<f64>() as f64
                    / 243.0;
                (guess, variance)
            })
            .collect();
        scores.sort_asc();
        scores
    }

    // Previously used method, slightly less stable than lowest_total_number_of_remaining_solutions
    fn expected_remaining_solution_counts(words: &Words) -> Vec<(&Guess, f64)> {
        let total_solutions = words.secrets.len() as f64;
        let mut scores: Vec<_> = words
            .guesses
            .par_iter()
            .map(|guess| {
                let buckets = hint_buckets(guess, &words.secrets);
                let expected_remaining_word_count = buckets
                    .into_iter()
                    .map(|indices| {
                        let solutions_in_bucket: f64 = indices.len() as f64;
                        let bucket_probability = solutions_in_bucket / total_solutions;
                        bucket_probability * solutions_in_bucket
                        // solutions_in_bucket ^ 2 / total_solutions;
                    })
                    .sum();
                (guess, expected_remaining_word_count)
            })
            .collect();
        scores.sort_asc();
        scores
    }

    // Allow because determine_hints expects &Guess not &[char]
    fn hint_buckets(
        #[allow(clippy::ptr_arg)] guess: &Guess,
        solutions: &HashSet<Secret>,
    ) -> [Vec<usize>; 243] {
        const EMPTY_VEC: Vec<usize> = Vec::new();
        let mut hint_buckets = [EMPTY_VEC; 243];
        for (i, solution) in solutions.iter().enumerate() {
            let hint = guess.get_hint(solution);
            hint_buckets[hint.value() as usize].push(i);
        }
        hint_buckets
    }

    #[ignore] // ~13s English or ~2s German
    #[test]
    fn test_print_full_guess_tree() {
        print_full_guess_tree();
    }
    fn print_full_guess_tree() {
        let words = Words::new(German);
        let secrets: HashSet<_> = words.secrets.iter().collect();
        let hg_solutions = SolutionsByHintByGuess::of(&words);
        let guessed = [];
        explore_tree(&words, &secrets, &guessed, &hg_solutions);
    }
    #[ignore]
    #[test]
    fn test_print_partial_guess_tree() {
        print_partial_guess_tree();
    }
    fn print_partial_guess_tree() {
        let words = Words::new(English);
        let secret = "piper".to_word();
        let guess1 = "roate".to_word();
        let guess2 = "feued".to_word();
        let hint1 = guess1.get_hint(&secret); // "🟨⬛⬛⬛🟨"
        let hint2 = guess2.get_hint(&secret); // "⬛⬛⬛🟩⬛"
        let hg_solutions = SolutionsByHintByGuess::of(&words);
        let secrets1 = &hg_solutions.by_hint_by_guess[&guess1][&hint1.value()];
        println!("{} roate secrets {}", secrets1.len(), secrets1.to_string());
        let secrets2 = &hg_solutions.by_hint_by_guess[&guess2][&hint2.value()];
        println!("{} feued secrets {}", secrets2.len(), secrets2.to_string());
        let secrets: HashSet<&Secret> = secrets1
            .intersection(secrets2)
            .into_iter()
            .cloned()
            .collect();
        println!(
            "{} intersected secrets {}",
            secrets.len(),
            secrets.to_string()
        );

        let guessed = [&guess1, &guess2];
        explore_tree(&words, &secrets, &guessed, &hg_solutions);
    }
    fn explore_tree(
        words: &Words,
        secrets: &HashSet<&Secret>,
        guessed: &[&Word],
        hg_solutions: &SolutionsByHintByGuess,
    ) {
        if guessed.len() == MAX_ATTEMPTS {
            println!(
                "            7. Still not found after 6 guesses {}. Secrets: {}",
                guessed.to_string(),
                secrets.to_string()
            );
            return;
        } else if secrets.len() <= 2 {
            // 1 and 2 already printed info on how to proceed
            return;
        }
        let scores = fewest_remaining_solutions(words, secrets, guessed, hg_solutions);
        let guess = scores.lowest().unwrap();
        let guessed: Vec<_> = guessed
            .iter()
            .cloned()
            .chain(std::iter::once(&guess))
            .collect();

        let mut pairs: Vec<_> = hg_solutions.by_hint_by_guess[&guess]
            .iter()
            .map(|(h, solutions)| {
                (
                    h,
                    solutions
                        .intersection(secrets)
                        .into_iter()
                        .cloned()
                        .collect::<HashSet<_>>(),
                )
            })
            .filter(|(_, solutions)| !solutions.is_empty())
            .collect();
        pairs.sort_unstable_by(|(v1, s1), (v2, s2)| match s1.len().cmp(&s2.len()) {
            Ordering::Equal => v1.cmp(v2), // lower hint-value (more unknown) first
            fewer_elements_first => fewer_elements_first,
        });
        for (hint, secrets) in pairs {
            print_info(&guessed, *hint, &secrets);
            explore_tree(words, &secrets, &guessed, hg_solutions)
        }
    }
    fn print_info(guessed: &[&Guess], hint: HintValue, secrets: &HashSet<&Secret>) {
        let turn = guessed.len();
        let indent = "\t".repeat(turn - 1);
        let guess = guessed.last().unwrap().to_string();
        let first = secrets.iter().next().unwrap().to_string();
        print!(
            "{}{}. guess {} + hint {} matches ",
            indent,
            turn,
            guess,
            Hints::from(hint)
        );
        if secrets.len() == 1 {
            println!("{}, use it as {}. guess.", first, turn + 1);
        } else if secrets.len() == 2 {
            let second = secrets.iter().nth(1).unwrap().to_string();
            println!(
                "{} and {}. Pick one at random to win by the {}. guess.",
                first,
                second,
                turn + 2
            );
        } else if secrets.len() <= 5 {
            println!("{} secrets {}.", secrets.len(), secrets.to_string());
        } else {
            println!("{} secrets, for example {}.", secrets.len(), first);
        }
    }

    #[ignore] // < 2s
    #[test]
    fn count_solutions_by_hint_by_guess() {
        let words = Words::new(English);
        let solutions = SolutionsByHintByGuess::of(&words);
        assert_eq!(
            solutions
                .by_hint_by_guess
                .iter()
                .map(|(_, s)| s.len())
                .sum::<usize>(),
            1_120_540 // 1120540
        );
    }

    #[ignore]
    #[test]
    // 0. total time [ms]: 1080 cache initialization
    // 1. total time [ms]: 1416 cached,  883 w/o cache
    // 2. total time [ms]: 1772 cached, 1818 w/o cache <- break even
    // 3. total time [ms]: 2121 cached, 2765 w/o cache <- break even
    // 4. total time [ms]: 2473 cached, 3722 w/o cache <- break even
    fn does_hint_cache_help_part_1_without_cache() {
        let words = Words::new(English);

        let start = Instant::now();
        let hints = HintsBySecretByGuess::of(&words);
        let mut t_cached = start.elapsed();
        println!(
            "0. total time [ms]: {:4} cache initialization",
            t_cached.as_millis(),
        );

        let sum_with_cache = |words: &Words| -> usize {
            words
                .guesses
                .par_iter()
                .map(|guess| {
                    words
                        .secrets
                        .iter()
                        .map(|secret| hints.by_secret_by_guess[guess][secret].value() as usize)
                        .sum::<usize>()
                })
                .sum()
        };

        let sum_without_cache = |words: &Words| -> usize {
            words
                .guesses
                .par_iter()
                .map(|guess| {
                    words
                        .secrets
                        .iter()
                        .map(|secret| guess.get_hint(secret).value() as usize)
                        .sum::<usize>()
                })
                .sum()
        };

        let mut t_uncached = Duration::default();
        for i in 1..5 {
            let start = Instant::now();
            let sum = sum_without_cache(&words);
            assert_eq!(1_056_428_862, sum); // 1056428862
            t_uncached += start.elapsed();

            let start = Instant::now();
            let sum = sum_with_cache(&words);
            assert_eq!(1_056_428_862, sum); // 1056428862
            t_cached += start.elapsed();

            println!(
                "{}. total time [ms]: {:4} cached, {:4} w/o cache{}",
                i,
                t_cached.as_millis(),
                t_uncached.as_millis(),
                if t_cached <= t_uncached {
                    " <- break even"
                } else {
                    ""
                }
            );
        }
    }

    struct HintsBySecretByGuess<'a> {
        by_secret_by_guess: HashMap<&'a Guess, HashMap<&'a Secret, Hints>>,
    }
    impl<'a> HintsBySecretByGuess<'a> {
        fn of(words: &'a Words) -> Self {
            HintsBySecretByGuess::new(&words.guesses, &words.secrets)
        }
        fn new(guesses: &'a [Guess], secrets: &'a HashSet<Secret>) -> Self {
            HintsBySecretByGuess {
                by_secret_by_guess: guesses
                    .par_iter()
                    .map(|guess| {
                        let hints_by_solution = secrets
                            .iter()
                            .map(|secret| (secret, guess.get_hint(secret)))
                            .collect::<HashMap<&Secret, Hints>>();
                        (guess, hints_by_solution)
                    })
                    .collect(),
            }
        }
    }

    #[ignore] // 2-4s
    #[test]
    fn count_solutions_by_secret_by_guess() {
        let words = Words::new(English);
        let solutions = SolutionsByHintByGuess::of(&words);
        let solutions_sg = SolutionsBySecretByGuess::of(&words, &solutions);

        assert_eq!(
            solutions_sg
                .by_secret_by_guess
                .iter()
                .map(|(_, s)| s.len())
                .sum::<usize>(),
            words.secrets.len() * words.guesses.len() // 2_315 * 12_972 = 30_030_180 = 2315 * 12972 = 30030180
        );
    }

    struct SolutionsBySecretByGuess<'a> {
        by_secret_by_guess: HashMap<&'a Guess, HashMap<&'a Secret, &'a HashSet<&'a Secret>>>,
    }
    impl<'a> SolutionsBySecretByGuess<'a> {
        fn of(words: &'a Words, solutions: &'a SolutionsByHintByGuess) -> Self {
            SolutionsBySecretByGuess::new(&words.guesses, &words.secrets, solutions)
        }
        fn new(
            guesses: &'a [Guess],
            secrets: &'a HashSet<Secret>,
            solutions: &'a SolutionsByHintByGuess,
        ) -> Self {
            SolutionsBySecretByGuess {
                by_secret_by_guess: guesses
                    .par_iter()
                    .map(|guess| {
                        let mut solutions_by_secret: HashMap<&Secret, &HashSet<&Secret>> =
                            HashMap::new();
                        for secret in secrets.iter() {
                            let hint = guess.get_hint(secret).value();
                            let solutions = &solutions.by_hint_by_guess[guess][&hint];
                            solutions_by_secret.insert(secret, solutions);
                        }
                        (guess, solutions_by_secret)
                    })
                    .collect(),
            }
        }
    }

    #[ignore] // ~3s
    #[test]
    // Top 5: 139883 roate, 141217 raise, 141981 raile, 144227 soare, 147525 arise
    fn find_optimal_first_word_english() {
        let words = Words::new(English);
        let cache = SolutionsByHintByGuess::of(&words);
        let game = Wordle::with(&words, &cache);
        let scores = fewest_remaining_solutions_for_game(&game);
        // println!("scores {}", scores.to_string(5));
        let optimal = scores.lowest().unwrap();
        assert_eq!("roate".to_word(), optimal);
    }

    #[ignore] // ~1s
    #[test]
    // Top 5: 71017 tarne, 72729 raine, 74391 trane, 75473 lernt, 75513 raete
    fn find_optimal_first_word_german() {
        let words = Words::new(German);
        let cache = SolutionsByHintByGuess::of(&words);
        let game = Wordle::with(&words, &cache);
        let scores = fewest_remaining_solutions_for_game(&game);
        println!("scores {}", scores.to_string(5));
        let optimal = scores.lowest().unwrap();
        assert_eq!("raine".to_word(), optimal);
    }

    #[ignore]
    #[test]
    // ~10s (i9) or ~13s (M1) for 5 single German words
    // ~1min 51s (i9) or ~2min 21s (M1) for 5 single English words
    // Deutsch:
    // Best 1. guesses: 36655 raine, 41291 taler, 42405 raten, 42461 laser, 42897 reale
    // Best 2. guesses after 1. 'raine': 3803 holst, 3893 kults, 3911 lotus, 3965 stuhl, 4117 buhlt
    // Best 3. guesses after 1. 'raine' and 2. 'holst': 1635 dumpf, 1677 umgab, 1709 umweg, 1745 bekam, 1761 bezug
    // Best 4. guesses after 1. 'raine' and 2. 'holst' and 3. 'dumpf': 1247 biwak, 1261 abweg, 1271 bezog, 1271 bezug, 1273 beeck
    // Best 5. guesses after 1. 'raine' and 2. 'holst' and 3. 'dumpf' and 4. 'biwak': 1179 legen, 1179 leger, 1181 engen, 1181 enzen, 1181 genen
    //
    // English
    // Best 2. guesses after 1. 'roate': 11847 linds, 11947 sling, 12033 clips, 12237 limns, 12337 blins
    // Best 3. guesses after 1. 'roate' and 2. 'linds': 3803 chump, 3905 bumph, 4117 crump, 4169 clump, 4173 bumpy
    // Best 4. guesses after 1. 'roate' and 2. 'linds' and 3. 'chump': 2659 gleby, 2673 gawky, 2675 gybed, 2685 befog, 2685 bogey
    // Best 5. guesses after 1. 'roate' and 2. 'linds' and 3. 'chump' and 4. 'gleby': 2399 wakfs, 2419 waift, 2421 swift, 2427 fatwa, 2431 fawns
    //
    // Best top-2 5-combos in 13min 30s:
    // Best 2. guesses after 1. 'roate': 11847 linds, 11947 sling, 12033 clips, 12237 limns, 12337 blins
    // Best 3. guesses after 1. 'roate' and 2. 'linds': 3803 chump, 3905 bumph, 4117 crump, 4169 clump, 4173 chomp
    // Best 4. guesses after 1. 'roate' and 2. 'linds' and 3. 'chump': 2659 gleby, 2673 gawky, 2675 gybed, 2685 bogey, 2685 befog
    // Best 5. guesses after 1. 'roate' and 2. 'linds' and 3. 'chump' and 4. 'gleby': 2399 wakfs, 2419 waift, 2421 swift, 2427 fatwa, 2431 fawny
    // Best 5. guesses after 1. 'roate' and 2. 'linds' and 3. 'chump' and 4. 'gawky': 2397 befit, 2405 feebs, 2405 brief, 2409 fubar, 2409 beefy
    //
    // Best 4. guesses after 1. 'roate' and 2. 'linds' and 3. 'bumph': 2599 gawcy, 2665 cagey, 2697 fleck, 2701 wacky, 2701 gucky
    // Best 5. guesses after 1. 'roate' and 2. 'linds' and 3. 'bumph' and 4. 'gawcy': 2399 fleek, 2401 skelf, 2403 flisk, 2403 fleck, 2405 kraft
    // Best 5. guesses after 1. 'roate' and 2. 'linds' and 3. 'bumph' and 4. 'cagey': 2427 wakfs, 2427 swift, 2429 wheft, 2429 tweak, 2433 frown
    //
    // Best 3. guesses after 1. 'roate' and 2. 'sling': 3867 chump, 3935 dumpy, 3963 bumph, 4005 duchy, 4105 daych
    // Best 4. guesses after 1. 'roate' and 2. 'sling' and 3. 'chump': 2553 bawdy, 2577 byked, 2581 dweeb, 2597 bedew, 2609 fyked
    // Best 5. guesses after 1. 'roate' and 2. 'sling' and 3. 'chump' and 4. 'bawdy': 2383 fever, 2387 kraft, 2387 keefs, 2387 fleek, 2389 kiefs
    // Best 5. guesses after 1. 'roate' and 2. 'sling' and 3. 'chump' and 4. 'byked': 2393 frowy, 2395 waift, 2395 frows, 2397 wheft, 2397 frown
    //
    // Best 4. guesses after 1. 'roate' and 2. 'sling' and 3. 'dumpy': 2625 beech, 2631 batch, 2637 chowk, 2653 bitch, 2659 broch
    // Best 5. guesses after 1. 'roate' and 2. 'sling' and 3. 'dumpy' and 4. 'beech': 2397 wakfs, 2415 awful, 2417 waift, 2423 wauff, 2425 woful
    // Best 5. guesses after 1. 'roate' and 2. 'sling' and 3. 'dumpy' and 4. 'batch': 2409 wakfs, 2421 wheft, 2425 fewer, 2427 frows, 2427 frown
    //
    // Best 2. guesses after 1. 'raise': 11827 clout, 12307 cloth, 12677 count, 12735 poult, 13037 ymolt
    // Best 3. guesses after 1. 'raise' and 2. 'clout': 3943 pwned, 4005 pownd, 4021 pyned, 4049 nymph, 4111 bendy
    // Best 4. guesses after 1. 'raise' and 2. 'clout' and 3. 'pwned': 2729 bimah, 2735 begem, 2741 hamba, 2749 gamba, 2749 bhang
    // Best 5. guesses after 1. 'raise' and 2. 'clout' and 3. 'pwned' and 4. 'bimah': 2417 gulfy, 2417 fugly, 2419 goofy, 2423 fuggy, 2423 fudgy
    // Best 5. guesses after 1. 'raise' and 2. 'clout' and 3. 'pwned' and 4. 'begem': 2421 hefty, 2425 khafs, 2425 huffy, 2425 forky, 2427 fishy
    //
    // Best 4. guesses after 1. 'raise' and 2. 'clout' and 3. 'pownd': 2709 begem, 2757 bimah, 2763 gamba, 2767 embay, 2767 beamy
    // Best 5. guesses after 1. 'raise' and 2. 'clout' and 3. 'pownd' and 4. 'begem': 2407 hefty, 2409 huffy, 2411 fishy, 2415 khafs, 2421 skyfs
    // Best 5. guesses after 1. 'raise' and 2. 'clout' and 3. 'pownd' and 4. 'bimah': 2413 gulfy, 2413 fugly, 2419 goofy, 2419 fuggy, 2419 fogey
    // Best 3. guesses after 1. 'raise' and 2. 'cloth': 3795 dungy, 3829 bundy, 3837 gundy, 3901 pyned, 3911 dumpy
    //
    // Best 4. guesses after 1. 'raise' and 2. 'cloth' and 3. 'dungy': 2637 abamp, 2657 pombe, 2665 frump, 2683 bumps, 2685 bumpy
    // Best 5. guesses after 1. 'raise' and 2. 'cloth' and 3. 'dungy' and 4. 'abamp': 2413 wakfs, 2417 fewer, 2417 fetwa, 2419 wheft, 2421 welkt
    // Best 5. guesses after 1. 'raise' and 2. 'cloth' and 3. 'dungy' and 4. 'pombe': 2415 wheft, 2417 wakfs, 2417 fewer, 2419 fetwa, 2423 tweak
    //
    // Best 4. guesses after 1. 'raise' and 2. 'cloth' and 3. 'bundy': 2643 gompa, 2645 gramp, 2659 grump, 2669 gimps, 2671 gimpy
    // Best 5. guesses after 1. 'raise' and 2. 'cloth' and 3. 'bundy' and 4. 'gompa': 2401 wakfs, 2409 wheft, 2409 fewer, 2413 tweak, 2413 fetwa
    // Best 5. guesses after 1. 'raise' and 2. 'cloth' and 3. 'bundy' and 4. 'gramp': 2407 wakfs, 2413 fewer, 2415 wheft, 2415 fetwa, 2417 swift
    fn find_optimal_word_combos() {
        let words = Words::new(English);
        let cache = SolutionsByHintByGuess::of(&words);
        let game = Wordle::with(&words, &cache);
        let mut scores = fewest_remaining_solutions_for_game(&game);
        println!("Best 1. guesses: {}", scores.to_string(5));
        scores.sort_asc();

        let top_pick_count = 1;
        for (guess1, _) in scores.into_iter().take(top_pick_count) {
            let guessed = [guess1];
            let scores = find_best_next_guesses(&game, &guessed);

            println!(
                "Best 2. guesses after 1. {}: {}",
                guess1.to_string(),
                scores.to_string(5)
            );

            for (guess2, _) in scores.into_iter().take(top_pick_count) {
                let guessed = [guess1, guess2];
                let scores = find_best_next_guesses(&game, &guessed);
                println!(
                    "Best 3. guesses after 1. {} and 2. {}: {}",
                    guess1.to_string(),
                    guess2.to_string(),
                    scores.to_string(5)
                );

                for (guess3, _) in scores.into_iter().take(top_pick_count) {
                    let guessed = [guess1, guess2, guess3];
                    let scores = find_best_next_guesses(&game, &guessed);
                    println!(
                        "Best 4. guesses after 1. {} and 2. {} and 3. {}: {}",
                        guess1.to_string(),
                        guess2.to_string(),
                        guess3.to_string(),
                        scores.to_string(5)
                    );

                    for (guess4, _) in scores.into_iter().take(top_pick_count) {
                        let guessed = [guess1, guess2, guess3, guess4];
                        let scores = find_best_next_guesses(&game, &guessed);
                        println!(
                            "Best 5. guesses after 1. {} and 2. {} and 3. {} and 4. {}: {}",
                            guess1.to_string(),
                            guess2.to_string(),
                            guess3.to_string(),
                            guess4.to_string(),
                            scores.to_string(5)
                        );
                    }
                }
            }
        }
    }

    fn find_best_next_guesses<'g>(game: &'g Wordle, guessed: &[&Guess]) -> Vec<(&'g Guess, usize)> {
        let first = *guessed.iter().next().unwrap();
        let words = &game.words;
        let solutions_hg = SolutionsByHintByGuess::of(words);
        let solutions_sg = SolutionsBySecretByGuess::of(words, &solutions_hg);
        // false solutions_sg.by_secret_by_guess 25s/25s/25s
        // false solutions_sg.by_secret_by_guess 19s/22s/22s/23s
        let use_solutions_by_secret_by_guess = false;

        let solutions_bh_bg = SolutionsByHintByGuess::of(words);
        let mut scores: Vec<_> = game
            .allowed()
            .into_par_iter()
            .filter(|next| !guessed.contains(next))
            .map(|next| {
                let count: usize = game
                    .solutions
                    .iter()
                    .map(|&secret| {
                        let (solutions1, solutions2) = if use_solutions_by_secret_by_guess {
                            let solutions1 = solutions_sg.by_secret_by_guess[first][secret];
                            let solutions2 = solutions_sg.by_secret_by_guess[next][secret];
                            (solutions1, solutions2)
                        } else {
                            let hint1 = first.get_hint(secret);
                            let solutions1 =
                                &solutions_bh_bg.by_hint_by_guess[first][&hint1.value()];
                            let hint2 = next.get_hint(secret);
                            let solutions2 =
                                &solutions_bh_bg.by_hint_by_guess[next][&hint2.value()];
                            (solutions1, solutions2)
                        };
                        // apply first and next guess
                        let mut solutions: HashSet<_> =
                            solutions1.intersection(solutions2).cloned().collect();

                        // Apply other previous guesses
                        for other in guessed.iter().skip(1).cloned() {
                            let solutions3 = solutions_sg.by_secret_by_guess[other][secret];
                            solutions = solutions.intersection(solutions3).cloned().collect();
                        }
                        solutions.len()
                    })
                    .sum();
                (next, count)
            })
            .collect();

        scores.sort_asc();
        scores
    }

    #[ignore]
    #[test]
    fn test_hint_from_value_and_back() {
        for value in 0..243 {
            let hint = Hints::from(value);
            println!("{} = {}", hint, value);
            assert_eq!(value, Hints::from(value).value());
        }
    }

    #[ignore] // ~65min
    #[test]
    // Average attempts = 3.547; 2: 39, 3: 1015, 4: 1216, 5: 45
    fn auto_play_word_that_results_in_fewest_remaining_solutions() {
        autoplay_and_print_stats(WordThatResultsInFewestRemainingSolutions);
    }

    #[ignore]
    #[test]
    // Average attempts = 4.032; 2 (0.086%) failed games (> 6 attempts):
    // 2: 54, 3: 511, 4: 1109, 5: 592, 6: 47, 7: 2
    fn auto_play_tubes_fling_champ_wordy_every_time() {
        let strategy = FixedGuessList::new(vec!["tubes", "fling", "champ", "wordy"]);
        autoplay_and_print_stats(strategy);
    }

    #[ignore]
    #[test]
    // Average attempts = 3.829; 7 (0.302%) failed games (> 6 attempts):
    // 2: 57, 3: 770, 4: 1077, 5: 342, 6: 62, 7: 6, 8: 1
    fn auto_play_roate_linds_chump_gawky_befit() {
        let strategy = FixedGuessList::new(vec!["roate", "linds", "chump", "gawky", "befit"]);
        autoplay_and_print_stats(strategy);
    }

    #[ignore]
    #[test]
    // Average attempts = 3.816; 2 (0.086%) failed games (> 6 attempts):
    // 2: 47, 3: 796, 4: 1065, 5: 353, 6: 52, 7: 2
    fn auto_play_roate_linds_chump_gawky() {
        let strategy = FixedGuessList::new(vec!["roate", "linds", "chump", "gawky"]);
        autoplay_and_print_stats(strategy);
    }

    #[ignore]
    #[test]
    // Average attempts = 3.807; 5 (0.216%) failed games (> 6 attempts):
    // 2: 62, 3: 789, 4: 1073, 5: 321, 6: 65, 7: 4, 8: 1
    fn auto_play_roate_linds_chump_gleby_wakfs() {
        let strategy = FixedGuessList::new(vec!["roate", "linds", "chump", "gleby", "wakfs"]);
        autoplay_and_print_stats(strategy);
    }

    #[ignore]
    #[test]
    // Average attempts = 3.809; 4 (0.173%) failed games (> 6 attempts):
    // 2: 65, 3: 769, 4: 1085, 5: 340, 6: 52, 7: 4
    fn auto_play_roate_linds_chump_gleby() {
        let strategy = FixedGuessList::new(vec!["roate", "linds", "chump", "gleby"]);
        autoplay_and_print_stats(strategy);
    }

    #[ignore]
    #[test]
    // Average attempts = 3.564; 0 (0.000%) failed games (> 6 attempts):
    // 2: 41, 3: 530, 4: 502, 5: 94, 6: 4
    fn auto_play_german_raine_holst_dumpf_biwak_legen() {
        let strategy = FixedGuessList::new(vec!["raine", "holst", "dumpf", "biwak", "legen"]);
        autoplay_and_print_stats_with_language(strategy, German);
    }

    #[ignore]
    #[test]
    // Average attempts = 3.564; 0 (0.000%) failed games (> 6 attempts):
    // 2: 41, 3: 530, 4: 502, 5: 94, 6: 4
    fn auto_play_german_raine_holst_dumpf_biwak() {
        let strategy = FixedGuessList::new(vec!["raine", "holst", "dumpf", "biwak"]);
        autoplay_and_print_stats_with_language(strategy, German);
    }

    #[ignore]
    #[test]
    // Average attempts = 3.749; 9 (0.546%) failed games (> 6 attempts):
    // 2: 82, 3: 649, 4: 610, 5: 229, 6: 70, 7: 6, 8: 3
    fn auto_play_german_tarne_helis_gudok_zamba_fiept() {
        let strategy = FixedGuessList::new(vec!["tarne", "helis", "gudok", "zamba", "fiept"]);
        autoplay_and_print_stats_with_language(strategy, German);
    }

    #[ignore]
    #[test]
    // Average attempts = 3.763; 7 (0.424%) failed games (> 6 attempts):
    // 2: 79, 3: 623, 4: 631, 5: 249, 6: 60, 7: 6, 8: 1
    fn auto_play_german_tarne_helis_gudok_zamba() {
        let strategy = FixedGuessList::new(vec!["tarne", "helis", "gudok", "zamba"]);
        autoplay_and_print_stats_with_language(strategy, German);
    }

    #[ignore]
    #[test]
    // Average attempts = 3.994; 21 (0.907%) failed games (> 6 attempts):
    // 2: 53, 3: 738, 4: 873, 5: 496, 6: 134, 7: 19, 8: 2
    fn auto_play_soare_until_pygmy_whack_every_time() {
        let strategy = FixedGuessList::new(vec!["soare", "until", "pygmy", "whack"]);
        autoplay_and_print_stats(strategy);
    }

    #[ignore]
    #[test]
    // Average attempts = 4.879; 219 (9.460%) failed games (> 6 attempts):
    // 1: 1, 2: 18, 3: 317, 4: 592, 5: 622, 6: 546, 7: 200, 8: 18, 9: 1
    fn auto_play_quick_brown_foxed_jumps_glazy_vetch_every_time() {
        let strategy =
            FixedGuessList::new(vec!["quick", "brown", "foxed", "jumps", "glazy", "vetch"]);
        autoplay_and_print_stats(strategy);
    }

    #[ignore]
    #[test]
    // Average attempts = 4.024; 2 (0.086%) failed games (> 6 attempts):
    // 1: 1, 2: 51, 3: 500, 4: 1177, 5: 513, 6: 71, 7: 2
    fn auto_play_fixed_guess_list_1() {
        let strategy = FixedGuessList::new(vec!["brake", "dying", "clots", "whump"]);
        autoplay_and_print_stats(strategy);
    }

    #[ignore]
    #[test]
    // Average attempts = 3.910; 1 (0.043%) failed games (> 6 attempts):
    // 1: 1, 2: 64, 3: 636, 4: 1114, 5: 443, 6: 56, 7: 1
    fn auto_play_fixed_guess_list_2() {
        let strategy = FixedGuessList::new(vec!["maple", "sight", "frown", "ducky"]);
        autoplay_and_print_stats(strategy);
    }

    #[ignore]
    #[test]
    // Average attempts = 4.034; 5 (0.216%) failed games (> 6 attempts):
    // 1: 1, 2: 49, 3: 550, 4: 1081, 5: 544, 6: 85, 7: 5
    fn auto_play_fixed_guess_list_3() {
        let strategy = FixedGuessList::new(vec!["fiend", "paths", "crumb", "glows"]);
        autoplay_and_print_stats(strategy);
    }

    #[ignore]
    #[test]
    // Average attempts = 3.785; 11 (0.475%) failed games (> 6 attempts):
    // 2: 67, 3: 854, 4: 974, 5: 364, 6: 45, 7: 7, 8: 4
    fn auto_play_fixed_guess_list_4() {
        let strategy = FixedGuessList::new(vec!["reals", "point", "ducky"]);
        autoplay_and_print_stats(strategy);
    }

    #[ignore]
    #[test]
    // Average attempts = 3.831; 8 (0.346%) failed games (> 6 attempts):
    // 2: 72, 3: 725, 4: 1113, 5: 344, 6: 53, 7: 6, 8: 1, 9: 1
    fn auto_play_fixed_guess_list_5() {
        let strategy = FixedGuessList::new(vec!["laser", "pitch", "mound"]);
        autoplay_and_print_stats(strategy);
    }

    #[ignore]
    #[test]
    // Average attempts = 3.743; 23 (0.994%) failed games (> 6 attempts):
    // 1: 1, 2: 135, 3: 853, 4: 924, 5: 304, 6: 75, 7: 16, 8: 5, 9: 2
    fn auto_play_most_frequent_global_characters() {
        autoplay_and_print_stats(MostFrequentGlobalCharacter);
    }

    #[ignore]
    #[test]
    // Average attempts = 3.715; 26 (1.123%) failed games (> 6 attempts):
    // 1: 1, 2: 130, 3: 919, 4: 888, 5: 268, 6: 83, 7: 18, 8: 7, 10: 1
    fn auto_play_most_frequent_global_characters_high_variety_word() {
        autoplay_and_print_stats(MostFrequentGlobalCharacterHighVarietyWord);
    }

    #[ignore]
    #[test]
    // Average attempts = 3.778; 22 (0.950%) failed games (> 6 attempts):
    // 1: 1, 2: 148, 3: 773, 4: 955, 5: 343, 6: 73, 7: 19, 8: 2, 9: 1
    fn auto_play_most_frequent_characters_per_pos() {
        autoplay_and_print_stats(MostFrequentCharacterPerPos);
    }

    #[ignore]
    #[test]
    // Average attempts = 3.667; 19 (0.821%) failed games (> 6 attempts):
    // 1: 1, 2: 148, 3: 903, 4: 929, 5: 263, 6: 52, 7: 15, 8: 2, 9: 2
    fn auto_play_most_frequent_characters_per_pos_high_variety_word() {
        autoplay_and_print_stats(MostFrequentCharacterPerPosHighVarietyWord);
    }

    #[ignore]
    #[test]
    // Average attempts = 3.951; 54 (2.333%) failed games (> 6 attempts):
    // 2: 50, 3: 829, 4: 873, 5: 390, 6: 119, 7: 34, 8: 15, 9: 5
    fn auto_play_most_frequent_characters_of_words() {
        autoplay_and_print_stats(MostFrequentCharactersOfRemainingWords);
    }

    #[ignore]
    #[test]
    // Average attempts = 3.861; 32 (1.382%) failed games (> 6 attempts):
    // 2: 78, 3: 813, 4: 932, 5: 380, 6: 80, 7: 22, 8: 8, 9: 2
    fn auto_play_most_frequent_unused_characters() {
        let words = Words::new(English);
        autoplay_and_print_stats(MostFrequentUnusedCharacters::new(&words.guesses));
    }
    struct MostFrequentUnusedCharacters<'w> {
        combined_global_char_count_sums_by: HashMap<&'w Word, usize>,
    }
    impl<'w> MostFrequentUnusedCharacters<'w> {
        #[allow(clippy::ptr_arg)] // for global_character_counts_in defined for Vec<Guess> not &[Guess]
        fn new(guesses: &'w Vec<Guess>) -> Self {
            let global_count_by_char: HashMap<char, usize> =
                guesses.global_character_counts_in(&ALL_POS);
            let combined_global_char_count_sums_by: HashMap<_, usize> = guesses
                .par_iter()
                .map(|word| {
                    let count = word
                        .unique_chars_in(&ALL_POS)
                        .iter()
                        .map(|c| global_count_by_char[c])
                        .sum::<usize>();
                    (word, count)
                })
                .collect();
            MostFrequentUnusedCharacters {
                combined_global_char_count_sums_by,
            }
        }
    }
    impl<'w> TryToPickWord for MostFrequentUnusedCharacters<'w> {
        fn pick(&self, game: &Wordle) -> Option<Guess> {
            if game.solutions.len() < 10 {
                return None;
            };
            let words_with_most_new_chars: Vec<&Word> =
                words_with_most_new_chars(&game.guessed_chars(), game.allowed())
                    .into_iter()
                    .map(|(_, word)| word)
                    .collect();

            let mut scores: Vec<_> = words_with_most_new_chars
                .iter()
                .map(|&word| {
                    let count = self.combined_global_char_count_sums_by[word];
                    (word, count)
                })
                .collect();
            if game.print_output {
                scores.sort_asc();
                println!("best unplayed words {}", scores.to_string(5));
            }
            scores.highest()
        }
    }
    fn words_with_most_new_chars<'w>(
        used_chars: &HashSet<char>,
        words: Vec<&'w Word>,
    ) -> Vec<(Vec<char>, &'w Word)> {
        let new: Vec<(Vec<char>, &Word)> = words
            .into_iter()
            .map(|word| {
                let mut unique_unused_chars: Vec<char> = word
                    .unique_chars_in(&ALL_POS)
                    .into_iter()
                    .filter(|c| !used_chars.contains(c))
                    .collect();
                unique_unused_chars.sort_unstable();
                (unique_unused_chars, word)
            })
            .collect();
        let max = new.iter().map(|(uc, _)| uc.len()).max().unwrap();
        // println!("most new chars = {}", max);
        if max == 0 {
            return vec![]; // No new chars is not helpful. Returning empty vec signals to use fallback
        }
        new.into_iter().filter(|(ch, _)| ch.len() == max).collect()
    }

    #[ignore]
    #[test]
    // Average attempts = 4.004; 40 (1.728%) failed games (> 6 attempts):
    // 1: 1, 2: 126, 3: 632, 4: 887, 5: 495, 6: 134, 7: 29, 8: 8, 9: 3
    fn auto_play_most_other_words_in_at_least_one_open_position() {
        autoplay_and_print_stats(MatchingMostOtherWordsInAtLeastOneOpenPosition);
    }

    #[ignore]
    #[test]
    // Average attempts = 3.794; 29 (1.253%) failed games (> 6 attempts):
    // 1: 1, 2: 114, 3: 856, 4: 886, 5: 341, 6: 88, 7: 23, 8: 6
    fn auto_play_most_other_words_in_at_least_one_open_position_high_variety_word() {
        autoplay_and_print_stats(MatchingMostOtherWordsInAtLeastOneOpenPositionHighVarietyWord);
    }

    fn autoplay_and_print_stats<S: TryToPickWord + Sync>(strategy: S) {
        autoplay_and_print_stats_with_language(strategy, English);
    }
    fn autoplay_and_print_stats_with_language<S: TryToPickWord + Sync>(
        strategy: S,
        language: Language,
    ) {
        let words = Words::new(language);
        let cache = SolutionsByHintByGuess::of(&words);
        // let secrets = ["piper".to_word()];
        let mut secrets: Vec<_> = words.secrets.iter().collect();
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
        print_stats(attempts);
    }

    fn print_stats(attempts: Vec<usize>) {
        let mut counts_by_attempts: HashMap<usize, usize> = HashMap::new();
        for attempt in attempts {
            *counts_by_attempts.entry(attempt).or_default() += 1;
        }
        let mut attempt_counts: Vec<_> = counts_by_attempts.into_iter().collect();
        attempt_counts.sort_unstable();
        let total = attempt_counts.iter().map(|&(_, cnt)| cnt).sum::<usize>() as f64;
        let average = attempt_counts
            .iter()
            .map(|&(attempts, cnt)| attempts as f64 * cnt as f64)
            .sum::<f64>()
            / total;
        let failures = attempt_counts
            .iter()
            .filter(|&&(attempts, _)| attempts > MAX_ATTEMPTS)
            .map(|&(_, cnt)| cnt)
            .sum::<usize>();
        let stats = attempt_counts
            .iter()
            .map(|(attempts, cnt)| format!("{}: {}", attempts, cnt))
            .collect::<Vec<_>>()
            .join(", ");

        print!("\n{:.2} average attempts; {}", average, stats);
        if failures > 0 {
            let percent_failed = 100.0 * failures as f64 / total;
            println!("; {} ({:.2}%) failures", failures, percent_failed)
        } else {
            println!();
        }
    }

    #[ignore]
    // Takes around 6s for the 2'315 solution words
    #[test]
    fn test_word_that_results_in_fewest_remaining_possible_words() {
        let words = Words::new(English);
        let cache = SolutionsByHintByGuess::of(&words);
        let game = Wordle::with(&words, &cache);
        let word = WordThatResultsInFewestRemainingSolutions.pick(&game);

        // Worst:
        // - 701.8 jazzy
        // - 723.5 fluff
        // - 728.0 fizzy
        // - 734.7 jiffy
        // - 735.8 civic
        // - 777.8 puppy
        // - 781.7 mamma
        // - 815.4 vivid
        // - 820.7 mummy
        // - 855.7 fuzzy

        // Best:
        // - 71.57 slate
        // - 71.29 stare
        // - 71.10 snare
        // - 70.22 later
        // - 70.13 saner
        // - 69.99 alter
        // - 66.02 arose
        // - 63.78 irate
        // - 63.73 arise
        // - 61.00 raise

        assert_eq!(word.unwrap().to_string(), "raise");
    }

    #[ignore]
    // Takes around a minute for the 12'972 words in the combined list
    #[test]
    fn test_word_that_results_in_fewest_remaining_possible_words_for_full_word_list() {
        let words = Words::new(English);
        let cache = SolutionsByHintByGuess::of(&words);
        let game = Wordle::with(&words, &cache);
        let word = WordThatResultsInFewestRemainingSolutions.pick(&game);

        // Worst:
        // - 4935.9 jugum
        // - 4955.1  yukky
        // - 4975.4 bubby
        // - 5027.8 cocco
        // - 5041.8 fuzzy
        // - 5226.3 immix
        // - 5233.3 hyphy
        // - 5379.7 gyppy
        // - 5391.8 xylyl
        // - 5396.1 fuffy

        // Best:
        // - 315.13 serai
        // - 314.73 arles
        // - 311.36 rates
        // - 309.73 aeros
        // - 305.55 nares
        // - 304.76 reais
        // - 303.83 soare
        // - 302.49 tares
        // - 292.11 rales
        // - 288.74 lares

        assert_eq!(word.unwrap().to_string(), "lares");
    }

    #[ignore]
    // Takes around 24s (parallel) guessing with 12'972 combined words in 2'315 solutions.
    #[test]
    fn test_word_from_combined_list_that_results_in_fewest_remaining_possible_solution_words() {
        let words = Words::new(English);
        let cache = SolutionsByHintByGuess::of(&words);
        let game = Wordle::with(&words, &cache);
        let word = WordThatResultsInFewestRemainingSolutions.pick(&game);

        // Using 12'972 combined words on 2'315 solutions
        // Remaining worst:
        // 862 zoppo, 870 kudzu, 871 susus, 878 yukky, 883 fuffy,
        // 886 gyppy, 901 jugum, 903 jujus, 925 qajaq, 967 immix

        // Remaining best:
        // 67 raine, 66 arose, 65 ariel, 64 orate, 64 irate,
        // 64 arise, 62 soare, 61 raile, 61 raise, 60 roate

        assert_eq!(word.unwrap().to_string(), "roate");
    }

    #[ignore]
    #[test]
    fn test_pick_word_that_exactly_matches_most_others_in_at_least_one_open_position() {
        let words = Words::new(English);
        let cache = SolutionsByHintByGuess::of(&words);
        let game = Wordle::with(&words, &cache);
        let word = MatchingMostOtherWordsInAtLeastOneOpenPosition.pick(&game);
        assert_eq!(word.unwrap().to_string(), "sauce");
    }

    #[ignore]
    #[test]
    fn test_char_hint_values() {
        assert_eq!(Hint::from('⬛').value(), 0);
        assert_eq!(Hint::from('🟨').value(), 1);
        assert_eq!(Hint::from('🟩').value(), 2);
    }

    #[ignore]
    #[test]
    #[allow(clippy::identity_op)] // For the 1_usize * a
    fn test_word_hint_values() {
        let value =
            |a, b, c, d, e| -> HintValue { 81_u8 * a + 27_u8 * b + 9_u8 * c + 3_u8 * d + 1_u8 * e };
        assert_eq!(value(0, 0, 0, 0, 0), Hints::from("⬛⬛⬛⬛⬛").value());
        assert_eq!(value(1, 1, 1, 1, 1), Hints::from("🟨🟨🟨🟨🟨").value());
        assert_eq!(value(2, 2, 2, 2, 2), Hints::from("🟩🟩🟩🟩🟩").value());

        assert_eq!(value(0, 1, 0, 2, 1), Hints::from("⬛🟨⬛🟩🟨").value());
        assert_eq!(value(1, 0, 1, 2, 0), Hints::from("🟨⬛🟨🟩⬛").value());
        assert_eq!(value(0, 2, 0, 0, 2), Hints::from("⬛🟩⬛⬛🟩").value());
        assert_eq!(value(1, 0, 0, 2, 0), Hints::from("🟨⬛⬛🟩⬛").value());
        assert_eq!(value(0, 0, 0, 2, 1), Hints::from("⬛⬛⬛🟩🟨").value());
        assert_eq!(value(1, 0, 2, 0, 0), Hints::from("🟨⬛🟩⬛⬛").value());
        assert_eq!(value(0, 1, 2, 0, 0), Hints::from("⬛🟨🟩⬛⬛").value());
    }

    #[ignore]
    #[test]
    fn test_get_hint() {
        let hint = "guest".to_word().get_hint(&"truss".to_word());
        assert_eq!("⬛🟨⬛🟩🟨", hint.to_string());

        let hint = "briar".to_word().get_hint(&"error".to_word());
        assert_eq!("⬛🟩⬛⬛🟩", hint.to_string());

        let hint = "sissy".to_word().get_hint(&"truss".to_word());
        assert_eq!("🟨⬛⬛🟩⬛", hint.to_string());

        let hint = "eject".to_word().get_hint(&"geese".to_word());
        assert_eq!("🟨⬛🟩⬛⬛", hint.to_string());

        let hint = "three".to_word().get_hint(&"beret".to_word());
        assert_eq!("🟨⬛🟩🟩🟨", hint.to_string());
    }

    #[ignore]
    #[test]
    fn lowest_total_number_of_remaining_solutions_only_counts_remaining_viable_solutions() {
        let secrets: HashSet<Secret> = ["augur", "briar", "friar", "lunar", "sugar"]
            .iter()
            .map(|w| w.to_word())
            .collect();
        let guesses: Vec<Guess> = ["fubar", "rural", "aurar", "goier", "urial"]
            .iter()
            .map(|w| w.to_word())
            .collect();
        let mut words = Words::new(English);
        words.guesses = guesses;
        words.secrets = secrets;
        let cache = SolutionsByHintByGuess::of(&words);
        let game = Wordle::with(&words, &cache);
        let allowed = &game.words.guesses;
        let mut scores = fewest_remaining_solutions_for_game(&game);
        scores.sort_asc();
        // println!("scores {}", scores.to_string(5));
        assert_eq!(scores[0], (&allowed[0], 7));
        assert_eq!(scores[1], (&allowed[1], 7));
        assert_eq!(scores[2], (&allowed[4], 7));
        assert_eq!(scores[3], (&allowed[2], 9));
        assert_eq!(scores[4], (&allowed[3], 9));
    }
}
