use rand::prelude::*;
use rayon::prelude::*;
use std::cmp::{Ordering, Reverse};
use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter};
use std::io;
use Hint::*;

#[macro_use]
extern crate lazy_static;

const COMBINED: &str = include_str!("../data/wordlists/combined.txt");
const SOLUTIONS: &str = include_str!("../data/wordlists/solutions.txt");
const ALL_POS: [usize; 5] = [0, 1, 2, 3, 4];

lazy_static! {
    static ref COMBINED_WORDS: Vec<Word> = COMBINED.lines().map(|w| w.to_word()).collect();
    static ref COMBINED_GLOBAL_CHAR_COUNTS_BY_CHAR: HashMap<char, usize> = COMBINED
        .lines()
        .map(|w| w.to_word())
        .collect::<Vec<Word>>()
        .as_slice()
        .global_character_counts_in(&ALL_POS);
    static ref COMBINED_GLOBAL_CHAR_COUNT_SUMS_BY_WORD: HashMap<Word, usize> = COMBINED_WORDS
        .iter()
        .map(|word| {
            let count = word
                .unique_chars_in(&ALL_POS)
                .iter()
                .map(|c| COMBINED_GLOBAL_CHAR_COUNTS_BY_CHAR[c])
                .sum::<usize>();
            (word.to_vec(), count)
        })
        .collect();
}

type Word = Vec<char>;

// Helper for https://www.powerlanguage.co.uk/wordle/
#[derive(Debug)]
struct Wordle {
    solutions: Vec<Word>,
    words: Vec<Word>,
    illegal_chars: HashSet<char>,
    correct_chars: [Option<char>; 5],
    illegal_at_pos: [HashSet<char>; 5],
    mandatory_chars: HashSet<char>,
    guessed_words: HashSet<Word>,
    rng: ThreadRng,
}
impl Wordle {
    fn new(wordlist: &str) -> Self {
        let solutions: Vec<Word> = wordlist.lines().map(|word| word.to_word()).collect();
        let words = solutions.clone();
        let empty = HashSet::new;
        let illegal_position_for_char: [HashSet<char>; 5] =
            [empty(), empty(), empty(), empty(), empty()];
        let rng = thread_rng();
        Wordle {
            solutions,
            words,
            illegal_chars: HashSet::new(),
            correct_chars: [None; 5],
            illegal_at_pos: illegal_position_for_char,
            mandatory_chars: HashSet::new(),
            guessed_words: HashSet::new(),
            rng,
        }
    }
    #[cfg(test)]
    fn autoplay<S: PickBestWord>(&mut self, wanted: Word, strategy: S) -> usize {
        let mut attempts = 0;
        while !self.is_game_over() && attempts < 10 {
            let guess = strategy.pick(self).unwrap_or_else(|| {
                MostFrequentGlobalCharacter
                    .pick(self)
                    .unwrap_or_else(|| self.random_suggestion())
            });
            attempts += 1;
            let (hint, _) = get_hints(&guess, &wanted);
            println!(
                "{:4} solutions left, {}. guess '{}', hint '{}', wanted '{}'",
                self.solutions.len(),
                attempts,
                guess.to_string(),
                hint.to_string(),
                wanted.to_string(),
            );
            for (i, feedback) in hint.hints.iter().enumerate() {
                let c = guess[i].to_ascii_lowercase();
                match feedback {
                    Illegal => {}
                    WrongPos => {
                        // println!("Inserting '{}' as illegal @ {}", c, i);
                        self.illegal_at_pos[i].insert(c);
                        self.mandatory_chars.insert(c);
                    }
                    Correct => {
                        // println!("Inserting '{}' as correct character @ {}", c, i);
                        self.correct_chars[i] = Some(c);
                        self.mandatory_chars.insert(c);
                    }
                }
            }
            self.guessed_words.insert(guess.clone());
            if self.is_game_over() {
                break;
            }
            let open_positions = self.open_positions();
            for (_, c) in guess
                .into_iter()
                .enumerate()
                .filter(|(_, c)| !self.mandatory_chars.contains(c))
                .filter(|&(i, c)| self.correct_chars[i] != Some(c))
            {
                if !self.correct_chars.iter().any(|&o| o == Some(c)) {
                    // println!("Inserting globally illegal char '{}'", c);
                    self.illegal_chars.insert(c);
                } else {
                    // println!("Inserting '{}' as illegal @ {:?}", c, open_positions);
                    for i in &open_positions {
                        self.illegal_at_pos[*i].insert(c);
                    }
                }
            }
            self.update_words();
        }
        if self.solutions.len() == 1 {
            println!(
                "After {} guesses: The only word left in the list is '{}'\n",
                attempts,
                self.solutions[0].to_string()
            );
        } else {
            let word: String = self.correct_chars.iter().map(|c| c.unwrap()).collect();
            println!("After {} guesses: The word is '{}'\n", attempts, word);
        }
        attempts
    }
    fn play(&mut self) {
        while !self.is_game_over() {
            self.print_remaining_word_count();
            self.single_round();
        }
        self.print_result();
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
        let guess = self.ask_for_guess();
        self.guessed_words.insert(guess.clone());
        self.ask_for_feedback(&guess);
        if self.is_game_over() {
            return;
        }
        self.update_illegal_chars(guess);
        self.update_words();
    }

