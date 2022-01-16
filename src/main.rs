use rand::prelude::*;
use std::cmp::{Ordering, Reverse};
use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::io;

const WORDLIST: &str = include_str!("../data/subset_of_actual_wordles.txt");

type Word = Vec<char>;

// Helper for https://www.powerlanguage.co.uk/wordle/
struct Wordle {
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
        let words: Vec<Word> = wordlist.lines().map(|word| word.to_word()).collect();
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
        let open_positions = self.open_positions();

        // println!("open positions {:?}", open_positions);
        let freq = self.character_frequency_of_open_positions(&open_positions);
        // println!("Overall character counts: {}", freq.to_string());
        let freqs = self.character_frequencies_of_open_positions(&open_positions);
        // for (i, freq) in freqs.iter().enumerate() {
        //     println!("Position[{}] character counts: {}", i, freq.to_string());
        // }

        // Called just to print the top words
        self.max_char_freqs_sum_word(&self.words, &freqs, &open_positions);
        self.max_char_freq_sum_word(&self.words, &freq, &open_positions);

        let high_variety_words = self.high_variety_words(&open_positions);
        println!(
            "{}/{} are \"high-variety\" words with different characters in all the open spots",
            high_variety_words.len(),
            self.words.len()
        );

        // Called just to print the top high variety words
        self.max_char_freqs_sum_word(&high_variety_words, &freqs, &open_positions);
        self.max_char_freq_sum_word(&high_variety_words, &freq, &open_positions);
        self.find_word_matching_most_others_in(&open_positions);

