use rand::prelude::*;
use std::collections::HashSet;
use std::io;

const WORDLIST: &str = include_str!("../wordlist.txt");

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
                    .map(|word| word.iter().collect::<String>())
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

    fn suggest_a_word(&mut self) -> Vec<char> {
        self.words
            .iter()
            .filter(|&word| !self.guessed_words.contains(word))
            .choose(&mut self.rng)
            .unwrap()
            .to_vec()
    }

    fn ask_about_correct_chars_in_correct_position(&mut self) {
        println!("Enter characters in the correct spot. Use _ as prefix if necessary.");
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
        println!("Enter correct characters in the wrong spot. Use _ as prefix if necessary");
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
            .collect();
    }

    fn print_result(&self) {
        if self.words.len() == 1 {
            println!(
                "\nThe only word left in the list is '{}'",
                self.words[0].iter().collect::<String>()
            );
        } else {
            let word: String = self.correct_chars.iter().map(|c| c.unwrap()).collect();
            println!("\nThe word is '{}'", word);
        }
    }
}

fn main() {
    Wordle::new(WORDLIST).play()
}