    fn ask_for_guess(&mut self) -> Word {
        let suggestion = self.suggest_a_word();
        let mut input;
        println!(
            "Enter the word you guessed, or use suggestion '{}':",
            suggestion.to_string()
        );
        loop {
            input = String::new();
            io::stdin().read_line(&mut input).unwrap();
            if !input.trim().is_empty()
                && input
                    .trim()
                    .chars()
                    .filter(|c| c.is_ascii_alphabetic())
                    .count()
                    != 5
            {
                println!("Please enter 5 alphabetic characters, or nothing to use the suggestion:")
            } else {
                break;
            }
        }
        if !input.trim().is_empty() {
            input
                .trim()
                .chars()
                .map(|c| c.to_ascii_lowercase())
                .collect()
        } else {
            suggestion
        }
    }

    fn suggest_a_word(&mut self) -> Word {
        self.optimized_suggestion()
            .unwrap_or_else(|| self.random_suggestion())
    }

    fn optimized_suggestion(&mut self) -> Option<Word> {
        // Called just to print the top words
        MostFrequentGlobalCharacter.pick(self);
        MostFrequentCharacterPerPos.pick(self);
        MatchingMostOtherWordsInAtLeastOneOpenPosition.pick(self);

        // Called just to print the top high variety words
        MostFrequentGlobalCharacterHighVarietyWord.pick(self);
        MostFrequentCharacterPerPosHighVarietyWord.pick(self);
        MatchingMostOtherWordsInAtLeastOneOpenPositionHighVarietyWord.pick(self);

        // MostFrequentRemainingUnusedCharacters.pick(self)
        MostFrequentUnusedCharacters.pick(self)

        // TubesFlingChampWordyEveryTime.pick(self)

        // Slow
        // WordThatResultsInFewestPossibleRemainingWords.pick(self)
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

    fn ask_for_feedback(&mut self, guess: &[char]) {
        println!(
            "Enter feedback using upper-case for correct and lower-case for wrong positions,\n\
            or any non-alphabetic for illegal:"
        );
        let mut input = "123456".to_string();
        while input.trim().chars().count() > 5 {
            input = String::new();
            io::stdin().read_line(&mut input).unwrap();
            if input.trim().chars().count() > 5 {
                println!("Enter at most 5 characters:")
            }
        }

        let feedback: Word = input.trim().to_word();
        for (i, feedback) in feedback
            .iter()
            .enumerate()
            .filter(|(_, c)| c.is_ascii_alphabetic())
        {
            let c = guess[i];
            if !guess.contains(&feedback.to_ascii_lowercase()) {
                panic!("Char '{}' at pos {} is not part of the guess", feedback, i,);
            } else if feedback.is_ascii_uppercase() {
                println!("Inserting '{}' as correct character @ {}", c, i);
                self.correct_chars[i] = Some(c.to_ascii_lowercase());
            } else {
                println!("Inserting '{}' as illegal @ {}", c, i);
                self.illegal_at_pos[i].insert(c);
                self.mandatory_chars.insert(c);
            }
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
                println!("Inserting globally illegal char '{}'", c);
                self.illegal_chars.insert(c);
            } else {
                println!("Inserting '{}' as illegal @ {:?}", c, open_positions);
                for i in &open_positions {
                    self.illegal_at_pos[*i].insert(c);
                }
            }
        }
    }

