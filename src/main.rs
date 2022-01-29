use rand::prelude::*;
use rayon::prelude::*;
use std::cmp::{Ordering, Reverse};
use std::collections::{HashMap, HashSet};
use std::env::args;
use std::fmt::{Display, Formatter};
use std::io;
use Hint::*;
use Language::*;

const GUESSES: [&str; 2] = [
    include_str!("../data/wordlists/original/combined.txt"),
    include_str!("../data/wordlists/german/combined.txt"),
];
const SOLUTIONS: [&str; 2] = [
    include_str!("../data/wordlists/original/solutions.txt"),
    include_str!("../data/wordlists/german/solutions.txt"),
];

#[cfg(test)]
const AUTOPLAY_MAX_ATTEMPTS: usize = 10;
const MAX_ATTEMPTS: usize = 6;
const ALL_POS: [usize; 5] = [0, 1, 2, 3, 4];

type Word = Vec<char>;
type Guess = Word;
type Secret = Word;
type HintValue = u8;

#[derive(Clone)]
struct Words {
    secrets: Vec<Secret>,
    guesses: Vec<Guess>,
}
impl Words {
    fn new(lang: Language) -> Self {
        Words {
            secrets: Words::from_str(SOLUTIONS[lang as usize]),
            guesses: Words::from_str(GUESSES[lang as usize]),
        }
    }
    fn from_str(txt: &str) -> Vec<Word> {
        txt.lines().map(|w| w.to_word()).collect()
    }
}

struct Wordle<'a> {
    words: &'a Words,
    cache: &'a SolutionsByHintByGuess<'a>,
    illegal_chars: HashSet<char>,
    correct_chars: [Option<char>; 5],
    illegal_at_pos: [HashSet<char>; 5],
    mandatory_chars: HashSet<char>,
    guessed: Vec<Guess>,
    print_output: bool,
}
impl<'a> Wordle<'a> {
    fn with(words: &'a Words, cache: &'a SolutionsByHintByGuess<'a>) -> Self {
        let empty = HashSet::new;
        Wordle {
            words,
            cache,
            illegal_chars: HashSet::new(),
            correct_chars: [None; 5],
            illegal_at_pos: [empty(), empty(), empty(), empty(), empty()],
            mandatory_chars: HashSet::new(),
            guessed: Vec::new(),
            print_output: true,
        }
    }
    fn play(&mut self) {
        while !self.are_all_chars_known() && self.solutions().len() > 1 {
            self.print_remaining_word_count();

            let suggestion = self.suggest_a_word();
            let guess = self.ask_for_guess(suggestion);
            let feedback = self.ask_for_feedback(&guess);

            self.process_feedback(feedback, &guess);
            self.update_illegal_chars(&guess);
            self.guessed.push(guess);
        }
        self.print_result();
    }

    #[cfg(test)]
    #[allow(clippy::ptr_arg)] // to_string is implemented for Word but not &[char]
    fn autoplay(&mut self, secret: &Secret, strategy: &mut dyn PickWord) {
        self.print_output = false;

        while !self.are_all_chars_known() && self.guessed.len() < AUTOPLAY_MAX_ATTEMPTS {
            let guess: Guess = strategy.pick(self);

            let hints = guess.get_hint(secret);
            println!(
                "{:4} solutions left, {}. guess '{}', hint '{}', secret '{}'",
                self.solutions().len(),
                self.guessed.len(),
                guess.to_string(),
                hints,
                secret.to_string(),
            );
            for (pos, hint) in hints.hints.into_iter().enumerate() {
                self.update_illegal_mandatory_and_correct_chars(&guess, pos, hint);
            }
            self.update_illegal_chars(&guess);
            self.guessed.push(guess);
        }
        if !self.are_all_chars_known() {
            println!(
                "After {} guesses: No solutions for '{}'",
                self.guessed.len(),
                secret.to_string()
            );
        }
    }

    fn has_max_open_positions(&self, open_count: usize) -> bool {
        let known_count = 5 - open_count;
        self.correct_chars.iter().filter(|o| o.is_some()).count() >= known_count
    }

