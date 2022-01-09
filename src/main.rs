use rand::prelude::*;
use std::collections::{HashMap, HashSet};
use std::io;

const WORDLIST: &str = include_str!("../wordlist.txt");

// Helper for https://www.powerlanguage.co.uk/wordle/
struct Wordle {
    words: Vec<Vec<char>>,
    illegal_chars: HashSet<char>,
    correct_chars: HashMap<usize, char>,
    illegal_at_pos: [HashSet<char>; 5],
    must_have_chars: HashSet<char>,
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
            correct_chars: HashMap::new(),
            illegal_at_pos: illegal_position_for_char,
            must_have_chars: HashSet::new(),
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
        self.correct_chars.len() == 5 || self.words.len() <= 1
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
                    .map(|word| word.iter().collect::<String>())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }
    fn single_round(&mut self) {
        let guess = self.ask_for_guess();
        self.guessed_words.insert(guess.clone());
        let correct_pos = self.ask_about_correct_chars_in_correct_position();
        if self.correct_chars.len() == 5 {
            return;
        }
        let wrong_pos = self.ask_about_correct_chars_in_wrong_position();
        self.update_illegal_chars(guess, correct_pos, wrong_pos);

        self.update_words();
    }

    fn ask_for_guess(&mut self) -> Vec<char> {
        let suggestion = self
            .words
            .iter()
            .filter(|&word| !self.guessed_words.contains(word))
            .choose(&mut self.rng)
            .unwrap()
            .to_vec();
        println!(
            "Enter the word you guessed, or nothing if you use the random suggestion '{}':",
            suggestion.iter().collect::<String>()
        );
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        if !input.trim().is_empty() {
            input.trim().chars().collect()
        } else {
            suggestion
        }
    }

    fn ask_about_correct_chars_in_correct_position(&mut self) -> Vec<char> {
        println!("Enter characters in the correct spot. Use _ as prefix if necessary.");
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        let correct_pos: Vec<_> = input.trim().chars().collect();
        for (i, c) in correct_pos.iter().enumerate().filter(|(_, &c)| c != '_') {
            // println!("Inserting '{}' as correct character @ {}", c, i);
            self.correct_chars.insert(i, *c);
        }
        correct_pos
    }

    fn ask_about_correct_chars_in_wrong_position(&mut self) -> Vec<char> {
        let mut input = String::new();
        println!("Enter correct characters in the wrong spot. Use _ as prefix if necessary");
        io::stdin().read_line(&mut input).unwrap();

        let wrong_pos: Vec<_> = input.trim().chars().collect();
        for (i, c) in wrong_pos.iter().enumerate().filter(|(_, &c)| c != '_') {
            // println!("Inserting '{}' as wrong character @ {}", c, i);
            self.illegal_at_pos[i].insert(*c);
            self.must_have_chars.insert(*c);
        }
        wrong_pos
    }

    fn update_illegal_chars(
        &mut self,
        guess: Vec<char>,
        correct_pos: Vec<char>,
        wrong_pos: Vec<char>,
    ) {
        // println!("guess {:?}", guess);
        // println!("correct_pos {:?}", correct_pos);
        // println!("wrong_pos {:?}", wrong_pos);
        // println!("self.must_have_chars {:?}", self.must_have_chars);
        // println!("self.correct_chars {:?}", self.correct_chars);

        for c in guess.into_iter().filter(|c| {
            !correct_pos.contains(c)
                && !wrong_pos.contains(c)
                && !self.must_have_chars.contains(c)
                && !self.correct_chars.values().any(|correct| correct == c)
        }) {
            // println!("Inserting illegal char '{}'", c);
            self.illegal_chars.insert(c);
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
                self.must_have_chars
                    .iter()
                    .all(|mandatory| word.contains(mandatory))
            })
            .filter(|word| self.correct_chars.iter().all(|(i, c)| word[*i] == *c))
            .filter(|word| {
                !word
                    .iter()
                    .enumerate()
                    .any(|(i, c)| self.illegal_at_pos[i].contains(c))
            })
            .collect();
    }

    fn print_result(&self) {
        if self.words.len() == 1 {
            println!(
                "\nThe only word left in the list is '{}'",
                self.words[0].iter().collect::<String>()
            );
        } else {
            let mut word: Vec<_> = self.correct_chars.iter().collect();
            word.sort_unstable_by_key(|(i, _)| *i);
            let word: String = word.into_iter().map(|(_, c)| c).collect();
            println!("\nThe word is '{}'", word);
        }
    }
}

fn main() {
    Wordle::new(WORDLIST).play()
}
