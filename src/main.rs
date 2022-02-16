use std::collections::BTreeMap;
use std::env::args;

use wordle_helper::words::Language::English;
use wordle_helper::words::{Language, Words};
use wordle_helper::{
    Attempt, Cache, ChainedStrategies, Count, FirstOfTwoOrFewerRemainingSolutions, FixedGuessList,
    HintsBySecretByGuess, PickFirstSolution, SolutionsByHintByGuess, TryToPickWord,
    WordWithMostNewCharsFromRemainingSolutions, Wordle, MAX_ATTEMPTS,
};

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

fn autoplay_and_print_stats_with_language<S: TryToPickWord + Sync>(strategy: S, lang: Language) {
    let words = Words::new(lang);
    let hsg = HintsBySecretByGuess::of(&words);
    let shg = SolutionsByHintByGuess::of(&words, &hsg);
    let cache = Cache::new(&words, &hsg, &shg);

    let mut secrets: Vec<_> = words
        .secrets()
        .iter()
        // .filter(|w| w.to_string().eq("'rowdy'"))
        .collect();
    secrets.sort_unstable();
    let attempts: Vec<usize> = secrets
        .iter()
        .map(|secret| {
            let mut game = Wordle::with(&words, &cache);
            let strategy = ChainedStrategies::new(
                vec![
                    &FirstOfTwoOrFewerRemainingSolutions,
                    &WordWithMostNewCharsFromRemainingSolutions,
                    &strategy,
                ],
                PickFirstSolution,
            );
            game.autoplay(secret, strategy);
            game.guessed.len()
        })
        .collect();
    let mut count_by_attempts: BTreeMap<Attempt, Count> = BTreeMap::new();
    for attempt in attempts {
        *count_by_attempts.entry(attempt).or_default() += 1;
    }
    print_stats(count_by_attempts.iter());
}

fn print_stats<'a>(count_by_attempts: impl Iterator<Item = (&'a Attempt, &'a Count)>) {
    let mut games = 0;
    let mut attempts_sum = 0;
    let mut failures = 0;
    let mut descs = vec![];
    for (attempts, count) in count_by_attempts {
        games += count;
        attempts_sum += attempts * count;
        if attempts > &MAX_ATTEMPTS {
            failures += count;
        }
        descs.push(format!("{}: {}", attempts, count));
    }
    let average = attempts_sum as f64 / games as f64;

    print!("\n{:.3} average attempts; {}", average, descs.join(", "));
    if failures > 0 {
        let percent_failed = 100.0 * failures as f64 / games as f64;
        println!("; {} ({:.2}%) failures", failures, percent_failed)
    } else {
        println!();
    }
}
