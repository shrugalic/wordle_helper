use std::env::args;

use wordle_helper::words::Language;
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
    let lang = lang.unwrap_or(Language::English);
    println!(
        "Language: {}. Choices: English, NYTimes, At, Ch, De, Uber, Primal.",
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
        let mut game = Wordle::with(lang);
        game.play();
    }
}