    fn are_all_chars_known(&self) -> bool {
        self.correct_chars.iter().all(|o| o.is_some())
    }
    fn print_remaining_word_count(&self) {
        let len = self.solutions().len();
        if len > 10 {
            println!("\n{} words left", len);
        } else {
            println!(
                "\n{} words left: {}",
                len,
                self.solutions()
                    .iter()
                    .map(|word| word.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
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

    fn process_feedback(&mut self, feedback: Guess, guess: &[char]) {
        for (pos, feedback) in feedback
            .iter()
            .enumerate()
            .filter(|(_, c)| c.is_ascii_alphabetic())
        {
            let hint = Self::feedback_to_hint(feedback);
            self.update_illegal_mandatory_and_correct_chars(guess, pos, hint)
        }
    }

    fn ask_for_guess(&self, suggestion: Word) -> Word {
        println!(
            "Enter your guess, or press enter to use the suggestion '{}':",
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

    fn suggest_a_word(&self) -> Word {
        self.optimized_suggestion().unwrap_or_else(|| {
            println!("falling back to random suggestion");
            PickRandomWord.pick(self)
        })
    }

    fn optimized_suggestion(&self) -> Option<Word> {
        if self.solutions().len() == 1 {
            return self.solutions().into_iter().next().cloned();
        } else if self.has_max_open_positions(1) {
            println!("Falling back to single position strategy");
            SuggestWordCoveringMostCharsInOpenPositions.pick(self)
        } else if self.wanted_chars().len() < 10 {
            WordsWithMostCharsFromRemainingSolutions.pick(self)
        } else {
            WordThatResultsInFewestRemainingSolutions.pick(self)
        }
    }

    fn open_positions(&self) -> Vec<usize> {
        self.correct_chars
            .iter()
            .enumerate()
            .filter(|(_, o)| o.is_none())
            .map(|(i, _)| i)
            .collect()
    }

    fn update_illegal_mandatory_and_correct_chars(
        &mut self,
        guess: &[char],
        pos: usize,
        hint: Hint,
    ) {
        let ch = guess[pos].to_ascii_lowercase();
        match hint {
            Illegal => {}
            WrongPos => {
                if self.print_output {
                    println!("Inserting '{}' as illegal @ {}", ch, pos);
                }
                self.illegal_at_pos[pos].insert(ch);
                self.mandatory_chars.insert(ch);
            }
            Correct => {
                if self.print_output {
                    println!("Inserting '{}' as correct character @ {}", ch, pos);
                }
                self.correct_chars[pos] = Some(ch);
                self.mandatory_chars.insert(ch);
            }
        }
    }

    fn feedback_to_hint(feedback: &char) -> Hint {
        if feedback.is_ascii_lowercase() {
            WrongPos
        } else if feedback.is_ascii_uppercase() {
            Correct
        } else {
            Illegal
        }
    }

    fn update_illegal_chars(&mut self, guess: &Guess) {
        // println!("guess {:?}", guess);
        // println!("self.mandatory_chars {:?}", self.mandatory_chars);
        // println!("self.correct_chars {:?}", self.correct_chars);

        let open_positions = self.open_positions();
        for (_, c) in guess
            .iter()
            .enumerate()
            .filter(|(_, c)| !self.mandatory_chars.contains(c))
            .filter(|&(i, c)| self.correct_chars[i] != Some(*c))
        {
            if !self.correct_chars.iter().any(|&o| o == Some(*c)) {
                if self.print_output {
                    println!("Inserting globally illegal char '{}'", c);
                }
                self.illegal_chars.insert(*c);
            } else {
                if self.print_output {
                    println!("Inserting '{}' as illegal @ {:?}", c, open_positions);
                }
                for i in &open_positions {
                    self.illegal_at_pos[*i].insert(*c);
                }
            }
        }
    }

    fn allowed(&self) -> Vec<&Guess> {
        self.words
            .guesses
            .iter()
            .filter(|guess| !self.guessed.contains(guess))
            .collect()
    }
    fn solutions(&self) -> Vec<&Secret> {
        self.words
            .secrets
            .iter()
            .filter(|secret| {
                !self
                    .illegal_chars
                    .iter()
                    .any(|illegal| secret.contains(illegal))
            })
            .filter(|secret| {
                self.mandatory_chars
                    .iter()
                    .all(|mandatory| secret.contains(mandatory))
            })
            .filter(|secret| {
                self.correct_chars
                    .iter()
                    .enumerate()
                    .filter(|(_, o)| o.is_some())
                    .all(|(i, &o)| secret[i] == o.unwrap())
            })
            .filter(|secret| {
                !secret
                    .iter()
                    .enumerate()
                    .any(|(i, c)| self.illegal_at_pos[i].contains(c))
            })
            .filter(|secret| !self.guessed.contains(secret))
            .collect()
    }

    fn print_result(&self) {
        let solutions = self.solutions();
        if solutions.len() == 1 {
            println!(
                "\nThe only word left in the list is '{}'",
                solutions[0].to_string()
            );
        } else if self.correct_chars.iter().all(|o| o.is_some()) {
            let word: String = self.correct_chars.iter().map(|c| c.unwrap()).collect();
            println!("\nThe word is '{}'", word);
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
        self.solutions().into_iter().flatten().cloned().collect()
    }

    fn guessed_chars(&self) -> HashSet<char> {
        self.guessed.iter().flatten().cloned().collect()
    }

    fn attempts(&self) -> usize {
        self.guessed.len()
    }
}

struct HintsBySolutionAndGuess<'a> {
    by_solution_by_guess: HashMap<&'a Guess, HashMap<&'a Secret, Hints>>,
}
impl<'a> HintsBySolutionAndGuess<'a> {
    fn of(words: &'a Words) -> Self {
        HintsBySolutionAndGuess::new(&words.guesses, &words.secrets)
    }
    fn new(guesses: &'a [Guess], secrets: &'a [Secret]) -> Self {
        HintsBySolutionAndGuess {
            by_solution_by_guess: guesses
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

struct SolutionsByHintByGuess<'a> {
    by_hint_by_guess: HashMap<&'a Guess, HashMap<HintValue, HashSet<&'a Secret>>>,
}
impl<'a> SolutionsByHintByGuess<'a> {
    fn of(words: &'a Words) -> Self {
        SolutionsByHintByGuess::new(&words.guesses, &words.secrets)
    }
    fn new(guesses: &'a [Guess], secrets: &'a [Secret]) -> Self {
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
        self.iter().collect::<String>()
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

struct PickRandomWord;
impl PickWord for PickRandomWord {
    fn pick(&mut self, game: &Wordle) -> Word {
        game.solutions()
            .into_iter()
            .choose(&mut ThreadRng::default())
            .cloned()
            .unwrap()
    }
}

struct ChainedStrategies<'a, F: PickWord> {
    strategies: Vec<&'a dyn TryToPickWord>,
    fallback: F,
}
#[cfg(test)]
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
        self.fallback.pick(game)
    }
}

trait TryToPickWord {
    fn pick(&self, game: &Wordle) -> Option<Guess>;
}

struct PickRandomSolutionIfEnoughAttemptsLeft;
impl TryToPickWord for PickRandomSolutionIfEnoughAttemptsLeft {
    fn pick(&self, game: &Wordle) -> Option<Guess> {
        if game.attempts() + game.solutions().len() <= MAX_ATTEMPTS {
            game.solutions()
                .into_iter()
                .choose(&mut ThreadRng::default())
                .cloned()
        } else {
            None
        }
    }
}
struct WordThatResultsInFewestRemainingSolutions;
impl TryToPickWord for WordThatResultsInFewestRemainingSolutions {
    fn pick(&self, game: &Wordle) -> Option<Guess> {
        let mut scores = lowest_total_number_of_remaining_solutions(game);
        if game.print_output {
            scores.sort_asc();
            println!("Best (fewest remaining solutions): {}", scores.to_string(5));
        }
        // scores.sort_desc();
        // println!("Remaining worst: {}", scores.to_string());

        scores.lowest()
    }
}

fn lowest_total_number_of_remaining_solutions<'a>(game: &'a Wordle) -> Vec<(&'a Guess, usize)> {
    let mut scores: Vec<_> = game
        .allowed()
        .into_par_iter()
        .map(|guess| {
            let count: usize = game
                .solutions()
                .iter()
                .map(|secret| {
                    let hint = guess.get_hint(secret);
                    game.cache.by_hint_by_guess[guess][&hint.value()].len()
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
        let freq = game.solutions().global_character_counts_in(positions);

        let scores: Vec<_> = game
            .solutions()
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
        // scores.sort_desc();
        // println!("{}", scores.to_string());
        scores.highest()
    }
}
struct MostFrequentGlobalCharacterHighVarietyWord;
impl TryToPickWord for MostFrequentGlobalCharacterHighVarietyWord {
    fn pick(&self, game: &Wordle) -> Option<Guess> {
        let positions = &game.open_positions();
        let solutions = game.solutions();
        let high_variety_words = solutions.high_variety_words(positions);
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
        let played_chars: HashSet<char> = game.guessed_chars();
        let remaining_words_with_unique_unplayed_chars: Vec<_> = game
            .allowed()
            .iter()
            .filter(|&word| {
                word.unique_chars_in(&ALL_POS).len() == 5
                    && !word.iter().any(|c| played_chars.contains(c))
            })
            .cloned()
            .collect();

        let positions = &ALL_POS;
        let freq = remaining_words_with_unique_unplayed_chars.global_character_counts_in(positions);
        let scores: Vec<_> = remaining_words_with_unique_unplayed_chars
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
        let already_know_all_chars = game.mandatory_chars.len() == 5;
        let very_few_solutions_left = game.solutions().len() < 10;
        if already_know_all_chars || very_few_solutions_left {
            return None;
        };
        let used_chars: HashSet<char> = game.guessed_chars();
        let words_with_most_new_chars: Vec<&Word> =
            words_with_most_new_chars(&used_chars, game.allowed())
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

struct SuggestWordCoveringMostCharsInOpenPositions;
impl TryToPickWord for SuggestWordCoveringMostCharsInOpenPositions {
    fn pick(&self, game: &Wordle) -> Option<Guess> {
        let candidates: HashSet<char> = game
            .solutions()
            .iter()
            .flat_map(|solution| solution.unique_chars_in(&game.open_positions()))
            .collect();
        let scores: Vec<(&Word, usize)> = game
            .allowed()
            .into_iter()
            .map(|word| {
                let count = word
                    .unique_chars_in(&ALL_POS)
                    .into_iter()
                    .filter(|c| candidates.contains(c))
                    .count();
                (word, count)
            })
            .collect();
        scores.highest()
    }
}

struct WordsWithMostCharsFromRemainingSolutions;
impl TryToPickWord for WordsWithMostCharsFromRemainingSolutions {
    fn pick(&self, game: &Wordle) -> Option<Guess> {
        let wanted_chars: HashSet<char> = game.wanted_chars();
        let scores: Vec<(&Word, usize)> =
            words_with_most_wanted_chars(&wanted_chars, game.allowed());
        if game.print_output {
            println!(
                "Words with most of wanted chars {:?} are:\n  {}",
                wanted_chars,
                scores.to_string(5)
            );
        }
        scores.highest()
    }
}

struct MostFrequentCharacterPerPos;
impl TryToPickWord for MostFrequentCharacterPerPos {
    fn pick(&self, game: &Wordle) -> Option<Guess> {
        let words = game.solutions();
        let positions = &game.open_positions();
        let counts = words.character_counts_per_position_in(positions);

        let scores: Vec<_> = words
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

struct MostFrequentCharacterPerPosHighVarietyWord;
impl TryToPickWord for MostFrequentCharacterPerPosHighVarietyWord {
    fn pick(&self, game: &Wordle) -> Option<Guess> {
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
        scores.highest()
    }
}

struct MatchingMostOtherWordsInAtLeastOneOpenPosition;
impl TryToPickWord for MatchingMostOtherWordsInAtLeastOneOpenPosition {
    fn pick(&self, game: &Wordle) -> Option<Guess> {
        let positions = &game.open_positions();
        // for each word, find out how many other words it matches in any open position
        let mut scores: Vec<_> = game
            .solutions()
            .into_iter()
            .map(|word| (word, 0_usize))
            .collect();
        let open_chars: Vec<_> = game
            .solutions()
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
        scores.highest()
    }
}

trait HighVarietyWords {
    fn high_variety_words(&self, open_positions: &[usize]) -> Vec<&Word>;
}
impl HighVarietyWords for Vec<&Word> {
    fn high_variety_words(&self, open_positions: &[usize]) -> Vec<&Word> {
        self.iter()
            .filter(|&&word| word.unique_chars_in(open_positions).len() == open_positions.len())
            .cloned()
            .collect()
    }
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
}
impl CalcHintValue for Hints {
    fn value(&self) -> HintValue {
        const MULTIPLIERS: [HintValue; 5] = [81, 27, 9, 3, 1];
        self.hints
            .iter()
            .enumerate()
            .map(|(i, h)| MULTIPLIERS[i] * h.value())
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
    use super::*;

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
        let totals = lowest_total_number_of_remaining_solutions(&game);
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
        solutions: &[Word],
    ) -> [Vec<usize>; 243] {
        const EMPTY_VEC: Vec<usize> = Vec::new();
        let mut hint_buckets = [EMPTY_VEC; 243];
        for (i, solution) in solutions.iter().enumerate() {
            let hint = guess.get_hint(solution);
            hint_buckets[hint.value() as usize].push(i);
        }
        hint_buckets
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

    #[ignore] // 1s for 1, 5s for 5, 10s for 10
    #[test]
    fn does_hint_cache_help_part_1() {
        let words = Words::new(English);
        for _ in 0..10 {
            let sum: usize = words
                .guesses
                .par_iter()
                .map(|guess| {
                    words
                        .secrets
                        .iter()
                        .map(|secret| guess.get_hint(secret).value() as usize)
                        .sum::<usize>()
                })
                .sum();
            assert_eq!(1_056_428_862, sum); // 1056428862
        }
    }

    #[ignore] // 3s for 1, 4.2s for 5, 6.1s for 10
    #[test]
    fn does_hint_cache_help_part_2() {
        let words = Words::new(English);
        let hints = HintsBySolutionAndGuess::of(&words);
        for _ in 0..10 {
            let sum: usize = words
                .guesses
                .par_iter()
                .map(|guess| {
                    words
                        .secrets
                        .iter()
                        .map(|secret| hints.by_solution_by_guess[guess][secret].value() as usize)
                        .sum::<usize>()
                })
                .sum();
            assert_eq!(1_056_428_862, sum); // 1056428862
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

    #[ignore] // ~3s
    #[test]
    // Top 5: 139883 roate, 141217 raise, 141981 raile, 144227 soare, 147525 arise
    fn find_optimal_first_word_english() {
        let words = Words::new(English);
        let cache = SolutionsByHintByGuess::of(&words);
        let game = Wordle::with(&words, &cache);
        let scores = lowest_total_number_of_remaining_solutions(&game);
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
        let scores = lowest_total_number_of_remaining_solutions(&game);
        println!("scores {}", scores.to_string(5));
        let optimal = scores.lowest().unwrap();
        assert_eq!("raine".to_word(), optimal);
    }

    #[ignore] // ~10s for 5 single German words, ~1min 51s for 5 single English words
    #[test]
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
        let mut scores = lowest_total_number_of_remaining_solutions(&game);
        println!("Best 1. guesses: {}", scores.to_string(5));
        scores.sort_asc();

        let top_pick_count = 1;
        for (guess1, _) in scores.into_iter().take(top_pick_count) {
            let guessed = [guess1];
            let scores = find_best_next_guesses(&game, &guessed);

            println!(
                "Best 2. guesses after 1. '{}': {}",
                guess1.to_string(),
                scores.to_string(5)
            );

            for (guess2, _) in scores.into_iter().take(top_pick_count) {
                let guessed = [guess1, guess2];
                let scores = find_best_next_guesses(&game, &guessed);
                println!(
                    "Best 3. guesses after 1. '{}' and 2. '{}': {}",
                    guess1.to_string(),
                    guess2.to_string(),
                    scores.to_string(5)
                );

                for (guess3, _) in scores.into_iter().take(top_pick_count) {
                    let guessed = [guess1, guess2, guess3];
                    let scores = find_best_next_guesses(&game, &guessed);
                    println!(
                        "Best 4. guesses after 1. '{}' and 2. '{}' and 3. '{}': {}",
                        guess1.to_string(),
                        guess2.to_string(),
                        guess3.to_string(),
                        scores.to_string(5)
                    );

                    for (guess4, _) in scores.into_iter().take(top_pick_count) {
                        let guessed = [guess1, guess2, guess3, guess4];
                        let scores = find_best_next_guesses(&game, &guessed);
                        println!(
                            "Best 5. guesses after 1. '{}' and 2. '{}' and 3. '{}' and 4. '{}': {}",
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
                    .solutions()
                    .into_iter()
                    .map(|secret| {
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

    #[ignore] // 2-3s
    #[test]
    fn hint_by_solution_by_guess() {
        let words = Words::new(English);
        let hints = HintsBySolutionAndGuess::of(&words);
        assert_eq!(hints.by_solution_by_guess.len(), 12972);
        assert!(hints
            .by_solution_by_guess
            .iter()
            .all(|(_, hint_by_secret)| hint_by_secret.len() == 2315));
    }

    #[ignore] // ~63min
    #[test]
    // Average attempts = 3.545; 0 (0.000%) failed games (> 6 attempts):
    // 2: 64, 3: 1088, 4: 1007, 5: 149, 6: 7
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
        let attempts: Vec<usize> = words
            .secrets
            .iter()
            .map(|secret| {
                let mut game = Wordle::with(&words, &cache);
                let mut strategy = ChainedStrategies::new(
                    vec![&PickRandomSolutionIfEnoughAttemptsLeft, &strategy],
                    PickRandomWord,
                );
                game.autoplay(secret, &mut strategy);
                game.attempts()
            })
            .collect();
        print_stats(attempts);
    }

    fn print_stats(attempts: Vec<usize>) {
        let mut attempts_map: HashMap<usize, usize> = HashMap::new();
        for round in attempts {
            *attempts_map.entry(round).or_default() += 1;
        }
        let mut attempts: Vec<_> = attempts_map.into_iter().collect();
        attempts.sort_unstable();
        let total = attempts.iter().map(|&(_, cnt)| cnt).sum::<usize>() as f64;
        let avg = attempts
            .iter()
            .map(|&(rnd, cnt)| rnd as f64 * cnt as f64)
            .sum::<f64>()
            / total;
        let failed = attempts
            .iter()
            .filter(|&&(rnd, _)| rnd > 6)
            .map(|&(_, cnt)| cnt)
            .sum::<usize>();
        let stats = attempts
            .iter()
            .map(|(rnd, cnt)| format!("{}: {}", rnd, cnt))
            .collect::<Vec<_>>()
            .join(", ");
        println!(
            "\nAverage attempts = {:.3}; {} ({:.3}%) failed games (> 6 attempts):\n{}\n",
            avg,
            failed,
            100.0 * failed as f64 / total,
            stats
        );
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
        let solutions: Vec<Secret> = ["augur", "briar", "friar", "lunar", "sugar"]
            .iter()
            .map(|w| w.to_word())
            .collect();
        let guesses: Vec<Guess> = ["fubar", "rural", "aurar", "goier", "urial"]
            .iter()
            .map(|w| w.to_word())
            .collect();
        let mut words = Words::new(English);
        words.guesses = guesses;
        words.secrets = solutions;
        let cache = SolutionsByHintByGuess::of(&words);
        let game = Wordle::with(&words, &cache);
        let allowed = &game.words.guesses;
        let mut scores = lowest_total_number_of_remaining_solutions(&game);
        scores.sort_asc();
        // println!("scores {}", scores.to_string(5));
        assert_eq!(scores[0], (&allowed[0], 7));
        assert_eq!(scores[1], (&allowed[1], 7));
        assert_eq!(scores[2], (&allowed[4], 7));
        assert_eq!(scores[3], (&allowed[2], 9));
        assert_eq!(scores[4], (&allowed[3], 9));
    }
}