    fn update_words(&mut self) {
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
            .filter(|word| !self.guessed_words.contains(word))
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
        positions.iter().map(|&i| self[i]).collect()
    }
    fn chars_in(&self, positions: &[usize]) -> Vec<char> {
        positions.iter().map(|&i| self[i]).collect()
    }
    fn char_position_set(&self, positions: &[usize]) -> HashSet<(usize, char)> {
        positions.iter().map(|&i| (i, self[i])).collect()
    }
    fn total_char_count(&self, positions: &[usize], ch: &char) -> usize {
        positions.iter().filter(|i| &self[**i] == ch).count()
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
    fn to_string(&self) -> String;
    fn lowest(&self) -> Option<Word>;
    fn highest(&self) -> Option<Word>;
}
impl<T: PartialOrd + Display> ScoreTrait for Vec<(T, &Word)> {
    fn sort_asc(&mut self) {
        self.sort_unstable_by(|(a_value, a_word), (b_value, b_word)| {
            match b_value.partial_cmp(a_value) {
                Some(Ordering::Equal) | None => a_word.cmp(b_word),
                Some(by_value) => by_value,
            }
        });
    }
    fn sort_desc(&mut self) {
        self.sort_unstable_by(|(a_value, a_word), (b_value, b_word)| {
            match a_value.partial_cmp(b_value) {
                Some(Ordering::Equal) | None => a_word.cmp(b_word),
                Some(by_value) => by_value,
            }
        });
    }
    fn to_string(&self) -> String {
        self.iter()
            .skip(self.len().saturating_sub(100))
            .map(|(count, word)| format!("{} {}", count, word.to_string()))
            .collect::<Vec<_>>()
            .join(", ")
    }
    fn lowest(&self) -> Option<Word> {
        self.iter()
            .min_by(
                |(a_value, a_word), (b_value, b_word)| match a_value.partial_cmp(b_value) {
                    Some(Ordering::Equal) | None => a_word.cmp(b_word),
                    Some(by_value) => by_value,
                },
            )
            .map(|(_value, word)| word.to_vec())
    }
    fn highest(&self) -> Option<Word> {
        self.iter()
            .max_by(
                |(a_value, a_word), (b_value, b_word)| match a_value.partial_cmp(b_value) {
                    Some(Ordering::Equal) | None => a_word.cmp(b_word),
                    Some(by_value) => by_value,
                },
            )
            .map(|(_value, word)| word.to_vec())
    }
}

trait PickBestWord {
    fn pick(&self, game: &Wordle) -> Option<Word>;
}

struct WordThatResultsInFewestPossibleRemainingWords;
impl PickBestWord for WordThatResultsInFewestPossibleRemainingWords {
    fn pick(&self, game: &Wordle) -> Option<Word> {
        let words = &game.solutions;
        let all_buckets = try_out_all_words_with_each_other(words, &ALL_POS);
        let word_count = words.len() as f64;
        let mut scores: Vec<(f64, &Word)> = all_buckets
            .iter()
            .enumerate()
            .map(|(i, buckets)| {
                let expected_remaining_word_count = buckets
                    .iter()
                    .map(|&bucket_size| {
                        let probability_of_bucket = bucket_size as f64 / word_count;
                        probability_of_bucket * bucket_size as f64
                    })
                    .sum();
                (expected_remaining_word_count, &words[i])
            })
            .collect();

        scores.sort_desc();
        println!("Remaining worst: {}", scores.to_string());

        scores.sort_asc();
        println!("Remaining best:  {}", scores.to_string());

        scores.lowest()
    }
}

struct WordDividingSearchSpaceMostEvenly;
impl PickBestWord for WordDividingSearchSpaceMostEvenly {
    fn pick(&self, game: &Wordle) -> Option<Word> {
        let words = &game.solutions;
        let all_buckets = try_out_all_words_with_each_other(words, &ALL_POS);
        let averages: Vec<_> = all_buckets
            .iter()
            .map(|bucket| bucket.iter().sum::<usize>() as f64 / bucket.len() as f64)
            .collect();
        let variances: Vec<_> = all_buckets
            .iter()
            .enumerate()
            .map(|(i, bucket)| {
                bucket
                    .iter()
                    .map(|&v| ((v as f64) - averages[i]).powf(2.0))
                    .sum::<f64>() as f64
                    / bucket.len() as f64
            })
            .collect();
        let mut scores: Vec<(f64, &Word)> = words
            .iter()
            .enumerate()
            .map(|(i, word)| (variances[i], word))
            .collect();

        scores.sort_desc();
        println!("Worst/highest variance: {}", scores.to_string());
        scores.sort_asc();
        println!("Best/lowest variance:   {}", scores.to_string());

        scores.lowest()
    }
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
                (count, word)
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
                (count, word)
            })
            .collect();
        if scores.is_empty() {
            return None;
        }
        scores.highest()
    }
}

