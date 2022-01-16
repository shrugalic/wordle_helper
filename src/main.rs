use rand::prelude::*;
use std::cmp::{Ordering, Reverse};
use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter};
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
        println!("open positions {:?}", open_positions);

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
        MatchingMostOtherWordsInAtLeastOneOpenPosition.pick(&self.words, &open_positions);

        let all_buckets = try_out_words_with_each_other(&self.words, &open_positions);
        self.word_dividing_up_search_space_most_evenly(&all_buckets);
        self.word_that_results_in_fewest_remaining_possible_words(&all_buckets)
    }

    fn word_dividing_up_search_space_most_evenly(
        &self,
        all_buckets: &[[usize; 243]],
    ) -> Option<Word> {
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
        let mut scores: Vec<(f64, &Word)> = self
            .words
            .iter()
            .enumerate()
            .map(|(i, word)| (variances[i], word))
            .collect();

        scores.sort_desc();
        println!("Remaining worst: {}", scores.to_string());
        scores.sort_asc();
        println!("Remaining best:  {}", scores.to_string());

        scores.lowest()
    }

    fn word_that_results_in_fewest_remaining_possible_words(
        &self,
        all_buckets: &[[usize; 243]],
    ) -> Option<Word> {
        let word_count = self.words.len() as f64;
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
                (expected_remaining_word_count, &self.words[i])
            })
            .collect();

        scores.sort_desc();
        println!("Remaining worst: {}", scores.to_string());
        scores.sort_asc();
        println!("Remaining best:  {}", scores.to_string());

        scores.lowest()
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

trait PickBestWord {
    fn pick(&self, list: &[Word], positions: &[usize]) -> Option<Word>;
}

struct MatchingMostOtherWordsInAtLeastOneOpenPosition;
impl PickBestWord for MatchingMostOtherWordsInAtLeastOneOpenPosition {
    fn pick(&self, words: &[Word], positions: &[usize]) -> Option<Word> {
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

        scores.sort_asc();
        println!("Matching fewest words: {}", scores.to_string());

        scores.sort_desc();
        println!("Matching most words:   {}", scores.to_string());

        // Worst 10
        // 320 lymph, 314 igloo, 313 umbra, 310 unzip, 308 affix,
        // 304 ethos, 301 jumbo, 298 ethic, 282 nymph, 279 inbox

        // Best 10
        // 1077 slate, 1095 sooty, 1097 scree, 1098 gooey, 1099 spree,
        // 1100 sense, 1104 saute, 1114 soapy, 1115 saucy, 1122 sauce

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

use Hint::*;
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

fn try_out_words_with_each_other(words: &[Word], positions: &[usize]) -> Vec<[usize; 243]> {
    let mut all_buckets: Vec<[usize; 243]> = vec![[0; 243]; words.len()];
    let word_and_open_chars: Vec<_> = words
        .iter()
        .map(|word| (word, word.chars_in(positions)))
        .collect();
    for (idx_a, (word_a, chars_a)) in word_and_open_chars
        .iter()
        .enumerate()
        .take(word_and_open_chars.len() - 1)
    {
        for (idx_b, (word_b, chars_b)) in word_and_open_chars.iter().enumerate().skip(idx_a + 1) {
            assert_ne!(idx_a, idx_b);
            assert_ne!(word_a, word_b);
            let (hint_a, hint_b) = get_hints(chars_a, chars_b);
            let bucket_a = hint_a.value();
            let bucket_b = hint_b.value();
            all_buckets[idx_a][bucket_a] += 1; // word_a was the guess
            all_buckets[idx_b][bucket_b] += 1; // word_b was the guess
        }
    }
    all_buckets
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
                .filter(|(_, c)| c == &ch)
                .count()
        };
        let char1 = &word1[i];
        let char2 = &word2[i];
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
    Wordle::new(WORDLIST).play()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pick_word_that_exactly_matches_most_others_in_at_least_one_open_position() {
        let words: Vec<Word> = WORDLIST.lines().map(|word| word.to_word()).collect();
        let word = MatchingMostOtherWordsInAtLeastOneOpenPosition.pick(&words, &[0, 1, 2, 3, 4]);
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
    fn test_get_hints_and_values() {
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
    }
}
