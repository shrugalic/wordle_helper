use rand::prelude::*;
use rayon::prelude::*;
use std::cmp::{Ordering, Reverse};
use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter};
use std::io;
use Hint::*;

#[macro_use]
extern crate lazy_static;

const ENGLISH: bool = true;

const GUESSES_LIST: &str = if ENGLISH {
    include_str!("../data/wordlists/original/combined.txt")
} else {
    include_str!("../data/wordlists/german/combined.txt")
};
const SOLUTIONS_LIST: &str = if ENGLISH {
    include_str!("../data/wordlists/original/solutions.txt")
} else {
    include_str!("../data/wordlists/german/solutions.txt")
};
lazy_static! {
    static ref SOLUTIONS: Vec<Word> = SOLUTIONS_LIST.lines().map(|w| w.to_word()).collect();
    static ref GUESSES: Vec<Word> = GUESSES_LIST.lines().map(|w| w.to_word()).collect();
    static ref COMBINED_GLOBAL_CHAR_COUNTS_BY_CHAR: HashMap<char, usize> =
        GUESSES.as_slice().global_character_counts_in(&ALL_POS);
    static ref COMBINED_GLOBAL_CHAR_COUNT_SUMS_BY_WORD: HashMap<Word, usize> = GUESSES
        .par_iter()
        .map(|word| {
            let count = word
                .unique_chars_in(&ALL_POS)
                .iter()
                .map(|c| COMBINED_GLOBAL_CHAR_COUNTS_BY_CHAR[c])
                .sum::<usize>();
            (word.to_vec(), count)
        })
        .collect();
    static ref HINT_BY_SOLUTION_BY_GUESS: HashMap<&'static Guess, HashMap<&'static Solution, Hints>> =
        GUESSES
            .par_iter()
            .map(|guess| {
                let hints_by_solution = SOLUTIONS
                    .iter()
                    .map(|solution| (solution, determine_hint(guess, solution)))
                    .collect::<HashMap<&Solution, Hints>>();
                (guess, hints_by_solution)
            })
            .collect();
    static ref SOLUTIONS_BY_HINT_BY_GUESS: HashMap<&'static Guess, HashMap<HintValue, HashSet<&'static Solution>>> =
        GUESSES
            .par_iter()
            .map(|guess| {
                let mut solution_by_value: HashMap<HintValue, HashSet<&Solution>> = HashMap::new();
                for solution in SOLUTIONS.iter() {
                    solution_by_value
                        .entry(determine_hint(guess, solution).value())
                        .or_default()
                        .insert(solution);
                }
                (guess, solution_by_value)
            })
            .collect();
    static ref SOLUTIONS_BY_SECRET_BY_GUESS: HashMap<&'static Guess, HashMap<&'static Solution, &'static HashSet<&'static Solution>>> =
        GUESSES
            .par_iter()
            .map(|guess| {
                let mut solutions_by_secret: HashMap<&Solution, &HashSet<&Solution>> =
                    HashMap::new();
                for secret in SOLUTIONS.iter() {
                    let hint = determine_hint(guess, secret).value();
                    let solutions = &SOLUTIONS_BY_HINT_BY_GUESS[guess][&hint];
                    solutions_by_secret.insert(secret, solutions);
                }
                (guess, solutions_by_secret)
            })
            .collect();
}

const ALL_POS: [usize; 5] = [0, 1, 2, 3, 4];
#[cfg(test)]
const AUTOPLAY_MAX_ATTEMPTS: usize = 10;

type Word = Vec<char>;
type Guess = Word;
type Solution = Word;
type HintValue = u8;

// Helper for https://www.powerlanguage.co.uk/wordle/
#[derive(Debug)]
struct Wordle {
    solutions: Vec<Word>,
    allowed: Vec<Word>,
    illegal_chars: HashSet<char>,
    correct_chars: [Option<char>; 5],
    illegal_at_pos: [HashSet<char>; 5],
    mandatory_chars: HashSet<char>,
    guessed: Vec<Word>,
    rng: ThreadRng,
    print_output: bool,
}
impl Wordle {
    fn new(solutions: &[Solution]) -> Self {
        let empty = HashSet::new;
        Wordle {
            solutions: solutions.to_vec(),
            allowed: GUESSES.clone(),
            illegal_chars: HashSet::new(),
            correct_chars: [None; 5],
            illegal_at_pos: [empty(), empty(), empty(), empty(), empty()],
            mandatory_chars: HashSet::new(),
            guessed: Vec::new(),
            rng: thread_rng(),
            print_output: true,
        }
    }
    fn play(&mut self) {
        while !self.is_game_over() {
            self.print_remaining_word_count();
            self.single_round();
        }
        self.print_result();
    }
    #[cfg(test)]
    #[allow(clippy::ptr_arg)] // to_string is implemented for Word but not &[char]
    fn autoplay<S: PickBestWord>(&mut self, secret: &Solution, strategy: &S) -> usize {
        self.print_output = false;
        let mut attempts = 0;
        while !self.is_game_over() && attempts < AUTOPLAY_MAX_ATTEMPTS {
            let guess = if self.solutions.len() <= 6_usize.saturating_sub(attempts) {
                None
            // } else if self.has_max_open_positions(1) {
            //     self.suggest_word_covering_chars_in_open_positions()
            } else {
                strategy
                    .pick(self)
                    .or_else(|| MostFrequentGlobalCharacter.pick(self))
            }
            .unwrap_or_else(|| self.random_suggestion());

            attempts += 1;
            let hint = determine_hint(&guess, secret);
            println!(
                "{:4} solutions left, {}. guess '{}', hint '{}', secret '{}'",
                self.solutions.len(),
                attempts,
                guess.to_string(),
                hint.to_string(),
                secret.to_string(),
            );
            for (pos, hint) in hint.hints.into_iter().enumerate() {
                self.update_illegal_mandatory_and_correct_chars(&guess, pos, hint);
            }
            if self.is_game_over() {
                break;
            }
            self.move_from_allowed_to_guessed(&guess);
            self.update_illegal_chars(guess);
            self.update_possible_solutions();
        }
        if self.solutions.len() == 1 {
            println!(
                "After {} guesses: The only word left in the list is '{}'",
                attempts,
                self.solutions[0].to_string()
            );
        } else if self.correct_chars.iter().all(|o| o.is_some()) {
            let word: String = self.correct_chars.iter().map(|c| c.unwrap()).collect();
            println!("After {} guesses: The word is '{}'", attempts, word);
        } else {
            println!(
                "After {} guesses: No solutions for '{}'",
                attempts,
                secret.to_string()
            );
            attempts = 2 * AUTOPLAY_MAX_ATTEMPTS;
        }
        attempts
    }

