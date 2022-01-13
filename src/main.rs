use rand::prelude::*;
use std::cmp::{Ordering, Reverse};
use std::collections::{HashMap, HashSet};
use std::io;

const WORDLIST: &str = include_str!("../data/subset_of_actual_wordles.txt");

// Helper for https://www.powerlanguage.co.uk/wordle/
struct Wordle {
    words: Vec<Vec<char>>,
    illegal_chars: HashSet<char>,
    correct_chars: [Option<char>; 5],
    illegal_at_pos: [HashSet<char>; 5],
    mandatory_chars: HashSet<char>,
    guessed_words: HashSet<Vec<char>>,
    rng: ThreadRng,
}
impl Wordle {
    fn new(wordlist: &str) -> Self {
        let words: Vec<Vec<char>> = wordlist
            .lines()
            .map(|word| word.chars().collect())
            .collect();
        let illegal_position_for_char: [HashSet<char>; 5] = [
            HashSet::new(),
            HashSet::new(),
            HashSet::new(),
            HashSet::new(),
            HashSet::new(),
        ];
        let rng = thread_rng();
        Wordle {
            words,
            illegal_chars: HashSet::new(),
            correct_chars: [None; 5],
            illegal_at_pos: illegal_position_for_char,
            mandatory_chars: HashSet::new(),
            guessed_words: HashSet::new(),
            rng,
        }
    }
    fn play(&mut self) {
        while !self.is_game_over() {
            self.print_remaining_word_count();
            self.single_round();
        }
        self.print_result();
    }
    fn is_game_over(&self) -> bool {
        self.correct_chars.iter().all(|o| o.is_some()) || self.words.len() <= 1
    }
    fn print_remaining_word_count(&self) {
        if self.words.len() > 10 {
            println!("\n{} words left", self.words.len());
        } else {
            println!(
                "\n{} words left: {}",
                self.words.len(),
                self.words
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
        self.ask_about_correct_chars_in_correct_position();
        if self.is_game_over() {
            return;
        }
        self.ask_about_correct_chars_in_wrong_position();
        self.update_illegal_chars(guess);

        self.update_words();
    }

    fn ask_for_guess(&mut self) -> Vec<char> {
        let suggestion = self.suggest_a_word();
        println!(
            "Enter the word you guessed, or use suggestion '{}':",
            suggestion.to_string()
        );
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        if !input.trim().is_empty() {
            input.trim().chars().collect()
        } else {
            suggestion
        }
    }

    fn suggest_a_word(&mut self) -> Vec<char> {
        self.high_variety_suggestion()
            .unwrap_or_else(|| self.random_suggestion())
    }

    fn high_variety_suggestion(&mut self) -> Option<Vec<char>> {
        let open_positions = self.open_positions();
        // println!("open positions {:?}", open_positions);
        let freq = self.character_frequency_of_open_positions(&open_positions);
        // println!("Overall character counts: {}", freq.to_string());
        let freqs = self.character_frequencies_of_open_positions(&open_positions);
        // for (i, freq) in freqs.iter().enumerate() {
        //     println!("Position[{}] character counts: {}", i, freq.to_string());
        // }

        let high_variety_words = self.high_variety_words(&open_positions);
        println!(
            "{}/{} words have different characters in all the open spots",
            high_variety_words.len(),
            self.words.len()
        );

        // Called just to print the top words
        self.max_char_freqs_sum_word(&self.words, &freqs, &open_positions);
        self.max_char_freq_sum_word(&self.words, &freq, &open_positions);

        println!("Enforced high-variety words only:");
        if let Some(individual_variety_word) =
            self.max_char_freqs_sum_word(&high_variety_words, &freqs, &open_positions)
        {
            // Called just to print the top high variety words
            self.max_char_freq_sum_word(&high_variety_words, &freq, &open_positions);

            Some(individual_variety_word)
        } else {
            high_variety_words.into_iter().choose(&mut self.rng)
        }
    }

    fn high_variety_words(&self, open_positions: &[usize]) -> Vec<Vec<char>> {
        self.words
            .iter()
            .filter(|&word| word.unique_chars_in(open_positions).len() == open_positions.len())
            .cloned()
            .collect()
    }

    fn character_frequency_of_open_positions(
        &self,
        open_positions: &[usize],
    ) -> HashMap<char, usize> {
        let mut freq: HashMap<char, usize> = HashMap::new();
        for word in &self.words {
            for i in open_positions {
                *freq.entry(word[*i]).or_default() += 1;
            }
        }
        freq
    }

    fn character_frequencies_of_open_positions(
        &self,
        open_positions: &[usize],
    ) -> [HashMap<char, usize>; 5] {
        let empty = || ('a'..='z').into_iter().map(|c| (c, 0)).collect();
        let mut freqs: [HashMap<char, usize>; 5] = [empty(), empty(), empty(), empty(), empty()];
        for word in &self.words {
            for i in open_positions {
                *freqs[*i].get_mut(&word[*i]).unwrap() += 1;
            }
        }
        freqs
    }

    fn max_char_freq_sum_word(
        &self,
        words: &[Vec<char>],
        freq: &HashMap<char, usize>,
        open_positions: &[usize],
    ) -> Option<Vec<char>> {
        let mut scores: Vec<_> = words
            .iter()
            .map(|word| {
                let count = word
                    .unique_chars_in(open_positions)
                    .iter()
                    .map(|c| freq[c])
                    .sum::<usize>();
                (count, word)
            })
            .collect();
        if scores.is_empty() {
            return None;
        }
        scores.sort_unstable_by(|(a_cnt, a_word), (b_cnt, b_word)| match a_cnt.cmp(b_cnt) {
            Ordering::Equal => a_word.cmp(b_word),
            by_count => by_count,
        });

        // Best overall with char_set_at (no duplicate counts):
        // 4405 renal, 4405 learn, 4451 raise, 4451 arise, 4509 stare,
        // 4511 irate, 4534 arose, 4559 alter, 4559 alert, 4559 later

        // Best overall with char_vec_at (with duplicate counts):
        // 4763 terse, 4795 tepee, 4833 easel, 4833 lease, 4843 tease,
        // 4893 elate, 4909 rarer, 5013 erase, 5073 eater, 5269 eerie
        println!(
            "Best overall:    {}",
            scores
                .iter()
                .skip(scores.len().saturating_sub(10))
                .map(|(count, word)| format!("{} {}", count, word.to_string()))
                .collect::<Vec<_>>()
                .join(", ")
        );
        scores
            .iter()
            .max_by(|(a_cnt, a_word), (b_cnt, b_word)| match a_cnt.cmp(b_cnt) {
                Ordering::Equal => a_word.cmp(b_word),
                by_count => by_count,
            })
            .map(|(_count, word)| word.to_vec())
    }

    fn max_char_freqs_sum_word(
        &self,
        words: &[Vec<char>],
        freqs: &[HashMap<char, usize>; 5],
        open_positions: &[usize],
    ) -> Option<Vec<char>> {
        let mut scores: Vec<_> = words
            .iter()
            .map(|word| {
                let count = open_positions
                    .iter()
                    .map(|&i| freqs[i][&word[i]])
                    .sum::<usize>();
                (count, word)
            })
            .collect();
        if scores.is_empty() {
            return None;
        }
        scores.sort_unstable_by_key(|(count, _word)| *count);
        println!(
            "Best individual: {}",
            scores
                .iter()
                .skip(scores.len().saturating_sub(10))
                .map(|(count, word)| format!("{} {}", count, word.to_string()))
                .collect::<Vec<_>>()
                .join(", ")
        );
        scores
            .iter()
            .max_by_key(|(count, _word)| *count)
            .map(|(_count, word)| word.to_vec())
    }

    fn random_suggestion(&mut self) -> Vec<char> {
        self.words.iter().choose(&mut self.rng).unwrap().to_vec()
    }

    fn open_positions(&self) -> Vec<usize> {
        self.correct_chars
            .iter()
            .enumerate()
            .filter(|(_, o)| o.is_none())
            .map(|(i, _)| i)
            .collect()
    }

    fn ask_about_correct_chars_in_correct_position(&mut self) {
        println!("Enter characters in the correct spot. Use _ as prefix if necessary:");
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        let correct_pos: Vec<_> = input.trim().chars().collect();
        for (i, c) in correct_pos.iter().enumerate().filter(|(_, &c)| c != '_') {
            println!("Inserting '{}' as correct character @ {}", c, i);
            self.correct_chars[i] = Some(*c);
        }
    }

    fn ask_about_correct_chars_in_wrong_position(&mut self) {
        let mut input = String::new();
        println!("Enter correct characters in the wrong spot. Use _ as prefix if necessary:");
        io::stdin().read_line(&mut input).unwrap();

        let wrong_pos: Vec<_> = input.trim().chars().collect();
        for (i, c) in wrong_pos.iter().enumerate().filter(|(_, &c)| c != '_') {
            println!("Inserting '{}' as illegal @ {}", c, i);
            self.illegal_at_pos[i].insert(*c);
            self.mandatory_chars.insert(*c);
        }
    }

    fn update_illegal_chars(&mut self, guess: Vec<char>) {
        // println!("guess {:?}", guess);
        // println!("self.mandatory_chars {:?}", self.mandatory_chars);
        // println!("self.correct_chars {:?}", self.correct_chars);

        for (i, c) in guess
            .into_iter()
            .enumerate()
            .filter(|(_, c)| !self.mandatory_chars.contains(c))
        {
            if !self.correct_chars.iter().any(|&o| o == Some(c)) {
                println!("Inserting globally illegal char '{}'", c);
                self.illegal_chars.insert(c);
            } else if self.correct_chars[i] != Some(c) {
                println!("Inserting '{}' as illegal @ {}", c, i);
                self.illegal_at_pos[i].insert(c);
            }
        }
    }

    fn update_words(&mut self) {
        self.words = self
            .words
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
        if self.words.len() == 1 {
            println!(
                "\nThe only word left in the list is '{}'",
                self.words[0].to_string()
            );
        } else {
            let word: String = self.correct_chars.iter().map(|c| c.unwrap()).collect();
            println!("\nThe word is '{}'", word);
        }
    }
}

trait WordAsCharVec {
    fn to_string(&self) -> String;
    fn unique_chars_in(&self, positions: &[usize]) -> HashSet<char>;
    fn chars_in(&self, positions: &[usize]) -> Vec<char>;
}
impl WordAsCharVec for Vec<char> {
    fn to_string(&self) -> String {
        self.iter().collect::<String>()
    }
    fn unique_chars_in(&self, positions: &[usize]) -> HashSet<char> {
        positions.iter().map(|&i| self[i]).collect()
    }
    fn chars_in(&self, positions: &[usize]) -> Vec<char> {
        positions.iter().map(|&i| self[i]).collect()
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

fn main() {
    Wordle::new(WORDLIST).play()
}