        self.find_word_dividing_up_search_space_most_evenly(&open_positions)
    }

    fn find_word_matching_most_others_in(&self, positions: &[usize]) -> Option<Word> {
        // for each word, find out how many other words it matches in any open position
        let mut scores: Vec<(usize, &Word)> = self.words.iter().map(|word| (0, word)).collect();
        let char_position_sets: Vec<_> = self
            .words
            .iter()
            .map(|word| (word, word.char_position_set(positions)))
            .collect();
        println!("open positions {:?}", positions);
        for (i, (word_a, set_a)) in char_position_sets
            .iter()
            .enumerate()
            .take(char_position_sets.len() - 1)
        {
            for (j, (word_b, set_b)) in char_position_sets.iter().enumerate().skip(i + 1) {
                assert_ne!(i, j);
                assert_ne!(word_a, word_b);
                if !set_a.is_disjoint(set_b) {
                    scores[i].0 += 1;
                    scores[j].0 += 1;
                }
            }
        }
        scores.sort_desc();
        println!("Matching most:  {}", scores.to_string());
        scores.highest()
    }

    fn find_word_dividing_up_search_space_most_evenly(&self, positions: &[usize]) -> Option<Word> {
        // Inspired by Sean Plays https://youtu.be/BN-Yan03m8s
        let mut buckets: Vec<[usize; 243]> = vec![[0; 243]; self.words.len()];
        let word_and_open_chars: Vec<_> = self
            .words
            .iter()
            .map(|word| (word, word.chars_in(positions)))
            .collect();
        for (idx_a, (word_a, chars_a)) in word_and_open_chars
            .iter()
            .enumerate()
            .take(word_and_open_chars.len() - 1)
        {
            for (idx_b, (word_b, chars_b)) in word_and_open_chars.iter().enumerate().skip(idx_a + 1)
            {
                assert_ne!(idx_a, idx_b);
                assert_ne!(word_a, word_b);
                let (bucket_a, bucket_b) = Wordle::result_bucket(chars_a, chars_b);
                buckets[idx_a][bucket_a] += 1; // word_a was the guess
                buckets[idx_b][bucket_b] += 1; // word_b was the guess
            }
        }
        let averages: Vec<_> = buckets
            .iter()
            .map(|bucket| bucket.iter().sum::<usize>() as f64 / bucket.len() as f64)
            .collect();
        let variances: Vec<_> = buckets
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
        let mut scores: Vec<(f64, &Word)> = self
            .words
            .iter()
            .enumerate()
            .map(|(i, word)| (variances[i], word))
            .collect();
        scores.sort_desc();
        println!("Dividing worst: {}", scores.to_string());
        scores.sort_asc();
        println!("Dividing best:  {}", scores.to_string());

        scores.lowest()
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
    /// This function returns two values. The first with w1 as the guess and w2 as the solution,
    /// and the second value with w2 as the guess and w1 as the solution
    fn result_bucket(w1: &[char], w2: &[char]) -> (usize, usize) {
        const MULTIPLIERS: [usize; 5] = [1, 3, 9, 27, 81];

        // Exact matches
        let mut exact_match_sum = 0;
        let mut open_positions = vec![];
        w1.iter().zip(w2).enumerate().for_each(|(i, (c1, c2))| {
            if c1 == c2 {
                exact_match_sum += MULTIPLIERS[i] * 2; // exact match
            } else {
                open_positions.push(i);
            }
        });
        // println!("exact_match_sum {}", exact_match_sum);
        let (mut sum1, mut sum2) = (exact_match_sum, exact_match_sum);

        // Characters that are in in the solution, but at another position. These are a bit tricky,
        // because we must only consider the open positions, and also the remaining number of
        // these characters

        let total_char_count = |solution: &[char], ch: &char| {
            solution
                .iter()
                .enumerate()
                .filter(|(i, _)| open_positions.contains(i))
                .filter(|(_, c)| c == &ch)
                .count()
        };
        for (i, (c1, c2)) in w1
            .iter()
            .zip(w2.iter())
            .enumerate()
            .filter(|(i, _)| open_positions.contains(i))
        {
            let considered_char_count = |word: &[char], ch: &char| {
                word.iter()
                    .take(i + 1) // include current pos
                    .enumerate()
                    .filter(|(i, _)| open_positions.contains(i))
                    .filter(|(_, c)| c == &ch)
                    .count()
            };

            // println!(
            //     "({}, {}) @ {}: {} <= {} = {}, {} <= {} = {}",
            //     c1,
            //     c2,
            //     i,
            //     considered_char_count(w1, c1),
            //     total_char_count(w2, c1),
            //     considered_char_count(w1, c1) <= total_char_count(w2, c1),
            //     considered_char_count(w2, c2),
            //     total_char_count(w1, c2),
            //     considered_char_count(w2, c2) <= total_char_count(w1, c2)
            // );
            if considered_char_count(w1, c1) <= total_char_count(w2, c1) {
                sum1 += MULTIPLIERS[i];
            }
            if considered_char_count(w2, c2) <= total_char_count(w1, c2) {
                sum2 += MULTIPLIERS[i];
            }
        }
        (sum1, sum2)
    }

    fn high_variety_words(&self, open_positions: &[usize]) -> Vec<Word> {
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
        words: &[Word],
        freq: &HashMap<char, usize>,
        open_positions: &[usize],
    ) -> Option<Word> {
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
        scores.sort_desc();

        // Best overall with char_set_at (no duplicate counts):
        // 4405 renal, 4405 learn, 4451 raise, 4451 arise, 4509 stare,
        // 4511 irate, 4534 arose, 4559 alter, 4559 alert, 4559 later

        // Best overall with char_vec_at (with duplicate counts):
        // 4763 terse, 4795 tepee, 4833 easel, 4833 lease, 4843 tease,
        // 4893 elate, 4909 rarer, 5013 erase, 5073 eater, 5269 eerie
        println!("Best overall:    {}", scores.to_string());
        scores.highest()
    }

    fn max_char_freqs_sum_word(
        &self,
        words: &[Word],
        freqs: &[HashMap<char, usize>; 5],
        open_positions: &[usize],
    ) -> Option<Word> {
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
        scores.sort_desc();
        println!("Best individual: {}", scores.to_string());
        scores.highest()
    }

    fn random_suggestion(&mut self) -> Word {
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
        println!(
            "Enter characters in the correct spot. Use any non-alphabetic char as prefix if necessary:"
        );
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        let correct_pos: Word = input.trim().to_word();
        for (i, c) in correct_pos
            .iter()
            .enumerate()
            .filter(|(_, c)| c.is_ascii_alphabetic())
            .map(|(i, c)| (i, c.to_ascii_lowercase()))
        {
            println!("Inserting '{}' as correct character @ {}", c, i);
            self.correct_chars[i] = Some(c);
        }
    }

    fn ask_about_correct_chars_in_wrong_position(&mut self) {
        let mut input = String::new();
        println!("Enter correct characters in the wrong spot. Use any non-alphabetic char as prefix if necessary:");
        io::stdin().read_line(&mut input).unwrap();

        let wrong_pos: Word = input.trim().to_word();
        for (i, c) in wrong_pos
            .iter()
            .enumerate()
            .filter(|(_, c)| c.is_ascii_alphabetic())
            .map(|(i, c)| (i, c.to_ascii_lowercase()))
        {
            println!("Inserting '{}' as illegal @ {}", c, i);
            self.illegal_at_pos[i].insert(c);
            self.mandatory_chars.insert(c);
        }
    }

    fn update_illegal_chars(&mut self, guess: Word) {
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
    fn chars_in(&self, positions: &[usize]) -> Word;
    fn char_position_set(&self, positions: &[usize]) -> HashSet<(usize, char)>;
}
impl WordAsCharVec for Word {
    fn to_string(&self) -> String {
        self.iter().collect::<String>()
    }
    fn unique_chars_in(&self, positions: &[usize]) -> HashSet<char> {
        positions.iter().map(|&i| self[i]).collect()
    }
    fn chars_in(&self, positions: &[usize]) -> Word {
        positions.iter().map(|&i| self[i]).collect()
    }
    fn char_position_set(&self, positions: &[usize]) -> HashSet<(usize, char)> {
        positions.iter().map(|&i| (i, self[i])).collect()
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
            .skip(self.len().saturating_sub(10))
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

trait ToWord {
    fn to_word(&self) -> Word;
}
impl ToWord for &str {
    fn to_word(&self) -> Word {
        self.chars().collect()
    }
}

fn main() {
    Wordle::new(WORDLIST).play()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::identity_op, clippy::erasing_op)]
    fn test_bucket() {
        let (s1, s2) = Wordle::result_bucket(&"guess".to_word(), &"truss".to_word());
        assert_eq!(1 * 0 + 3 * 1 + 9 * 0 + 27 * 2 + 81 * 2, s1); // guess "guess" for solution "truss"
        assert_eq!(1 * 0 + 3 * 0 + 9 * 1 + 27 * 2 + 81 * 2, s2); // guess "truss" for solution "guess"

        let (s1, s2) = Wordle::result_bucket(&"briar".to_word(), &"error".to_word());
        assert_eq!(1 * 0 + 3 * 2 + 9 * 0 + 27 * 0 + 81 * 2, s1); // guess "briar" for wanted "error"
        assert_eq!(1 * 0 + 3 * 2 + 9 * 0 + 27 * 0 + 81 * 2, s2); // guess "error" for wanted "briar"

        let (s1, s2) = Wordle::result_bucket(&"sissy".to_word(), &"truss".to_word());
        assert_eq!(1 * 1 + 3 * 0 + 9 * 0 + 27 * 2 + 81 * 0, s1); // guess "sissy" for wanted "truss"
        assert_eq!(1 * 0 + 3 * 0 + 9 * 0 + 27 * 2 + 81 * 1, s2); // guess "truss" for wanted "sissy"

        let (s1, s2) = Wordle::result_bucket(&"abaca".to_word(), &"badaa".to_word());
        assert_eq!(1 * 1 + 3 * 1 + 9 * 1 + 27 * 0 + 81 * 2, s1); // guess "abaca" for wanted "badae"
        assert_eq!(1 * 1 + 3 * 1 + 9 * 0 + 27 * 1 + 81 * 2, s2); // guess "badae" for wanted "abaca"

        let (s1, s2) = Wordle::result_bucket(&"aaabc".to_word(), &"axyaa".to_word());
        assert_eq!(1 * 2 + 3 * 1 + 9 * 1 + 27 * 0 + 81 * 0, s1); // guess "aaabc" for wanted "axyaa"
        assert_eq!(1 * 2 + 3 * 0 + 9 * 0 + 27 * 1 + 81 * 1, s2); // guess "axyaa" for wanted "aaabc"
    }
}