struct TubesFlingChampWordyEveryTime;
impl PickBestWord for TubesFlingChampWordyEveryTime {
    fn pick(&self, game: &Wordle) -> Option<Word> {
        ["tubes", "fling", "champ", "wordy"]
            .get(game.guessed_words.len())
            .map(|w| w.to_word())
    }
}

struct QuickBrownFoxedJumpsGlazyVetchEveryTime;
impl PickBestWord for QuickBrownFoxedJumpsGlazyVetchEveryTime {
    fn pick(&self, game: &Wordle) -> Option<Word> {
        ["quick", "brown", "foxed", "jumps", "glazy", "vetch"]
            .get(game.guessed_words.len())
            .map(|w| w.to_word())
    }
}

struct SoareUntilPygmyWhackEveryTime;
impl PickBestWord for SoareUntilPygmyWhackEveryTime {
    fn pick(&self, game: &Wordle) -> Option<Word> {
        ["soare", "until", "pygmy", "whack"]
            .get(game.guessed_words.len())
            .map(|w| w.to_word())
    }
}

struct MostFrequentCharactersOfRemainingWords;
impl PickBestWord for MostFrequentCharactersOfRemainingWords {
    fn pick(&self, game: &Wordle) -> Option<Word> {
        let played_chars: HashSet<char> = game.guessed_words.iter().flatten().cloned().collect();
        let remaining_words_with_unique_unplayed_chars: Vec<_> = game
            .words
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
                (count, word)
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
        }
        let used_chars: HashSet<char> = game.guessed_words.iter().flatten().cloned().collect();
        let words_with_most_new_chars: Vec<&Word> =
            words_with_most_new_chars(&used_chars, &COMBINED_WORDS)
                .into_iter()
                .map(|(_, word)| word)
                .collect();

        let scores: Vec<_> = words_with_most_new_chars
            .iter()
            .map(|&word| {
                let count = COMBINED_GLOBAL_CHAR_COUNT_SUMS_BY_WORD[word];
                (count, word)
            })
            .collect();
        // if scores.len() > 10 {
        //     println!("{} unplayed words", scores.len());
        // } else if scores.is_empty() {
        //     println!("no more unplayed words");
        // } else {
        //     scores.sort_asc();
        //     println!("best unplayed words {}", scores.to_string());
        // }
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
                (count, word)
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
                (count, word)
            })
            .collect();
        scores.highest()
    }
}

