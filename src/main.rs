use std::env::args;

use wordle_helper::cache::{Cache, HintsBySecretByGuess, SolutionsByHintByGuess};
use wordle_helper::words::Language::English;
use wordle_helper::words::{Language, Words};
use wordle_helper::{autoplay_and_print_stats_with_language, FixedGuessList, Wordle};

fn main() {
    let args: Vec<String> = args().collect();
    let mut lang: Option<Language> = None;
    let mut consumed_args = 1;
    if args.len() > 1 {
        if let Ok(parsed_lang) = Language::try_from(args[1].as_str()) {
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