    fn has_max_open_positions(&self, open_count: usize) -> bool {
        let known_count = 5 - open_count;
        self.correct_chars.iter().filter(|o| o.is_some()).count() >= known_count
    }

    fn suggest_word_covering_chars_in_open_positions(&self) -> Option<Word> {
        let positions = self.open_positions();
        let candidates: HashSet<char> = self
            .solutions
            .iter()
            .flat_map(|solution| solution.unique_chars_in(&positions).into_iter())
            .collect();
        let scores: Vec<(&Word, usize)> = self
            .allowed
            .iter()
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

    fn is_game_over(&self) -> bool {
        self.correct_chars.iter().all(|o| o.is_some()) || self.solutions.len() <= 1
    }
    fn print_remaining_word_count(&self) {
        if self.solutions.len() > 10 {
            println!("\n{} words left", self.solutions.len());
        } else {
            println!(
                "\n{} words left: {}",
                self.solutions.len(),
                self.solutions
                    .iter()
                    .map(|word| word.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }
    fn single_round(&mut self) {
        let suggestion = self.suggest_a_word();
        let guess = self.ask_for_guess(suggestion);

        let feedback = self.ask_for_feedback(&guess);
        self.process_feedback(feedback, &guess);
        if self.is_game_over() {
            return;
        }

        self.move_from_allowed_to_guessed(&guess);
        self.update_illegal_chars(guess);
        self.update_possible_solutions();
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

    fn move_from_allowed_to_guessed(&mut self, guess: &[char]) {
        let pos = self.allowed.iter().position(|word| word == guess).unwrap();
        let guessed = self.allowed.remove(pos);
        self.guessed.push(guessed);
    }

    fn ask_for_guess(&mut self, suggestion: Word) -> Word {
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
            if !self.allowed.iter().any(|word| word == &guess) {
                println!("This word is not allowed, please enter another one, or nothing to use the suggestion:")
            } else {
                return guess;
            }
        }
    }

    fn suggest_a_word(&mut self) -> Word {
        self.optimized_suggestion().unwrap_or_else(|| {
            println!("falling back to random suggestion");
            self.random_suggestion()
        })
    }

    fn optimized_suggestion(&mut self) -> Option<Word> {
        if self.has_max_open_positions(1) {
            println!("Falling back to single position strategy");
            return self.suggest_word_covering_chars_in_open_positions();
        }
        if self.wanted_chars().len() < 10 {
            WordsWithMostCharsFromRemainingSolutions.pick(self);
        }
        if false {
            // Called just to print the top words
            MostFrequentGlobalCharacter.pick(self);
            MostFrequentCharacterPerPos.pick(self);
            MatchingMostOtherWordsInAtLeastOneOpenPosition.pick(self);

            // Called just to print the top high variety words
            MostFrequentGlobalCharacterHighVarietyWord.pick(self);
            MostFrequentCharacterPerPosHighVarietyWord.pick(self);
            MatchingMostOtherWordsInAtLeastOneOpenPositionHighVarietyWord.pick(self);

            MostFrequentCharactersOfRemainingWords.pick(self);
            MostFrequentUnusedCharacters.pick(self);
        }

        WordThatResultsInFewestRemainingSolutions.pick(self)
    }

    fn high_variety_words(&self, open_positions: &[usize]) -> Vec<Word> {
        let high_variety_words: Vec<Word> = self
            .solutions
            .iter()
            .filter(|&word| word.unique_chars_in(open_positions).len() == open_positions.len())
            .cloned()
            .collect();
        // println!(
        //     "{}/{} are 'high-variety' words with different characters in all open positions",
        //     high_variety_words.len(),
        //     self.solutions.len()
        // );
        high_variety_words
    }

    fn random_suggestion(&mut self) -> Word {
        self.solutions
            .iter()
            .choose(&mut self.rng)
            .unwrap()
            .to_vec()
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

    fn update_illegal_chars(&mut self, guess: Word) {
        // println!("guess {:?}", guess);
        // println!("self.mandatory_chars {:?}", self.mandatory_chars);
        // println!("self.correct_chars {:?}", self.correct_chars);

        let open_positions = self.open_positions();
        for (_, c) in guess
            .into_iter()
            .enumerate()
            .filter(|(_, c)| !self.mandatory_chars.contains(c))
            .filter(|&(i, c)| self.correct_chars[i] != Some(c))
        {
            if !self.correct_chars.iter().any(|&o| o == Some(c)) {
                if self.print_output {
                    println!("Inserting globally illegal char '{}'", c);
                }
                self.illegal_chars.insert(c);
            } else {
                if self.print_output {
                    println!("Inserting '{}' as illegal @ {:?}", c, open_positions);
                }
                for i in &open_positions {
                    self.illegal_at_pos[*i].insert(c);
                }
            }
        }
    }

    fn update_possible_solutions(&mut self) {
        self.solutions = self
            .solutions
            .drain(..)
            .filter(|word| {
                !self
                    .illegal_chars
                    .iter()
                    .any(|illegal| word.contains(illegal))
            })
            .filter(|word| {
                self.mandatory_chars
                    .iter()
                    .all(|mandatory| word.contains(mandatory))
            })
            .filter(|word| {
                self.correct_chars
                    .iter()
                    .enumerate()
                    .filter(|(_, o)| o.is_some())
                    .all(|(i, &o)| word[i] == o.unwrap())
            })
            .filter(|word| {
                !word
                    .iter()
                    .enumerate()
                    .any(|(i, c)| self.illegal_at_pos[i].contains(c))
            })
            .filter(|word| !self.guessed.contains(word))
            .collect();
    }

    fn print_result(&self) {
        if self.solutions.len() == 1 {
            println!(
                "\nThe only word left in the list is '{}'",
                self.solutions[0].to_string()
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
        self.solutions.iter().flatten().cloned().collect()
    }

    fn guessed_chars(&self) -> HashSet<char> {
        self.guessed.iter().flatten().cloned().collect()
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
impl CharacterCounts for &[Word] {
    fn global_character_counts_in(&self, positions: &[usize]) -> HashMap<char, usize> {
        let mut counts: HashMap<char, usize> = HashMap::new();
        for word in self.iter() {
            for i in positions {
                *counts.entry(word[*i]).or_default() += 1;
            }
        }
        // for (c, i) in counts.iter() {
        //     println!("{} * '{}'", i, c);
        // }
        counts
    }
    fn character_counts_per_position_in(&self, positions: &[usize]) -> [HashMap<char, usize>; 5] {
        let empty = || ('a'..='z').into_iter().map(|c| (c, 0)).collect();
        let mut counts: [HashMap<char, usize>; 5] = [empty(), empty(), empty(), empty(), empty()];
        for word in self.iter() {
            for i in positions {
                *counts[*i].get_mut(&word[*i]).unwrap() += 1;
            }
        }
        // for (i, count) in counts.iter().enumerate() {
        //     println!("Position[{}] character counts: {}", i, count.to_string());
        // }
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
            match b_value.partial_cmp(a_value) {
                Some(Ordering::Equal) | None => a_word.cmp(b_word),
                Some(by_value) => by_value,
            }
        });
    }
    fn sort_desc(&mut self) {
        self.sort_unstable_by(|(a_word, a_value), (b_word, b_value)| {
            match a_value.partial_cmp(b_value) {
                Some(Ordering::Equal) | None => a_word.cmp(b_word),
                Some(by_value) => by_value,
            }
        });
    }
    fn to_string(&self, count: usize) -> String {
        self.iter()
            .rev()
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
            .map(|(word, _value)| word.to_vec())
    }
    fn highest(&self) -> Option<Word> {
        self.iter()
            .max_by(
                |(a_word, a_value), (b_word, b_value)| match a_value.partial_cmp(b_value) {
                    Some(Ordering::Equal) | None => a_word.cmp(b_word),
                    Some(by_value) => by_value,
                },
            )
            .map(|(word, _value)| word.to_vec())
    }
}

trait PickBestWord {
    fn pick(&self, game: &Wordle) -> Option<Word>;
}

struct WordThatResultsInFewestRemainingSolutions;
impl PickBestWord for WordThatResultsInFewestRemainingSolutions {
    fn pick(&self, game: &Wordle) -> Option<Word> {
        let mut scores = lowest_total_number_of_remaining_solutions(&game.solutions, &game.allowed);
        if game.print_output {
            scores.sort_asc();
            println!("Best (fewest remaining solutions): {}", scores.to_string(5));
        }
        // scores.sort_desc();
        // println!("Remaining worst: {}", scores.to_string());

        scores.lowest()
    }
}

fn lowest_total_number_of_remaining_solutions<'g>(
    solutions: &[Solution],
    guesses: &'g [Guess],
) -> Vec<(&'g Guess, usize)> {
    // Access lazy_static here to initialize it before the parallel part below
    assert!(SOLUTIONS_BY_HINT_BY_GUESS.len() > 0);

    let mut scores: Vec<_> = guesses
        .iter()
        .map(|guess| {
            let count: usize = solutions
                .iter()
                .map(|secret| {
                    let solutions_by_hint = &SOLUTIONS_BY_HINT_BY_GUESS[guess];
                    let hint = determine_hint(guess, secret);
                    solutions_by_hint[&hint.value()].len()
                })
                .sum();
            (guess, count)
        })
        .collect();
    scores.sort_asc();
    scores
}

struct MostFrequentGlobalCharacter;
impl PickBestWord for MostFrequentGlobalCharacter {
    fn pick(&self, game: &Wordle) -> Option<Word> {
        let words = &game.solutions;
        let positions = &game.open_positions();
        let freq = words.as_slice().global_character_counts_in(positions);

        let scores: Vec<_> = words
            .iter()
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
impl PickBestWord for MostFrequentGlobalCharacterHighVarietyWord {
    fn pick(&self, game: &Wordle) -> Option<Word> {
        let positions = &game.open_positions();
        let mut words = &game.high_variety_words(positions);
        if words.is_empty() {
            words = &game.solutions;
        }
        let freq = words.as_slice().global_character_counts_in(positions);
        // println!("Overall character counts: {}", freq.to_string());

        let scores: Vec<_> = words
            .iter()
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

struct FixedGuessList<'a> {
    guesses: Vec<&'a str>,
}
impl<'a> FixedGuessList<'a> {
    #[cfg(test)]
    fn new(guesses: Vec<&'a str>) -> Self {
        println!("Fixed guesses {:?}\n", guesses);
        FixedGuessList { guesses }
    }
}

impl<'a> PickBestWord for FixedGuessList<'a> {
    fn pick(&self, game: &Wordle) -> Option<Word> {
        self.guesses.get(game.guessed.len()).map(|w| w.to_word())
    }
}

struct MostFrequentCharactersOfRemainingWords;
impl PickBestWord for MostFrequentCharactersOfRemainingWords {
    fn pick(&self, game: &Wordle) -> Option<Word> {
        let played_chars: HashSet<char> = game.guessed.iter().flatten().cloned().collect();
        let remaining_words_with_unique_unplayed_chars: Vec<_> = game
            .allowed
            .iter()
            .filter(|&word| {
                word.unique_chars_in(&ALL_POS).len() == 5
                    && !word.iter().any(|c| played_chars.contains(c))
            })
            .cloned()
            .collect();

        let positions = &ALL_POS;
        let freq = remaining_words_with_unique_unplayed_chars
            .as_slice()
            .global_character_counts_in(positions);
        let scores: Vec<_> = remaining_words_with_unique_unplayed_chars
            .iter()
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

struct MostFrequentUnusedCharacters;
impl PickBestWord for MostFrequentUnusedCharacters {
    fn pick(&self, game: &Wordle) -> Option<Word> {
        let already_know_all_chars = game.mandatory_chars.len() == 5;
        let very_few_solutions_left = game.solutions.len() < 10;
        if already_know_all_chars || very_few_solutions_left {
            return None;
        };
        let used_chars: HashSet<char> = game.guessed.iter().flatten().cloned().collect();
        let words_with_most_new_chars: Vec<&Word> =
            words_with_most_new_chars(&used_chars, &GUESSES)
                .into_iter()
                .map(|(_, word)| word)
                .collect();

        let mut scores: Vec<_> = words_with_most_new_chars
            .iter()
            .map(|&word| {
                let count = COMBINED_GLOBAL_CHAR_COUNT_SUMS_BY_WORD[word];
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

struct WordsWithMostCharsFromRemainingSolutions;
impl PickBestWord for WordsWithMostCharsFromRemainingSolutions {
    fn pick(&self, game: &Wordle) -> Option<Word> {
        let wanted_chars: HashSet<char> = game.wanted_chars();
        let scores: Vec<(&Word, usize)> = words_with_most_wanted_chars(&wanted_chars, &GUESSES);
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
impl PickBestWord for MostFrequentCharacterPerPos {
    fn pick(&self, game: &Wordle) -> Option<Word> {
        let words = &game.solutions;
        let positions = &game.open_positions();
        let counts = words.as_slice().character_counts_per_position_in(positions);

        let scores: Vec<_> = words
            .iter()
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
impl PickBestWord for MostFrequentCharacterPerPosHighVarietyWord {
    fn pick(&self, game: &Wordle) -> Option<Word> {
        let positions = &game.open_positions();
        let mut words = &game.high_variety_words(positions);
        if words.is_empty() {
            words = &game.solutions;
        }
        let counts = words.as_slice().character_counts_per_position_in(positions);
        // for (i, count) in counts.iter().enumerate() {
        //     println!("Position[{}] character counts: {}", i, count.to_string());
        // }

        let scores: Vec<_> = words
            .iter()
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
impl PickBestWord for MatchingMostOtherWordsInAtLeastOneOpenPosition {
    fn pick(&self, game: &Wordle) -> Option<Word> {
        let solutions = &game.solutions;
        let positions = &game.open_positions();
        // for each word, find out how many other words it matches in any open position
        let mut scores: Vec<_> = solutions.iter().map(|word| (word, 0_usize)).collect();
        let open_chars: Vec<_> = solutions
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
impl PickBestWord for MatchingMostOtherWordsInAtLeastOneOpenPositionHighVarietyWord {
    fn pick(&self, game: &Wordle) -> Option<Word> {
        let positions = &game.open_positions();
        let mut solutions = &game.high_variety_words(positions);
        if solutions.is_empty() {
            solutions = &game.solutions;
        }
        // for each word, find out how many other words it matches in any open position
        let mut scores: Vec<_> = solutions.iter().map(|word| (word, 0_usize)).collect();
        let open_chars: Vec<_> = solutions
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
/// This function returns a hints for the given guess and solution
#[allow(clippy::ptr_arg)] // Because trait WordAsCharVec is implemented for Word not &[char]
fn determine_hint(guess: &Guess, solution: &Solution) -> Hints {
    // Initialize as every position incorrect
    let mut hint = Hints::default();

    // Fill in exact matches
    let mut open_positions = vec![];
    for i in 0..5 {
        if guess[i] == solution[i] {
            hint.set_correct(i);
        } else {
            open_positions.push(i);
        }
    }

    // For characters at another position, consider only characters not previously matched
    // For example:
    // Guessing "geese" for solution "eject"  matches exactly in the middle 'e', which leaves
    // "ge_se" and "ej_ct". The 'e' at pos 1 of "geese" will count as a present char, but the
    // last 'e' in "geese" is illegal, because all the 'e's in "ej_ct" were already matched.
    for &i in &open_positions {
        let considered_char_count = |word: &[char], ch: &char| {
            word.iter()
                .take(i + 1) // include current pos
                .enumerate()
                .filter(|(i, c)| c == &ch && open_positions.contains(i))
                .count()
        };
        let char = &guess[i];
        // println!(
        //     "considered_char_count('{}', {}) = {} | '{}'.total_char_count({:?}, {}) = {}",
        //     guess.to_string(),
        //     char,
        //     considered_char_count(guess, char),
        //     solution.to_string(),
        //     open_positions,
        //     char,
        //     solution.total_char_count(&open_positions, char)
        // );
        if considered_char_count(guess, char) <= solution.total_char_count(&open_positions, char) {
            hint.set_wrong_pos(i);
        }
    }
    hint
}

fn main() {
    // print_word_combinations();
    Wordle::new(&SOLUTIONS).play()
}

// TODO find 5 good starting words
fn print_word_combinations() {
    // let words: Vec<Word> = ["tubes", "fling", "champ", "wordy"]
    // let words: Vec<Word> = ["quick", "brown", "foxed", "jumps", "glazy", "vetch"]
    let words: Vec<Word> = ["soare", "until"]
        .into_iter()
        .map(|w| w.to_word())
        .collect();

    let used_chars: HashSet<char> = words.iter().flatten().cloned().collect();
    let words_by_new_unused_chars = words_with_most_unused_chars(&used_chars);
    for (chars, words) in &words_by_new_unused_chars {
        println!("{}: {:?}", chars.to_string(), words);
    }
    println!();
    for (chars, _) in words_by_new_unused_chars {
        println!(" {}:", chars.to_string());
        let mut used_chars = used_chars.clone();
        chars.into_iter().for_each(|c| {
            used_chars.insert(c);
        });
        let words_by_new_unused_chars = words_with_most_unused_chars(&used_chars);
        for (chars, words) in &words_by_new_unused_chars {
            println!("  {}: {:?}", chars.to_string(), words);

            for (chars, _) in &words_by_new_unused_chars {
                // println!("   {}:", chars.to_string());
                let mut used_chars = used_chars.clone();
                chars.iter().for_each(|c| {
                    used_chars.insert(*c);
                });
                let words_by_new_unused_chars = words_with_most_unused_chars(&used_chars);
                for (chars, words) in &words_by_new_unused_chars {
                    if words.len() > 10 {
                        continue;
                    }
                    println!("   {}: {:?}", chars.to_string(), words);
                }
            }
        }
        // break;
    }
}

fn words_with_most_unused_chars(used_chars: &HashSet<char>) -> Vec<(Vec<char>, Vec<String>)> {
    let best_words = words_with_most_new_chars(used_chars, &GUESSES);
    let mut words_by_new_unused_chars: HashMap<Vec<char>, Vec<String>> = HashMap::new();
    for (new_chars, word) in best_words {
        words_by_new_unused_chars
            .entry(new_chars)
            .or_default()
            .push(word.to_string());
    }
    let mut words_by_new_unused_chars: Vec<(Vec<char>, Vec<String>)> =
        words_by_new_unused_chars.into_iter().collect();
    words_by_new_unused_chars.sort_unstable();
    words_by_new_unused_chars
}

fn words_with_most_new_chars<'w>(
    used_chars: &HashSet<char>,
    words: &'w [Word],
) -> Vec<(Vec<char>, &'w Word)> {
    let new: Vec<(Vec<char>, &Word)> = words
        .iter()
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
    words: &'w [Word],
) -> Vec<(&'w Word, usize)> {
    let mut scores: Vec<_> = words
        .iter()
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

    #[ignore] // ~30s for all guesses, ~8s for solutions only
    #[test]
    fn compare_best_word_strategies() {
        let guesses = &GUESSES;
        let solutions = &SOLUTIONS;

        // Solutions only:
        // 484.89 roate, 490.38 raise, 493.53 raile, 502.77 soare, 516.34 arise,
        // 516.85 irate, 517.91 orate, 531.22 ariel, 538.21 arose, 548.07 raine
        // Guesses:
        // 12563.92 lares, 12743.70 rales, 13298.11 tares, 13369.60 soare, 13419.26 reais
        // 13461.31 nares, 13684.70 aeros, 13771.28 rates, 13951.64 arles, 13972.96 serai
        let variances = variance_of_remaining_words(guesses, solutions);
        println!("Best (lowest) variance:\n{}", variances.to_string(10));
        assert_eq!(variances.len(), guesses.len());

        // panic!(); // ~2s

        // Solutions only:
        // 60.42 roate, 61.00 raise, 61.33 raile, 62.30 soare, 63.73 arise,
        // 63.78 irate, 63.89 orate, 65.29 ariel, 66.02 arose, 67.06 raine
        // Guesses:
        // 288.74 lares, 292.11 rales, 302.49 tares, 303.83 soare, 304.76 reais
        // 305.55 nares, 309.73 aeros, 311.36 rates, 314.73 arles, 315.13 serai
        let remaining = expected_remaining_solution_counts(guesses, solutions);
        println!(
            "Best (lowest) expected remaining solution count:\n{}",
            remaining.to_string(10)
        );
        assert_eq!(remaining.len(), guesses.len());

        // panic!(); // ~5s

        // Solutions only:
        //
        // Guesses:
        // Best (lowest) average remaining solution count:
        // 3745512 lares, 3789200 rales, 3923922 tares, 3941294 soare, 3953360 reais
        // 3963578 nares, 4017862 aeros, 4038902 rates, 4082728 arles, 4087910 serai
        let totals = lowest_total_number_of_remaining_solutions(solutions, guesses);
        println!(
            "Best (lowest) average remaining solution count:\n{}",
            totals.to_string(10)
        );
        assert_eq!(totals.len(), guesses.len());

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

    // Previously used method, slightly inferior to lowest_total_number_of_remaining_solutions
    fn variance_of_remaining_words<'g>(
        guesses: &'g [Guess],
        solutions: &[Word],
    ) -> Vec<(&'g Guess, f64)> {
        let average = solutions.len() as f64 / 243.0;
        let mut scores: Vec<_> = guesses
            .par_iter()
            .map(|guess| {
                let buckets = hint_buckets(guess, solutions);
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

    // Previously used method, slightly inferior to lowest_total_number_of_remaining_solutions
    fn expected_remaining_solution_counts<'g>(
        guesses: &'g [Guess],
        solutions: &[Word],
    ) -> Vec<(&'g Guess, f64)> {
        let total_solutions = solutions.len() as f64;
        let mut scores: Vec<_> = guesses
            .par_iter()
            .map(|guess| {
                let buckets = hint_buckets(guess, solutions);
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

    // Allow because determine_hint expects &Guess not &[char]
    fn hint_buckets(
        #[allow(clippy::ptr_arg)] guess: &Guess,
        solutions: &[Word],
    ) -> [Vec<usize>; 243] {
        const EMPTY_VEC: Vec<usize> = Vec::new();
        let mut hint_buckets = [EMPTY_VEC; 243];
        for (i, solution) in solutions.iter().enumerate() {
            let hint = determine_hint(guess, solution);
            hint_buckets[hint.value() as usize].push(i);
        }
        hint_buckets
    }

    #[ignore] // 2-4s
    #[test]
    fn count_solutions_by_hint_by_guess() {
        assert_eq!(
            SOLUTIONS_BY_HINT_BY_GUESS
                .iter()
                .map(|(_, s)| s.len())
                .sum::<usize>(),
            1_120_540 // 1120540
        );
    }

    #[ignore]
    #[test]
    fn count_solutions_by_secret_by_guess() {
        assert_eq!(
            SOLUTIONS_BY_SECRET_BY_GUESS
                .iter()
                .map(|(_, s)| s.len())
                .sum::<usize>(),
            SOLUTIONS.len() * GUESSES.len() // 2_315 * 12_972 = 30_030_180 = 2315 * 12972 = 30030180
        );
    }

    #[ignore] // ~12s
    #[test]
    // Best 10 guesses (with lowest average number of remaining possible solutions):
    // 60.42 roate, 61.00 raise, 61.33 raile, 62.30 soare, 63.73 arise,
    // 63.78 irate, 63.89 orate, 65.29 ariel, 66.02 arose, 67.06 raine
    fn find_optimal_first_word() {
        let scores = lowest_total_number_of_remaining_solutions(&SOLUTIONS, &GUESSES);
        let optimal = scores.lowest().unwrap();
        assert_eq!("roate".to_word(), optimal);
    }

    #[ignore] // ~25s
    // English: "roate -> "linds"
    // Deutsch: "tarne" -> "helis"
    #[test]
    // English
    // Second word after 'roate' with lowest total remaining words in round 3:
    // 11847 linds, 11947 sling, 12033 clips, 12237 limns, 12337 blins,
    // 12667 slink, 12753 sclim, 12951 lings, 12977 lysin, 13021 cling
    //
    // Deutsch: 1. Wort "tarne, 2. Wort:
    // 7273 helis, 7451 heils, 7641 holis, 7925 selig, 8073 kilos,
    // 8221 gusli, 8281 bilds, 8313 beils, 8315 keils, 8381 solid
    //
    // Best 5-combo in 84s:
    // Best 5. guesses after 1. 'roate' and 2. 'linds' and 3. 'chump' and 4. 'gawky':
    // 2397 befit, 2405 feebs, 2405 brief, 2409 fubar, 2409 beefy
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
        let guesses = &GUESSES;
        let solutions = &SOLUTIONS;

        let mut scores = lowest_total_number_of_remaining_solutions(solutions, guesses);
        scores.sort_asc();

        let number_of_top_pickes_to_try = 1;
        for (guess1, _) in scores.into_iter().rev().take(number_of_top_pickes_to_try) {
            let guessed = [guess1];
            let scores = find_best_next_guesses(guesses, solutions, &guessed);

            println!(
                "Best 2. guesses after 1. '{}': {}",
                guess1.to_string(),
                scores.to_string(5)
            );

            for (guess2, _) in scores.into_iter().rev().take(number_of_top_pickes_to_try) {
                let guessed = [guess1, guess2];
                let scores = find_best_next_guesses(guesses, solutions, &guessed);
                println!(
                    "Best 3. guesses after 1. '{}' and 2. '{}': {}",
                    guess1.to_string(),
                    guess2.to_string(),
                    scores.to_string(5)
                );

                for (guess3, _) in scores.into_iter().rev().take(number_of_top_pickes_to_try) {
                    let guessed = [guess1, guess2, guess3];
                    let scores = find_best_next_guesses(guesses, solutions, &guessed);
                    println!(
                        "Best 4. guesses after 1. '{}' and 2. '{}' and 3. '{}': {}",
                        guess1.to_string(),
                        guess2.to_string(),
                        guess3.to_string(),
                        scores.to_string(5)
                    );

                    for (guess4, _) in scores.into_iter().rev().take(number_of_top_pickes_to_try) {
                        let guessed = [guess1, guess2, guess3, guess4];
                        let scores = find_best_next_guesses(guesses, solutions, &guessed);
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
            // let optimal_next_guess = scores.lowest().unwrap();
            // println!("Best 2nd guess: '{}'", optimal_next_guess.to_string());
        }
    }

    fn find_best_next_guesses<'g>(
        guesses: &'g [Guess],
        solutions: &[Solution],
        guessed: &[&Guess],
    ) -> Vec<(&'g Guess, usize)> {
        let first = *guessed.iter().next().unwrap();
        // false SOLUTIONS_BY_HINT_BY_GUESS 25s/25s/25s
        // false SOLUTIONS_BY_SECRET_BY_GUESS 19s/22s/22s/23s
        let use_solutions_by_secret_by_guess = true;
        // Access to initialize this lazy_static here to avoid doing so in parallel section below
        assert!(SOLUTIONS_BY_SECRET_BY_GUESS.len() > 0);

        let mut scores: Vec<_> = guesses
            .par_iter()
            .filter(|next| !guessed.contains(next))
            .map(|next| {
                let count: usize = solutions
                    .iter()
                    .map(|secret| {
                        let (solutions1, solutions2) = if use_solutions_by_secret_by_guess {
                            let solutions1 = SOLUTIONS_BY_SECRET_BY_GUESS[first][secret];
                            let solutions2 = SOLUTIONS_BY_SECRET_BY_GUESS[next][secret];
                            (solutions1, solutions2)
                        } else {
                            let hint1 = determine_hint(first, secret);
                            let solutions1 = &SOLUTIONS_BY_HINT_BY_GUESS[first][&hint1.value()];
                            let hint2 = determine_hint(next, secret);
                            let solutions2 = &SOLUTIONS_BY_HINT_BY_GUESS[next][&hint2.value()];
                            (solutions1, solutions2)
                        };
                        // apply first and next guess
                        let mut solutions: HashSet<_> =
                            solutions1.intersection(solutions2).cloned().collect();

                        // Apply other previous guesses
                        for other in guessed.iter().skip(1).cloned() {
                            let solutions3 = SOLUTIONS_BY_SECRET_BY_GUESS[other][secret];
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

    #[ignore]
    #[test]
    fn hint_by_solution_by_guess() {
        // -3s with --release
        let hint_by_solution_by_guess: HashMap<&'static Guess, HashMap<&'static Solution, Hints>> =
            GUESSES
                .par_iter()
                .map(|guess| {
                    let hints_by_solution = SOLUTIONS
                        .iter()
                        .map(|solution| (solution, determine_hint(guess, solution)))
                        .collect::<HashMap<&Solution, Hints>>();
                    (guess, hints_by_solution)
                })
                .collect();
        println!(
            "hint_by_solution_by_guess.len() = {}",
            hint_by_solution_by_guess.len()
        );
        // let mut rng = thread_rng();
        // let (guess, hint_by_solution) = hint_by_solution_by_guess.iter().choose(&mut rng).unwrap();
        // for (sol, hint) in hint_by_solution.iter() {
        //     println!("{} + {} = {}", guess.to_string(), sol.to_string(), hint);
        // }
    }

    #[ignore]
    #[test]
    //
    fn auto_play_word_that_results_in_fewest_remaining_solutions() {
        autoplay_and_print_stats(WordThatResultsInFewestRemainingSolutions);
    }

    #[ignore] // ~4s
    #[test]
    // Average attempts = 3.234; 0 (0.000%) failed games (> 6 attempts):
    // 1: 29, 2: 351, 3: 1071, 4: 782, 5: 78, 6: 4
    fn auto_play_tubes_fling_champ_wordy_every_time() {
        let strategy = FixedGuessList::new(vec!["tubes", "fling", "champ", "wordy"]);
        autoplay_and_print_stats(strategy);
    }

    #[ignore]
    #[test]
    // Average attempts = 3.067; 2 (0.086%) failed games (> 6 attempts):
    // 1: 23, 2: 521, 3: 1162, 4: 509, 5: 88, 6: 10, 7: 2
    fn auto_play_roate_linds_chump_gawky_befit() {
        let strategy = FixedGuessList::new(vec!["roate", "linds", "chump", "gawky", "befit"]);
        autoplay_and_print_stats(strategy);
    }

    #[ignore] // ~4s
    #[test]
    // Average attempts = 3.060; 0 (0.000%) failed games (> 6 attempts):
    // 1: 23, 2: 515, 3: 1179, 4: 498, 5: 97, 6: 3
    fn auto_play_roate_linds_chump_gawky() {
        let strategy = FixedGuessList::new(vec!["roate", "linds", "chump", "gawky"]);
        autoplay_and_print_stats(strategy);
    }

    #[ignore] // ~4s
    #[test]
    // 16s normally, 2s parallel
    // Without "whack":
    // Average attempts = 3.349; 9 (0.389%) failed games (> 6 attempts):
    // 1: 22, 2: 481, 3: 714, 4: 907, 5: 156, 6: 26, 7: 8, 8: 1
    // With "whack":
    // Average attempts = 3.479; 4 (0.173%) failed games (> 6 attempts):
    // 1: 22, 2: 481, 3: 714, 4: 610, 5: 444, 6: 40, 7: 4
    fn auto_play_soare_until_pygmy_whack_every_time() {
        let strategy = FixedGuessList::new(vec!["soare", "until", "pygmy", "whack"]);
        autoplay_and_print_stats(strategy);
    }

    #[ignore]
    #[test]
    // 21s normally, 3s parallel
    // Average attempts = 3.349; 9 (0.389%) failed games (> 6 attempts):
    // 1: 22, 2: 481, 3: 714, 4: 907, 5: 156, 6: 26, 7: 8, 8: 1
    fn auto_play_quick_brown_foxed_jumps_glazy_vetch_every_time() {
        let strategy =
            FixedGuessList::new(vec!["quick", "brown", "foxed", "jumps", "glazy", "vetch"]);
        autoplay_and_print_stats(strategy);
    }

    #[ignore]
    #[test]
    // Average attempts = 3.254; 0 (0.000%) failed games (> 6 attempts):
    // 1: 23, 2: 360, 3: 1083, 4: 709, 5: 136, 6: 4
    fn auto_play_fixed_guess_list_1() {
        let strategy = FixedGuessList::new(vec!["brake", "dying", "clots", "whump"]);
        autoplay_and_print_stats(strategy);
    }

    #[ignore]
    #[test]
    fn auto_play_fixed_guess_list_2() {
        // Average attempts = 3.170; 0 (0.000%) failed games (> 6 attempts):
        // 1: 29, 2: 431, 3: 1093, 4: 643, 5: 117, 6: 2
        let strategy = FixedGuessList::new(vec!["maple", "sight", "frown", "ducky"]);
        autoplay_and_print_stats(strategy);
    }

    #[ignore]
    #[test]
    // Average attempts = 3.253; 0 (0.000%) failed games (> 6 attempts):
    // 1: 24, 2: 387, 3: 1053, 4: 692, 5: 149, 6: 10
    fn auto_play_fixed_guess_list_3() {
        let strategy = FixedGuessList::new(vec!["fiend", "paths", "crumb", "glows"]);
        autoplay_and_print_stats(strategy);
    }

    #[ignore]
    #[test]
    // Average attempts = 3.083; 8 (0.346%) failed games (> 6 attempts):
    // 1: 21, 2: 559, 3: 1088, 4: 528, 5: 101, 6: 10, 7: 8
    fn auto_play_fixed_guess_list_4() {
        let strategy = FixedGuessList::new(vec!["reals", "point", "ducky"]);
        autoplay_and_print_stats(strategy);
    }

    #[ignore]
    #[test]
    // Average attempts = 3.097; 3 (0.130%) failed games (> 6 attempts):
    // 1: 27, 2: 492, 3: 1151, 4: 538, 5: 94, 6: 10, 7: 1, 8: 2
    fn auto_play_fixed_guess_list_5() {
        let strategy = FixedGuessList::new(vec!["laser", "pitch", "mound"]);
        autoplay_and_print_stats(strategy);
    }

    #[ignore]
    #[test]
    // 62s normally, 6s parallel
    // Average attempts 3.067; 9 (0.389%) failed games (> 6 attempts):
    // 1: 34, 2: 537, 3: 1145, 4: 467, 5: 114, 6: 9, 7: 8, 8: 1
    fn auto_play_most_frequent_global_characters() {
        autoplay_and_print_stats(MostFrequentGlobalCharacter);
    }

    #[ignore]
    #[test]
    // 60s normally, 9s parallel
    // Average attempts = 3.073; 12 (0.518%) failed games (> 6 attempts):
    // 1: 14, 2: 598, 3: 1121, 4: 423, 5: 120, 6: 27, 7: 9, 8: 2, 9: 1
    fn auto_play_most_frequent_global_characters_high_variety_word() {
        autoplay_and_print_stats(MostFrequentGlobalCharacterHighVarietyWord);
    }

    #[ignore]
    #[test]
    // 32s normally, 4s parallel
    // Average attempts 3.098; 7 (0.302%) failed games (> 6 attempts):
    // 1: 29, 2: 522, 3: 1133, 4: 498, 5: 98, 6: 28, 7: 6, 8: 1
    fn auto_play_most_frequent_characters_per_pos() {
        autoplay_and_print_stats(MostFrequentCharacterPerPos);
    }

    #[ignore]
    #[test]
    // 45s normally, 7s parallel
    // Average attempts = 2.998; 6 (0.259%) failed games (> 6 attempts):
    // 1: 29, 2: 599, 3: 1183, 4: 390, 5: 86, 6: 22, 7: 5, 8: 1
    fn auto_play_most_frequent_characters_per_pos_high_variety_word() {
        autoplay_and_print_stats(MostFrequentCharacterPerPosHighVarietyWord);
    }

    #[ignore]
    #[test]
    // 107s normally, 20s parallel
    // Average attempts = 3.223; 12 (0.518%) failed games (> 6 attempts):
    // 1: 14, 2: 394, 3: 1194, 4: 542, 5: 129, 6: 30, 7: 11, 8: 1
    fn auto_play_most_frequent_characters_of_words() {
        autoplay_and_print_stats(MostFrequentCharactersOfRemainingWords);
    }

    #[ignore]
    #[test]
    // Estimated ~10min normally, 59s parallel
    // Average attempts = 3.123; 0 (0.000%) failed games (> 6 attempts):
    // 1: 22, 2: 510, 3: 1100, 4: 537, 5: 137, 6: 9
    fn auto_play_most_frequent_unused_characters() {
        autoplay_and_print_stats(MostFrequentUnusedCharacters);
    }

    #[ignore]
    #[test]
    // 48min29s normally, 4min45s parallel
    // Average attempts = 3.357; 16 (0.691%) failed games (> 6 attempts):
    // 1: 27, 2: 397, 3: 957, 4: 672, 5: 201, 6: 45, 7: 11, 8: 5
    fn auto_play_most_other_words_in_at_least_one_open_position() {
        autoplay_and_print_stats(MatchingMostOtherWordsInAtLeastOneOpenPosition);
    }

    #[ignore]
    #[test]
    // 17min51s normally, 136s parallel
    // Average attempts = 3.144; 12 (0.518%) failed games (> 6 attempts):
    // 1: 20, 2: 540, 3: 1072, 4: 512, 5: 124, 6: 35, 7: 12
    fn auto_play_most_other_words_in_at_least_one_open_position_high_variety_word() {
        autoplay_and_print_stats(MatchingMostOtherWordsInAtLeastOneOpenPositionHighVarietyWord);
    }

    fn autoplay_and_print_stats<S: PickBestWord + Sync>(strategy: S) {
        let attempts: Vec<usize> = SOLUTIONS
            .iter()
            .map(|secret| Wordle::new(&SOLUTIONS).autoplay(secret, &strategy))
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
        let game = Wordle::new(&SOLUTIONS);
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
        let game = Wordle::new(&GUESSES);
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
        let game = Wordle::new(&SOLUTIONS);
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
        let game = Wordle::new(&SOLUTIONS);
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
    fn test_determine_hints() {
        let hint = determine_hint(&"guest".to_word(), &"truss".to_word());
        assert_eq!("⬛🟨⬛🟩🟨", hint.to_string());

        let hint = determine_hint(&"briar".to_word(), &"error".to_word());
        assert_eq!("⬛🟩⬛⬛🟩", hint.to_string());

        let hint = determine_hint(&"sissy".to_word(), &"truss".to_word());
        assert_eq!("🟨⬛⬛🟩⬛", hint.to_string());

        let hint = determine_hint(&"eject".to_word(), &"geese".to_word());
        assert_eq!("🟨⬛🟩⬛⬛", hint.to_string());

        let hint = determine_hint(&"three".to_word(), &"beret".to_word());
        assert_eq!("🟨⬛🟩🟩🟨", hint.to_string());
    }
}