struct MatchingMostOtherWordsInAtLeastOneOpenPosition;
impl PickBestWord for MatchingMostOtherWordsInAtLeastOneOpenPosition {
    fn pick(&self, game: &Wordle) -> Option<Word> {
        let words = &game.solutions;
        let positions = &game.open_positions();
        // for each word, find out how many other words it matches in any open position
        let mut scores: Vec<(usize, &Word)> = words.iter().map(|word| (0, word)).collect();
        let open_chars: Vec<_> = words.iter().map(|word| word.chars_in(positions)).collect();
        for (i, chars_a) in open_chars.iter().enumerate().take(open_chars.len() - 1) {
            for (j, chars_b) in open_chars.iter().enumerate().skip(i + 1) {
                assert_ne!(i, j);
                assert_ne!(chars_a, chars_b);
                let any_open_position_matches_exactly =
                    chars_a.iter().zip(chars_b.iter()).any(|(a, b)| a == b);
                if any_open_position_matches_exactly {
                    scores[i].0 += 1;
                    scores[j].0 += 1;
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
        let mut words = &game.high_variety_words(positions);
        if words.is_empty() {
            words = &game.solutions;
        }
        // for each word, find out how many other words it matches in any open position
        let mut scores: Vec<(usize, &Word)> = words.iter().map(|word| (0, word)).collect();
        let open_chars: Vec<_> = words.iter().map(|word| word.chars_in(positions)).collect();
        for (i, chars_a) in open_chars.iter().enumerate().take(open_chars.len() - 1) {
            for (j, chars_b) in open_chars.iter().enumerate().skip(i + 1) {
                assert_ne!(i, j);
                assert_ne!(chars_a, chars_b);
                let any_open_position_matches_exactly =
                    chars_a.iter().zip(chars_b.iter()).any(|(a, b)| a == b);
                if any_open_position_matches_exactly {
                    scores[i].0 += 1;
                    scores[j].0 += 1;
                }
            }
        }
        scores.highest()
    }
}

trait ToWord {
    fn to_word(&self) -> Word;
}
impl ToWord for &str {
    fn to_word(&self) -> Word {
        self.chars().collect()
    }
}

trait HintValue {
    fn value(&self) -> usize;
}

#[derive(Copy, Clone)]
enum Hint {
    Illegal,  // Not in the word at all
    WrongPos, // In word but at other position
    Correct,  // Correct at this position
}
impl HintValue for Hint {
    fn value(&self) -> usize {
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
impl HintValue for Hints {
    fn value(&self) -> usize {
        const MULTIPLIERS: [usize; 5] = [1, 3, 9, 27, 81];
        self.hints
            .iter()
            .enumerate()
            .map(|(i, h)| MULTIPLIERS[i] * h.value())
            .sum()
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

fn try_out_all_words_with_each_other(words: &[Word], positions: &[usize]) -> Vec<[usize; 243]> {
    let open_chars: Vec<_> = words.iter().map(|word| word.chars_in(positions)).collect();
    open_chars
        .par_iter()
        .map(|guess| {
            let mut bucket = [0; 243];
            for solution in open_chars.iter() {
                let (hint, _) = get_hints(guess, solution);
                bucket[hint.value()] += 1;
            }
            bucket
        })
        .collect()
}

/// Each guessed word results in a hint that depends on the wanted word.
/// For each character in a guess there are 3 options:
/// - The solution contains it in exactly this position: 🟩, given value 2
/// - The solution contains it, but a different position: 🟨, given value 1
/// - The solution does not contain this character anywhere: ⬛️, given value 0
///
/// As each of the 5 positions can result in one of 3 states, there are 3^5 = 243 possible hints.
/// Let's assign a number to each one. We multiply the values 0, 1 or 2 with a multiplier 3 ^ i,
/// which depends on its index `i` within the word (the first index being 0).
///
/// This function returns two hints. The first with w1 as the guess and w2 as the solution,
/// and the second with w2 as the guess and w1 as the solution
#[allow(clippy::ptr_arg)] // Because trait WordAsCharVec is implemented for Word not &[char]
fn get_hints(word1: &Word, word2: &Word) -> (Hints, Hints) {
    // Initialize as every position incorrect
    let mut hint1 = Hints::default(); // Hint for guessed word1 with solution word2
    let mut hint2 = Hints::default(); // Hint for guessed word2 with solution word1

    // Fill in exact matches
    let mut open_positions = vec![];
    for i in 0..5 {
        if word1[i] == word2[i] {
            hint1.set_correct(i);
            hint2.set_correct(i);
        } else {
            open_positions.push(i);
        }
    }

    // For characters at another position, consider only characters not previously matched
    // For example:
    // "eject" and "geese" match exactly in the middle character, leaving open "ej_ct" and "ge_se".
    // Both first 'e's in will have a match in the respective other word,
    // but the last 'e' in "geese" is illegal, because all the 'e's in "ej_ct" were already matched.
    for &i in &open_positions {
        let considered_char_count = |word: &[char], ch: &char| {
            word.iter()
                .take(i + 1) // include current pos
                .enumerate()
                .filter(|(i, c)| c == &ch && open_positions.contains(i))
                .count()
        };
        let char1 = &word1[i];
        let char2 = &word2[i];
        // println!(
        //     "considered_char_count('{}', {}) = {} | '{}'.total_char_count({:?}, {}) = {}",
        //     word1.to_string(),
        //     char1,
        //     considered_char_count(word1, char1),
        //     word2.to_string(),
        //     open_positions,
        //     char1,
        //     word2.total_char_count(&open_positions, char1)
        // );
        if considered_char_count(word1, char1) <= word2.total_char_count(&open_positions, char1) {
            hint1.set_wrong_pos(i);
        }
        if considered_char_count(word2, char2) <= word1.total_char_count(&open_positions, char2) {
            hint2.set_wrong_pos(i);
        }
    }
    (hint1, hint2)
}

fn main() {
    // print_word_combinations();
    Wordle::new(SOLUTIONS).play()
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
    let words: Vec<Word> = COMBINED.lines().map(|word| word.to_word()).collect();
    let best_words = words_with_most_new_chars(used_chars, &words);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[ignore]
    #[test]
    // 16s normally, 3.5s parallel
    // Average attempts = 3.335; 0 (0.000%) failed games (> 6 attempts):
    // 1: 29, 2: 322, 3: 963, 4: 853, 5: 142, 6: 6
    fn auto_play_tubes_fling_champ_wordy_every_time() {
        let words: Vec<_> = SOLUTIONS.trim().lines().map(|w| w.to_word()).collect();
        let attempts: Vec<usize> = words
            .into_par_iter()
            .map(|word| Wordle::new(SOLUTIONS).autoplay(word, TubesFlingChampWordyEveryTime))
            .collect();
        print_stats(attempts);
    }

    #[ignore]
    #[test]
    // 16s normally, 2s parallel
    // Without "whack":
    // Average attempts = 3.349; 9 (0.389%) failed games (> 6 attempts):
    // 1: 22, 2: 481, 3: 714, 4: 907, 5: 156, 6: 26, 7: 8, 8: 1
    // With "whack":
    // Average attempts = 3.479; 4 (0.173%) failed games (> 6 attempts):
    // 1: 22, 2: 481, 3: 714, 4: 610, 5: 444, 6: 40, 7: 4
    fn auto_play_soare_until_pygmy_whack_every_time() {
        let words: Vec<_> = SOLUTIONS.trim().lines().map(|w| w.to_word()).collect();
        let attempts: Vec<usize> = words
            .into_par_iter()
            .map(|word| Wordle::new(SOLUTIONS).autoplay(word, SoareUntilPygmyWhackEveryTime))
            .collect();
        print_stats(attempts);
    }

    #[ignore]
    #[test]
    // 21s normally, 3s parallel
    // Average attempts = 3.349; 9 (0.389%) failed games (> 6 attempts):
    // 1: 22, 2: 481, 3: 714, 4: 907, 5: 156, 6: 26, 7: 8, 8: 1
    fn auto_play_quick_brown_foxed_jumps_glazy_vetch_every_time() {
        let words: Vec<_> = SOLUTIONS.trim().lines().map(|w| w.to_word()).collect();
        let attempts: Vec<usize> = words
            .into_par_iter()
            .map(|word| {
                Wordle::new(SOLUTIONS).autoplay(word, QuickBrownFoxedJumpsGlazyVetchEveryTime)
            })
            .collect();
        print_stats(attempts);
    }

    #[ignore]
    #[test]
    // 62s normally, 6s parallel
    // Average attempts 3.067; 9 (0.389%) failed games (> 6 attempts):
    // 1: 34, 2: 537, 3: 1145, 4: 467, 5: 114, 6: 9, 7: 8, 8: 1
    fn auto_play_most_frequent_global_characters() {
        let words: Vec<_> = SOLUTIONS.trim().lines().map(|w| w.to_word()).collect();
        let attempts: Vec<usize> = words
            .into_par_iter()
            .map(|word| Wordle::new(SOLUTIONS).autoplay(word, MostFrequentGlobalCharacter))
            .collect();
        print_stats(attempts);
    }

    #[ignore]
    #[test]
    // 60s normally, 9s parallel
    // Average attempts = 3.073; 12 (0.518%) failed games (> 6 attempts):
    // 1: 14, 2: 598, 3: 1121, 4: 423, 5: 120, 6: 27, 7: 9, 8: 2, 9: 1
    fn auto_play_most_frequent_global_characters_high_variety_word() {
        let words: Vec<_> = SOLUTIONS.trim().lines().map(|w| w.to_word()).collect();
        let attempts: Vec<usize> = words
            .into_par_iter()
            .map(|word| {
                Wordle::new(SOLUTIONS).autoplay(word, MostFrequentGlobalCharacterHighVarietyWord)
            })
            .collect();
        print_stats(attempts);
    }

    #[ignore]
    #[test]
    // 32s normally, 4s parallel
    // Average attempts 3.098; 7 (0.302%) failed games (> 6 attempts):
    // 1: 29, 2: 522, 3: 1133, 4: 498, 5: 98, 6: 28, 7: 6, 8: 1
    fn auto_play_most_frequent_characters_per_pos() {
        let words: Vec<_> = SOLUTIONS.trim().lines().map(|w| w.to_word()).collect();
        let attempts: Vec<_> = words
            .into_par_iter()
            .map(|word| Wordle::new(SOLUTIONS).autoplay(word, MostFrequentCharacterPerPos))
            .collect();
        print_stats(attempts);
    }

    #[ignore]
    #[test]
    // 45s normally, 7s parallel
    // Average attempts = 2.998; 6 (0.259%) failed games (> 6 attempts):
    // 1: 29, 2: 599, 3: 1183, 4: 390, 5: 86, 6: 22, 7: 5, 8: 1
    fn auto_play_most_frequent_characters_per_pos_high_variety_word() {
        let words: Vec<_> = SOLUTIONS.trim().lines().map(|w| w.to_word()).collect();
        let attempts: Vec<_> = words
            .into_par_iter()
            .map(|word| {
                Wordle::new(SOLUTIONS).autoplay(word, MostFrequentCharacterPerPosHighVarietyWord)
            })
            .collect();
        print_stats(attempts);
    }

    #[ignore]
    #[test]
    // 107s normally, 20s parallel
    // Average attempts = 3.223; 12 (0.518%) failed games (> 6 attempts):
    // 1: 14, 2: 394, 3: 1194, 4: 542, 5: 129, 6: 30, 7: 11, 8: 1
    fn auto_play_most_frequent_characters_of_words() {
        let words: Vec<_> = SOLUTIONS.trim().lines().map(|w| w.to_word()).collect();
        let attempts: Vec<usize> = words
            .into_par_iter()
            .map(|word| {
                Wordle::new(SOLUTIONS).autoplay(word, MostFrequentCharactersOfRemainingWords)
            })
            .collect();
        print_stats(attempts);
    }

    // #[ignore]
    #[test]
    // Estimated ~10min normally, 59s parallel
    // Average attempts = 3.151; 8 (0.346%) failed games (> 6 attempts):
    // 1: 22, 2: 509, 3: 1097, 4: 522, 5: 122, 6: 35, 7: 8
    fn auto_play_most_frequent_unused_characters() {
        let words: Vec<_> = SOLUTIONS.trim().lines().map(|w| w.to_word()).collect();
        let attempts: Vec<usize> = words
            .into_par_iter()
            // .skip(1588)
            // .take(1)
            .map(|word| Wordle::new(SOLUTIONS).autoplay(word, MostFrequentUnusedCharacters))
            .collect();
        print_stats(attempts);
    }

    #[ignore]
    #[test]
    // 48min29s normally, 4min45s parallel
    // Average attempts = 3.357; 16 (0.691%) failed games (> 6 attempts):
    // 1: 27, 2: 397, 3: 957, 4: 672, 5: 201, 6: 45, 7: 11, 8: 5
    fn auto_play_most_other_words_in_at_least_one_open_position() {
        let words: Vec<_> = SOLUTIONS.trim().lines().map(|w| w.to_word()).collect();
        let attempts: Vec<_> = words
            .into_par_iter()
            .map(|word| {
                Wordle::new(SOLUTIONS)
                    .autoplay(word, MatchingMostOtherWordsInAtLeastOneOpenPosition)
            })
            .collect();
        print_stats(attempts);
    }

    #[ignore]
    #[test]
    // 17min51s normally, 136s parallel
    // Average attempts = 3.144; 12 (0.518%) failed games (> 6 attempts):
    // 1: 20, 2: 540, 3: 1072, 4: 512, 5: 124, 6: 35, 7: 12
    fn auto_play_most_other_words_in_at_least_one_open_position_high_variety_word() {
        let words: Vec<_> = SOLUTIONS.trim().lines().map(|w| w.to_word()).collect();
        let attempts: Vec<_> = words
            .into_par_iter()
            .map(|word| {
                Wordle::new(SOLUTIONS).autoplay(
                    word,
                    MatchingMostOtherWordsInAtLeastOneOpenPositionHighVarietyWord,
                )
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
            "Average attempts = {:.3}; {} ({:.3}%) failed games (> 6 attempts):\n{}",
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
        let game = Wordle::new(SOLUTIONS);
        let word = WordThatResultsInFewestPossibleRemainingWords.pick(&game);

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
    // Takes around 207s for the 12'972 words in the combined list!
    #[test]
    fn test_word_that_results_in_fewest_remaining_possible_words_for_full_word_list() {
        let game = Wordle::new(SOLUTIONS);
        let word = WordThatResultsInFewestPossibleRemainingWords.pick(&game);

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
    // Takes around 6s for the 2'315 solution words
    #[test]
    fn test_word_dividing_search_space_most_evenly() {
        let game = Wordle::new(SOLUTIONS);
        let word = WordDividingSearchSpaceMostEvenly.pick(&game);

        // Worst/highest variance:
        // - 6595.3 jazzy
        // - 6801.7 fluff
        // - 6845.0 fizzy
        // - 6908.4 jiffy
        // - 6919.4 civic
        // - 7319.3 puppy
        // - 7356.4 mamma
        // - 7677.5 vivid
        // - 7728.2 mummy
        // - 8061.1 fuzzy

        // Best/lowest variance:
        // - 591.1 slate
        // - 588.4 stare
        // - 586.6 snare
        // - 578.2 later
        // - 577.3 saner
        // - 576.0 alter
        // - 538.2 arose
        // - 516.9 irate
        // - 516.3 arise
        // - 490.3 raise

        assert_eq!(word.unwrap().to_string(), "raise");
    }
    #[ignore] // Takes around 202s for the 12'972 words in the combined list!
    #[test]
    fn test_word_dividing_search_space_most_evenly_for_full_word_list() {
        let game = Wordle::new(SOLUTIONS);

        let word = WordDividingSearchSpaceMostEvenly.pick(&game);

        // Worst/highest variance:
        // - 260640.7 jugum
        // - 261668.0 yukky
        // - 262751.1 bubby
        // - 265548.4 cocco
        // - 266296.7 fuzzy
        // - 276147.6 immix
        // - 276519.6 hyphy
        // - 284332.9 gyppy
        // - 284978.8 xylyl
        // - 285207.7 fuffy

        // Best/lowest variance:
        // - 13973.0 serai
        // - 13951.6 arles
        // - 13771.3 rates
        // - 13684.7 aeros
        // - 13461.3 nares
        // - 13419.3 reais
        // - 13369.6 soare
        // - 13298.1 tares
        // - 12743.7 rales
        // - 12563.9 lares

        assert_eq!(word.unwrap().to_string(), "lares");
    }

    #[ignore]
    #[test]
    fn test_pick_word_that_exactly_matches_most_others_in_at_least_one_open_position() {
        let game = Wordle::new(SOLUTIONS);
        let word = MatchingMostOtherWordsInAtLeastOneOpenPosition.pick(&game);
        assert_eq!(word.unwrap().to_string(), "sauce");
    }

    #[test]
    fn test_char_hint_values() {
        assert_eq!(Hint::from('⬛').value(), 0);
        assert_eq!(Hint::from('🟨').value(), 1);
        assert_eq!(Hint::from('🟩').value(), 2);
    }

    #[test]
    #[allow(clippy::identity_op)] // For the 1_usize * a
    fn test_word_hint_values() {
        let value = |a, b, c, d, e| -> usize {
            1_usize * a + 3_usize * b + 9_usize * c + 27_usize * d + 81_usize * e
        };
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
    #[test]
    fn test_get_hints() {
        let (hint1, hint2) = get_hints(&"guest".to_word(), &"truss".to_word());
        assert_eq!("⬛🟨⬛🟩🟨", hint1.to_string());
        assert_eq!("🟨⬛🟨🟩⬛", hint2.to_string());

        let (hint1, hint2) = get_hints(&"briar".to_word(), &"error".to_word());
        assert_eq!("⬛🟩⬛⬛🟩", hint1.to_string());
        assert_eq!("⬛🟩⬛⬛🟩", hint2.to_string());

        let (hint1, hint2) = get_hints(&"sissy".to_word(), &"truss".to_word());
        assert_eq!("🟨⬛⬛🟩⬛", hint1.to_string());
        assert_eq!("⬛⬛⬛🟩🟨", hint2.to_string());

        let (hint1, hint2) = get_hints(&"eject".to_word(), &"geese".to_word());
        assert_eq!("🟨⬛🟩⬛⬛", hint1.to_string());
        assert_eq!("⬛🟨🟩⬛⬛", hint2.to_string());

        let (hint1, _) = get_hints(&"three".to_word(), &"beret".to_word());
        assert_eq!("🟨⬛🟩🟩🟨", hint1.to_string());
    }
}
