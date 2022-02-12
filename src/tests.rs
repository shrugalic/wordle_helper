use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use super::*;

const MAX_ATTEMPTS: usize = 6;

#[ignore] // ~30s for all guesses, ~6s for solutions only
#[test]
fn compare_best_word_strategies() {
    let words = Words::new(English);

    // Solutions only:
    // 484.89 roate, 490.38 raise, 493.53 raile, 502.77 soare, 516.34 arise,
    // 516.85 irate, 517.91 orate, 531.22 ariel, 538.21 arose, 548.07 raine
    // Guesses:
    // 12563.92 lares, 12743.70 rales, 13298.11 tares, 13369.60 soare, 13419.26 reais
    // 13461.31 nares, 13684.70 aeros, 13771.28 rates, 13951.64 arles, 13972.96 serai
    let variances = variance_of_remaining_words(&words);
    println!("Best (lowest) variance:\n{}", variances.to_string(5));
    assert_eq!(variances.len(), words.guesses.len());

    // panic!(); // ~2s

    // Solutions only:
    // 60.42 roate, 61.00 raise, 61.33 raile, 62.30 soare, 63.73 arise,
    // 63.78 irate, 63.89 orate, 65.29 ariel, 66.02 arose, 67.06 raine
    // Guesses:
    // 288.74 lares, 292.11 rales, 302.49 tares, 303.83 soare, 304.76 reais
    // 305.55 nares, 309.73 aeros, 311.36 rates, 314.73 arles, 315.13 serai
    let remaining = expected_remaining_solution_counts(&words);
    println!(
        "Best (lowest) expected remaining solution count:\n{}",
        remaining.to_string(5)
    );
    assert_eq!(remaining.len(), words.guesses.len());

    // panic!(); // ~5s

    // Solutions only:
    // 139883 roate, 141217 raise, 141981 raile, 144227 soare, 147525 arise
    // All guesses:
    // Best (lowest) average remaining solution count:
    // 3745512 lares, 3789200 rales, 3923922 tares, 3941294 soare, 3953360 reais
    // 3963578 nares, 4017862 aeros, 4038902 rates, 4082728 arles, 4087910 serai

    let hsg = HintsBySecretByGuess::of(&words);
    let shg = SolutionsByHintByGuess::of(&words, &hsg);
    let cache = Cache::new(&words, &hsg, &shg);
    let solutions = words.secrets.iter().collect();
    let totals = fewest_remaining_solutions(&words, &solutions, &Vec::new(), &cache);
    println!(
        "Best (lowest) average remaining solution count:\n{}",
        totals.to_string(5)
    );
    assert_eq!(totals.len(), words.guesses.len());

    // panic!(); // ~8s
    println!("\ndifferences between fewest total and expected remaining:");
    let (mut different, mut same) = (0, 0);
    for (i, ((low_w, count), (rem_w, remaining))) in totals.iter().zip(remaining.iter()).enumerate()
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

    println!("differences between fewest total and lowest variance:");
    let (mut different, mut same) = (0, 0);
    for (i, ((low_w, count), (var_w, variance))) in totals.iter().zip(variances.iter()).enumerate()
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

// Previously used method, slightly less stable than lowest_total_number_of_remaining_solutions
fn variance_of_remaining_words(words: &Words) -> Vec<(&Guess, f64)> {
    let average = words.secrets.len() as f64 / 243.0;
    let mut scores: Vec<_> = words
        .guesses
        .par_iter()
        .map(|guess| {
            let count_by_hint = count_by_hint(guess, &words.secrets);
            let variance = count_by_hint
                .into_iter()
                .map(|count| (count as f64 - average).powf(2.0))
                .sum::<f64>() as f64
                / 243.0;
            (guess, variance)
        })
        .collect();
    scores.sort_asc();
    scores
}

// Previously used method
fn expected_remaining_solution_counts(words: &Words) -> Vec<(&Guess, f64)> {
    let total_solutions = words.secrets.len() as f64;
    let mut scores: Vec<_> = words
        .guesses
        .par_iter()
        .map(|guess| {
            let count_by_hint = count_by_hint(guess, &words.secrets);
            let expected_remaining_word_count = count_by_hint
                .into_iter()
                .map(|count| count.pow(2))
                .sum::<usize>() as f64
                / total_solutions;
            (guess, expected_remaining_word_count)
        })
        .collect();
    scores.sort_asc();
    scores
}

// Allow because determine_hints expects &Guess not &[char]
fn count_by_hint(
    #[allow(clippy::ptr_arg)] guess: &Guess,
    solutions: &BTreeSet<Secret>,
) -> [usize; 243] {
    let mut count_by_hint = [0; 243];
    for solution in solutions.iter() {
        let hint = guess.calculate_hint(solution);
        count_by_hint[hint.value() as usize] += 1;
    }
    count_by_hint
}

#[ignore] // ~15s English or ~3s German
#[test]
fn test_print_full_guess_tree() {
    print_full_guess_tree(English);
}
fn print_full_guess_tree(lang: Language) {
    let words = Words::new(lang);
    let secrets: Solutions = words.secrets.iter().collect();
    let hsg = HintsBySecretByGuess::of(&words);
    let shg = SolutionsByHintByGuess::of(&words, &hsg);
    let cache = Cache::new(&words, &hsg, &shg);

    let guessed = [];
    explore_tree(&words, &secrets, &guessed, &cache);
}
#[ignore]
#[test]
fn test_print_partial_guess_tree() {
    print_partial_guess_tree();
}
fn print_partial_guess_tree() {
    let lang = English;
    let words = Words::new(lang);
    let hsg = HintsBySecretByGuess::of(&words);
    let shg = SolutionsByHintByGuess::of(&words, &hsg);
    let cache = Cache::new(&words, &hsg, &shg);

    let secret = "piper".to_word();
    let guess1 = "roate".to_word(); // hint "🟨⬛⬛⬛🟨"
    let guess2 = "feued".to_word(); // hint "⬛⬛⬛🟩⬛"

    let secrets1 = &cache.secret_solutions.by_secret_by_guess[&guess1][&secret];
    println!("{} roate secrets {}", secrets1.len(), secrets1.to_string());
    let secrets2 = &cache.secret_solutions.by_secret_by_guess[&guess2][&secret];
    println!("{} feued secrets {}", secrets2.len(), secrets2.to_string());
    let secrets: Solutions = secrets1.intersect(secrets2);
    println!(
        "{} intersected secrets {}",
        secrets.len(),
        secrets.to_string()
    );

    let guessed = [&guess1, &guess2];
    explore_tree(&words, &secrets, &guessed, &cache);
}
fn explore_tree(words: &Words, secrets: &Solutions, guessed: &[&Word], cache: &Cache) {
    if guessed.len() == MAX_ATTEMPTS {
        println!(
            "            7. Still not found after 6 guesses {}. Secrets: {}",
            guessed.to_string(),
            secrets.to_string()
        );
        return;
    } else if secrets.len() <= 2 {
        // 1 and 2 already printed info on how to proceed
        return;
    }
    let scores = fewest_remaining_solutions(words, secrets, guessed, cache);
    let guess = scores.lowest().unwrap();
    let mut guessed = guessed.to_vec();
    guessed.push(&guess);

    let mut pairs: Vec<_> = cache.hint_solutions.by_hint_by_guess[&guess]
        .iter()
        .map(|(hint, solutions)| (hint, solutions.intersect(secrets)))
        .filter(|(_, solutions)| !solutions.is_empty())
        .collect();
    pairs.sort_unstable_by(|(v1, s1), (v2, s2)| match s1.len().cmp(&s2.len()) {
        Ordering::Equal => v1.cmp(v2), // lower hint-value (more unknown) first
        fewer_elements_first => fewer_elements_first,
    });
    for (hint, secrets) in pairs {
        print_info(&guessed, *hint, &secrets);
        explore_tree(words, &secrets, &guessed, cache)
    }
}
fn print_info(guessed: &[&Guess], hint: HintValue, secrets: &Solutions) {
    let turn = guessed.len();
    let indent = "\t".repeat(turn - 1);
    let guess = guessed.last().unwrap().to_string();
    let first = secrets.iter().next().unwrap().to_string();
    print!(
        "{}{}. guess {} + hint {} matches ",
        indent,
        turn,
        guess,
        Hints::from(hint)
    );
    if secrets.len() == 1 {
        println!("{}, use it as {}. guess.", first, turn + 1);
    } else if secrets.len() == 2 {
        let second = secrets.iter().nth(1).unwrap().to_string();
        println!(
            "{} and {}. Pick one at random to win by the {}. guess.",
            first,
            second,
            turn + 2
        );
    } else if secrets.len() <= 5 {
        println!("{} secrets {}.", secrets.len(), secrets.to_string());
    } else {
        println!("{} secrets, for example {}.", secrets.len(), first);
    }
}

#[ignore] // < 2s
#[test]
fn count_solutions_by_hint_by_guess() {
    let words = Words::new(English);
    let hsg = HintsBySecretByGuess::of(&words);
    let solutions = SolutionsByHintByGuess::of(&words, &hsg);
    assert_eq!(
        solutions
            .by_hint_by_guess
            .iter()
            .map(|(_, s)| s.len())
            .sum::<usize>(),
        1_120_540 // 1120540
    );
}

#[ignore]
#[test]
// 0. total time [ms]: 1080 cache initialization
// 1. total time [ms]: 1416 cached,  883 w/o cache
// 2. total time [ms]: 1772 cached, 1818 w/o cache <- break even
// 3. total time [ms]: 2121 cached, 2765 w/o cache <- break even
// 4. total time [ms]: 2473 cached, 3722 w/o cache <- break even
fn does_hint_cache_help_part_1_without_cache() {
    let words = Words::new(English);

    let start = Instant::now();
    let hints = HintsBySecretByGuess::of(&words);
    let mut t_cached = start.elapsed();
    println!(
        "0. total time [ms]: {:4} cache initialization",
        t_cached.as_millis(),
    );

    let sum_with_cache = |words: &Words| -> usize {
        words
            .guesses
            .par_iter()
            .map(|guess| {
                words
                    .secrets
                    .iter()
                    .map(|secret| hints.by_secret_by_guess[guess][secret] as usize)
                    .sum::<usize>()
            })
            .sum()
    };

    let sum_without_cache = |words: &Words| -> usize {
        words
            .guesses
            .par_iter()
            .map(|guess| {
                words
                    .secrets
                    .iter()
                    .map(|secret| hints.by_secret_by_guess[guess][secret] as usize)
                    .sum::<usize>()
            })
            .sum()
    };

    let mut t_uncached = Duration::default();
    for i in 1..5 {
        let start = Instant::now();
        let sum = sum_without_cache(&words);
        assert_eq!(1_056_428_862, sum); // 1056428862
        t_uncached += start.elapsed();

        let start = Instant::now();
        let sum = sum_with_cache(&words);
        assert_eq!(1_056_428_862, sum); // 1056428862
        t_cached += start.elapsed();

        println!(
            "{}. total time [ms]: {:4} cached, {:4} w/o cache{}",
            i,
            t_cached.as_millis(),
            t_uncached.as_millis(),
            if t_cached <= t_uncached {
                " <- break even"
            } else {
                ""
            }
        );
    }
}

#[ignore] // 2-4s
#[test]
fn count_solutions_by_secret_by_guess() {
    let words = Words::new(English);
    let hsg = HintsBySecretByGuess::of(&words);
    let shg = SolutionsByHintByGuess::of(&words, &hsg);
    let ssg = SolutionsBySecretByGuess::of(&words, &hsg, &shg);

    assert_eq!(
        ssg.by_secret_by_guess
            .iter()
            .map(|(_, s)| s.len())
            .sum::<usize>(),
        words.secrets.len() * words.guesses.len() // 2_315 * 12_972 = 30_030_180 = 2315 * 12972 = 30030180
    );
}

#[ignore] // ~3s
#[test]
// Top 5: 60.42 'roate', 61.00 'raise', 61.33 'raile', 62.30 'soare', 63.73 'arise'
fn find_optimal_first_word_english() {
    let lang = English;
    let words = Words::new(lang);
    let hsg = HintsBySecretByGuess::of(&words);
    let shg = SolutionsByHintByGuess::of(&words, &hsg);
    let cache = Cache::new(&words, &hsg, &shg);
    let game = Wordle::with(&words, &cache);

    let scores = fewest_remaining_solutions_for_game(&game);
    println!("scores {}", scores.to_string(5));
    let optimal = scores.lowest().unwrap();
    assert_eq!("roate".to_word(), optimal);
}

#[ignore] // ~1s
#[test]
// Top 5: 31.30 'raine', 35.26 'taler', 36.21 'raten', 36.26 'laser', 36.63 'reale'
fn find_optimal_first_word_german() {
    let lang = German;
    let words = Words::new(lang);
    let hsg = HintsBySecretByGuess::of(&words);
    let shg = SolutionsByHintByGuess::of(&words, &hsg);
    let cache = Cache::new(&words, &hsg, &shg);
    let game = Wordle::with(&words, &cache);

    let scores = fewest_remaining_solutions_for_game(&game);
    println!("scores {}", scores.to_string(5));
    let optimal = scores.lowest().unwrap();
    assert_eq!("raine".to_word(), optimal);
}

#[ignore]
#[test]
// ~10s (i9) or ~13s (M1) or 6.5s (M1 Max) for 5 single German words
// ~1min 51s (i9) or ~2min 21s (M1) or 67s (M1 Max) for 5 single English words
//
// Deutsch:
// Best 1. guesses: 31.30 'raine', 35.26 'taler', 36.21 'raten', 36.26 'laser', 36.63 'reale'
// Best 2. guesses after 1. 'raine': 3.25 'holst', 3.32 'kults', 3.34 'lotus', 3.39 'stuhl', 3.52 'buhlt'
// Best 3. guesses after 1. 'raine' and 2. 'holst': 1.40 'dumpf', 1.43 'umgab', 1.46 'umweg', 1.49 'bekam', 1.50 'bezug'
// Best 4. guesses after 1. 'raine' and 2. 'holst' and 3. 'dumpf': 1.06 'biwak', 1.08 'abweg', 1.09 'bezog', 1.09 'bezug', 1.09 'beeck'
// Best 5. guesses after 1. 'raine' and 2. 'holst' and 3. 'dumpf' and 4. 'biwak': 1.01 'legen', 1.01 'leger', 1.01 'engen', 1.01 'enzen', 1.01 'genen'
//
// English
// Best 1. guesses: 60.42 'roate', 61.00 'raise', 61.33 'raile', 62.30 'soare', 63.73 'arise'
// Best 2. guesses after 1. 'roate': 5.12 'linds', 5.16 'sling', 5.20 'clips', 5.29 'limns', 5.33 'blins'
// Best 3. guesses after 1. 'roate' and 2. 'linds': 1.64 'chump', 1.69 'bumph', 1.78 'crump', 1.80 'clump', 1.80 'bumpy'
// Best 4. guesses after 1. 'roate' and 2. 'linds' and 3. 'chump': 1.15 'gleby', 1.15 'gawky', 1.16 'gybed', 1.16 'befog', 1.16 'bogey'
// Best 5. guesses after 1. 'roate' and 2. 'linds' and 3. 'chump' and 4. 'gleby': 1.04 'wakfs', 1.04 'waift', 1.05 'swift', 1.05 'fatwa', 1.05 'fawns'
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
    let lang = English;
    let words = Words::new(lang);
    let hsg = HintsBySecretByGuess::of(&words);
    let shg = SolutionsByHintByGuess::of(&words, &hsg);
    let cache = Cache::new(&words, &hsg, &shg);
    let game = Wordle::with(&words, &cache);

    let mut scores = fewest_remaining_solutions_for_game(&game);
    println!("Best 1. guesses: {}", scores.to_string(5));
    scores.sort_asc();

    let top_pick_count = 1;
    for (guess1, _) in scores.into_iter().take(top_pick_count) {
        let guessed = [guess1];
        let scores = find_best_next_guesses(&game, &guessed);

        println!(
            "Best 2. guesses after 1. {}: {}",
            guess1.to_string(),
            scores.to_string(5)
        );

        for (guess2, _) in scores.into_iter().take(top_pick_count) {
            let guessed = [guess1, guess2];
            let scores = find_best_next_guesses(&game, &guessed);
            println!(
                "Best 3. guesses after 1. {} and 2. {}: {}",
                guess1.to_string(),
                guess2.to_string(),
                scores.to_string(5)
            );

            for (guess3, _) in scores.into_iter().take(top_pick_count) {
                let guessed = [guess1, guess2, guess3];
                let scores = find_best_next_guesses(&game, &guessed);
                println!(
                    "Best 4. guesses after 1. {} and 2. {} and 3. {}: {}",
                    guess1.to_string(),
                    guess2.to_string(),
                    guess3.to_string(),
                    scores.to_string(5)
                );

                for (guess4, _) in scores.into_iter().take(top_pick_count) {
                    let guessed = [guess1, guess2, guess3, guess4];
                    let scores = find_best_next_guesses(&game, &guessed);
                    println!(
                        "Best 5. guesses after 1. {} and 2. {} and 3. {} and 4. {}: {}",
                        guess1.to_string(),
                        guess2.to_string(),
                        guess3.to_string(),
                        guess4.to_string(),
                        scores.to_string(5)
                    );
                }
            }
        }
    }
}

fn find_best_next_guesses<'g>(game: &'g Wordle, guessed: &[&Guess]) -> Vec<(&'g Guess, f64)> {
    let first = *guessed.iter().next().unwrap();
    let hsg = HintsBySecretByGuess::of(game.words);
    let shg = SolutionsByHintByGuess::of(game.words, &hsg);
    let ssg = SolutionsBySecretByGuess::of(game.words, &hsg, &shg);

    let mut scores: Vec<_> = game
        .allowed()
        .into_par_iter()
        .filter(|next| !guessed.contains(next))
        .map(|next| {
            let len = game.solutions.len() as f64;
            let count: usize = game
                .solutions
                .iter()
                .map(|&secret| {
                    let solutions1 = &ssg.by_secret_by_guess[first][secret];
                    let solutions2 = &ssg.by_secret_by_guess[next][secret];

                    // apply first and next guess
                    let mut solutions: Solutions = solutions1.intersect(solutions2);

                    // Apply other previous guesses
                    for other in guessed.iter().skip(1).cloned() {
                        let solutions3 = ssg.by_secret_by_guess[other][secret];
                        solutions = solutions.intersect(solutions3);
                    }
                    solutions.len()
                })
                .sum();
            (next, count as f64 / len)
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

#[ignore] // ~26 min (i9) or ~39 min (M1) or 21.7min (M1 Max)
#[test]
// 3.55 average attempts; 2: 40, 3: 999, 4: 1234, 5: 42
fn auto_play_word_that_results_in_fewest_remaining_solutions() {
    autoplay_and_print_stats(WordThatResultsInFewestRemainingSolutions);
}

#[ignore] // 1min 38s (i9) or 1min 53s (M1) or 74s (M1 Max)
#[test]
// 3.36 average attempts; 2: 42, 3: 685, 4: 429, 5: 15
fn auto_play_word_that_results_in_fewest_remaining_solutions_german() {
    autoplay_and_print_stats_with_language(WordThatResultsInFewestRemainingSolutions, German);
}

#[ignore]
#[test]
// 4.071 average attempts; 2: 41, 3: 438, 4: 1185, 5: 617, 6: 34
fn auto_play_tubes_fling_champ_wordy_every_time() {
    let strategy = FixedGuessList::new(vec!["tubes", "fling", "champ", "wordy"]);
    autoplay_and_print_stats(strategy);
}

#[ignore]
#[test]
// 4.332 average attempts; 2: 27, 3: 364, 4: 944, 5: 788, 6: 178, 7: 14; 14 (0.60%) failures
fn auto_play_crwth_faxed_vulgo_zinky_jambs_every_time() {
    let strategy = FixedGuessList::new(vec!["crwth", "faxed", "vulgo", "zinky", "jambs"]);
    autoplay_and_print_stats(strategy);
}

#[ignore]
#[test]
// 3.821 average attempts; 1: 1, 2: 48, 3: 710, 4: 1201, 5: 317, 6: 37, 7: 1; 1 (0.04%) failures
fn auto_play_crane_sloth_pudgy_every_time() {
    let strategy = FixedGuessList::new(vec!["crane", "sloth", "pudgy"]);
    autoplay_and_print_stats(strategy);
}

#[ignore]
#[test]
// 4.107 average attempts; 1: 1, 2: 31, 3: 429, 4: 1164, 5: 641, 6: 47, 7: 2; 2 (0.09%) failures
fn auto_play_spade_lucky_brown_fight_every_time() {
    let strategy = FixedGuessList::new(vec!["spade", "lucky", "brown", "fight"]);
    autoplay_and_print_stats(strategy);
}

#[ignore]
#[test]
// 3.982 average attempts; 1: 1, 2: 52, 3: 537, 4: 1194, 5: 466, 6: 58, 7: 7; 7 (0.30%) failures
fn auto_play_stale_dough_brink_every_time() {
    let strategy = FixedGuessList::new(vec!["stale", "dough", "brink"]);
    autoplay_and_print_stats(strategy);
}

#[ignore]
#[test]
// 4.549 average attempts; 1: 1, 2: 19, 3: 258, 4: 821, 5: 889, 6: 301, 7: 26; 26 (1.12%) failures
fn auto_play_fjord_waltz_nymph_quick_vexes_every_time() {
    let strategy = FixedGuessList::new(vec!["fjord", "waltz", "nymph", "quick", "vexes"]);
    autoplay_and_print_stats(strategy);
}

#[ignore]
#[test]
// 4.495 average attempts; 1: 1, 2: 19, 3: 257, 4: 828, 5: 988, 6: 210, 7: 12; 12 (0.52%) failures
fn auto_play_fjord_waltz_psych_imbue_every_time() {
    let strategy = FixedGuessList::new(vec!["fjord", "waltz", "psych", "imbue"]);
    autoplay_and_print_stats(strategy);
}

#[ignore]
#[test]
// 4.599 average attempts; 1: 1, 2: 32, 3: 198, 4: 683, 5: 1159, 6: 234, 7: 8; 8 (0.35%) failures
fn auto_play_glyph_jocks_fixed_brawn_every_time() {
    let strategy = FixedGuessList::new(vec!["glyph", "jocks", "fixed", "brawn"]);
    autoplay_and_print_stats(strategy);
}

#[ignore]
#[test]
// 4.259 average attempts; 2: 30, 3: 457, 4: 924, 5: 713, 6: 169, 7: 22; 22 (0.95%) failures
fn auto_play_glent_brick_jumpy_vozhd_waqfs_every_time() {
    let strategy = FixedGuessList::new(vec!["glent", "brick", "jumpy", "vozhd", "waqfs"]);
    autoplay_and_print_stats(strategy);
}

#[ignore]
#[test]
// 4.256 average attempts; 2: 30, 3: 457, 4: 924, 5: 715, 6: 172, 7: 17; 17 (0.73%) failures
fn auto_play_glent_brick_jumpy_vozhd_every_time() {
    let strategy = FixedGuessList::new(vec!["glent", "brick", "jumpy", "vozhd"]);
    autoplay_and_print_stats(strategy);
}

#[ignore]
#[test]
// Average attempts = 3.829; 7 (0.302%) failed games (> 6 attempts):
// 2: 57, 3: 770, 4: 1077, 5: 342, 6: 62, 7: 6, 8: 1
fn auto_play_roate_linds_chump_gawky_befit() {
    let strategy = FixedGuessList::new(vec!["roate", "linds", "chump", "gawky", "befit"]);
    autoplay_and_print_stats(strategy);
}

#[ignore]
#[test]
// Average attempts = 3.816; 2 (0.086%) failed games (> 6 attempts):
// 2: 47, 3: 796, 4: 1065, 5: 353, 6: 52, 7: 2
fn auto_play_roate_linds_chump_gawky() {
    let strategy = FixedGuessList::new(vec!["roate", "linds", "chump", "gawky"]);
    autoplay_and_print_stats(strategy);
}

#[ignore]
#[test]
// Average attempts = 3.807; 5 (0.216%) failed games (> 6 attempts):
// 2: 62, 3: 789, 4: 1073, 5: 321, 6: 65, 7: 4, 8: 1
fn auto_play_roate_linds_chump_gleby_wakfs() {
    let strategy = FixedGuessList::new(vec!["roate", "linds", "chump", "gleby", "wakfs"]);
    autoplay_and_print_stats(strategy);
}

#[ignore]
#[test]
// Average attempts = 3.809; 4 (0.173%) failed games (> 6 attempts):
// 2: 65, 3: 769, 4: 1085, 5: 340, 6: 52, 7: 4
fn auto_play_roate_linds_chump_gleby() {
    let strategy = FixedGuessList::new(vec!["roate", "linds", "chump", "gleby"]);
    autoplay_and_print_stats(strategy);
}

#[ignore]
#[test]
// Average attempts = 3.564; 0 (0.000%) failed games (> 6 attempts):
// 2: 41, 3: 530, 4: 502, 5: 94, 6: 4
fn auto_play_german_raine_holst_dumpf_biwak_legen() {
    let strategy = FixedGuessList::new(vec!["raine", "holst", "dumpf", "biwak", "legen"]);
    autoplay_and_print_stats_with_language(strategy, German);
}

#[ignore]
#[test]
// Average attempts = 3.564; 0 (0.000%) failed games (> 6 attempts):
// 2: 41, 3: 530, 4: 502, 5: 94, 6: 4
fn auto_play_german_raine_holst_dumpf_biwak() {
    let strategy = FixedGuessList::new(vec!["raine", "holst", "dumpf", "biwak"]);
    autoplay_and_print_stats_with_language(strategy, German);
}

#[ignore]
#[test]
// Average attempts = 3.749; 9 (0.546%) failed games (> 6 attempts):
// 2: 82, 3: 649, 4: 610, 5: 229, 6: 70, 7: 6, 8: 3
fn auto_play_german_tarne_helis_gudok_zamba_fiept() {
    let strategy = FixedGuessList::new(vec!["tarne", "helis", "gudok", "zamba", "fiept"]);
    autoplay_and_print_stats_with_language(strategy, German);
}

#[ignore]
#[test]
// Average attempts = 3.763; 7 (0.424%) failed games (> 6 attempts):
// 2: 79, 3: 623, 4: 631, 5: 249, 6: 60, 7: 6, 8: 1
fn auto_play_german_tarne_helis_gudok_zamba() {
    let strategy = FixedGuessList::new(vec!["tarne", "helis", "gudok", "zamba"]);
    autoplay_and_print_stats_with_language(strategy, German);
}

#[ignore]
#[test]
// Average attempts = 3.994; 21 (0.907%) failed games (> 6 attempts):
// 2: 53, 3: 738, 4: 873, 5: 496, 6: 134, 7: 19, 8: 2
fn auto_play_soare_until_pygmy_whack_every_time() {
    let strategy = FixedGuessList::new(vec!["soare", "until", "pygmy", "whack"]);
    autoplay_and_print_stats(strategy);
}

#[ignore]
#[test]
// Average attempts = 4.879; 219 (9.460%) failed games (> 6 attempts):
// 1: 1, 2: 18, 3: 317, 4: 592, 5: 622, 6: 546, 7: 200, 8: 18, 9: 1
fn auto_play_quick_brown_foxed_jumps_glazy_vetch_every_time() {
    let strategy = FixedGuessList::new(vec!["quick", "brown", "foxed", "jumps", "glazy", "vetch"]);
    autoplay_and_print_stats(strategy);
}

#[ignore]
#[test]
// Average attempts = 4.024; 2 (0.086%) failed games (> 6 attempts):
// 1: 1, 2: 51, 3: 500, 4: 1177, 5: 513, 6: 71, 7: 2
fn auto_play_fixed_guess_list_1() {
    let strategy = FixedGuessList::new(vec!["brake", "dying", "clots", "whump"]);
    autoplay_and_print_stats(strategy);
}

#[ignore]
#[test]
// Average attempts = 3.910; 1 (0.043%) failed games (> 6 attempts):
// 1: 1, 2: 64, 3: 636, 4: 1114, 5: 443, 6: 56, 7: 1
fn auto_play_fixed_guess_list_2() {
    let strategy = FixedGuessList::new(vec!["maple", "sight", "frown", "ducky"]);
    autoplay_and_print_stats(strategy);
}

#[ignore]
#[test]
// Average attempts = 4.034; 5 (0.216%) failed games (> 6 attempts):
// 1: 1, 2: 49, 3: 550, 4: 1081, 5: 544, 6: 85, 7: 5
fn auto_play_fixed_guess_list_3() {
    let strategy = FixedGuessList::new(vec!["fiend", "paths", "crumb", "glows"]);
    autoplay_and_print_stats(strategy);
}

#[ignore]
#[test]
// Average attempts = 3.785; 11 (0.475%) failed games (> 6 attempts):
// 2: 67, 3: 854, 4: 974, 5: 364, 6: 45, 7: 7, 8: 4
fn auto_play_fixed_guess_list_4() {
    let strategy = FixedGuessList::new(vec!["reals", "point", "ducky"]);
    autoplay_and_print_stats(strategy);
}

#[ignore]
#[test]
// Average attempts = 3.831; 8 (0.346%) failed games (> 6 attempts):
// 2: 72, 3: 725, 4: 1113, 5: 344, 6: 53, 7: 6, 8: 1, 9: 1
fn auto_play_fixed_guess_list_5() {
    let strategy = FixedGuessList::new(vec!["laser", "pitch", "mound"]);
    autoplay_and_print_stats(strategy);
}

#[ignore]
#[test]
// Average attempts = 3.743; 23 (0.994%) failed games (> 6 attempts):
// 1: 1, 2: 135, 3: 853, 4: 924, 5: 304, 6: 75, 7: 16, 8: 5, 9: 2
fn auto_play_most_frequent_global_characters() {
    autoplay_and_print_stats(MostFrequentGlobalCharacter);
}

#[ignore]
#[test]
// Average attempts = 3.715; 26 (1.123%) failed games (> 6 attempts):
// 1: 1, 2: 130, 3: 919, 4: 888, 5: 268, 6: 83, 7: 18, 8: 7, 10: 1
fn auto_play_most_frequent_global_characters_high_variety_word() {
    autoplay_and_print_stats(MostFrequentGlobalCharacterHighVarietyWord);
}

#[ignore]
#[test]
// Average attempts = 3.778; 22 (0.950%) failed games (> 6 attempts):
// 1: 1, 2: 148, 3: 773, 4: 955, 5: 343, 6: 73, 7: 19, 8: 2, 9: 1
fn auto_play_most_frequent_characters_per_pos() {
    autoplay_and_print_stats(MostFrequentCharacterPerPos);
}

#[ignore]
#[test]
// Average attempts = 3.667; 19 (0.821%) failed games (> 6 attempts):
// 1: 1, 2: 148, 3: 903, 4: 929, 5: 263, 6: 52, 7: 15, 8: 2, 9: 2
fn auto_play_most_frequent_characters_per_pos_high_variety_word() {
    autoplay_and_print_stats(MostFrequentCharacterPerPosHighVarietyWord);
}

#[ignore]
#[test]
// Average attempts = 3.951; 54 (2.333%) failed games (> 6 attempts):
// 2: 50, 3: 829, 4: 873, 5: 390, 6: 119, 7: 34, 8: 15, 9: 5
fn auto_play_most_frequent_characters_of_words() {
    autoplay_and_print_stats(MostFrequentCharactersOfRemainingWords);
}

#[ignore]
#[test]
// Average attempts = 3.861; 32 (1.382%) failed games (> 6 attempts):
// 2: 78, 3: 813, 4: 932, 5: 380, 6: 80, 7: 22, 8: 8, 9: 2
fn auto_play_most_frequent_unused_characters() {
    let words = Words::new(English);
    autoplay_and_print_stats(MostFrequentUnusedCharacters::new(&words.guesses));
}
struct MostFrequentUnusedCharacters<'w> {
    combined_global_char_count_sums_by: HashMap<&'w Word, usize>,
}
impl<'w> MostFrequentUnusedCharacters<'w> {
    #[allow(clippy::ptr_arg)] // for global_character_counts_in defined for Vec<Guess> not &[Guess]
    fn new(guesses: &'w Vec<Guess>) -> Self {
        let global_count_by_char: HashMap<char, usize> =
            guesses.global_character_counts_in(&ALL_POS);
        let combined_global_char_count_sums_by: HashMap<_, usize> = guesses
            .par_iter()
            .map(|word| {
                let count = word
                    .unique_chars_in(&ALL_POS)
                    .iter()
                    .map(|c| global_count_by_char[c])
                    .sum::<usize>();
                (word, count)
            })
            .collect();
        MostFrequentUnusedCharacters {
            combined_global_char_count_sums_by,
        }
    }
}
impl<'w> TryToPickWord for MostFrequentUnusedCharacters<'w> {
    fn pick(&self, game: &Wordle) -> Option<Guess> {
        if game.solutions.len() < 10 {
            return None;
        };
        let words_with_most_new_chars: Vec<&Word> =
            words_with_most_new_chars(&game.guessed_chars(), game.allowed())
                .into_iter()
                .map(|(_, word)| word)
                .collect();

        let mut scores: Vec<_> = words_with_most_new_chars
            .iter()
            .map(|&word| {
                let count = self.combined_global_char_count_sums_by[word];
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
fn words_with_most_new_chars<'w>(
    used_chars: &HashSet<char>,
    words: Vec<&'w Word>,
) -> Vec<(Vec<char>, &'w Word)> {
    let new: Vec<(Vec<char>, &Word)> = words
        .into_iter()
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

#[ignore]
#[test]
// Average attempts = 4.004; 40 (1.728%) failed games (> 6 attempts):
// 1: 1, 2: 126, 3: 632, 4: 887, 5: 495, 6: 134, 7: 29, 8: 8, 9: 3
fn auto_play_most_other_words_in_at_least_one_open_position() {
    autoplay_and_print_stats(MatchingMostOtherWordsInAtLeastOneOpenPosition);
}

#[ignore]
#[test]
// Average attempts = 3.794; 29 (1.253%) failed games (> 6 attempts):
// 1: 1, 2: 114, 3: 856, 4: 886, 5: 341, 6: 88, 7: 23, 8: 6
fn auto_play_most_other_words_in_at_least_one_open_position_high_variety_word() {
    autoplay_and_print_stats(MatchingMostOtherWordsInAtLeastOneOpenPositionHighVarietyWord);
}

fn autoplay_and_print_stats<S: TryToPickWord + Sync>(strategy: S) {
    autoplay_and_print_stats_with_language(strategy, English);
}
fn autoplay_and_print_stats_with_language<S: TryToPickWord + Sync>(strategy: S, lang: Language) {
    let words = Words::new(lang);
    let hsg = HintsBySecretByGuess::of(&words);
    let shg = SolutionsByHintByGuess::of(&words, &hsg);
    let cache = Cache::new(&words, &hsg, &shg);

    let mut secrets: Vec<_> = words
        .secrets
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

/// Sum of attempts * count
fn attempts_sum<'a>(counts_by_attempts: impl Iterator<Item = (&'a Attempt, &'a Count)>) -> usize {
    counts_by_attempts
        .map(|(attempts, count)| attempts * count)
        .sum::<usize>()
}

#[ignore]
// Takes around 6s for the 2'315 solution words
#[test]
fn test_word_that_results_in_fewest_remaining_possible_words() {
    let lang = English;
    let words = Words::new(lang);
    let hsg = HintsBySecretByGuess::of(&words);
    let shg = SolutionsByHintByGuess::of(&words, &hsg);
    let cache = Cache::new(&words, &hsg, &shg);
    let game = Wordle::with(&words, &cache);

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
    let lang = English;
    let words = Words::new(lang);
    let hsg = HintsBySecretByGuess::of(&words);
    let shg = SolutionsByHintByGuess::of(&words, &hsg);
    let cache = Cache::new(&words, &hsg, &shg);
    let game = Wordle::with(&words, &cache);

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
    let lang = English;
    let words = Words::new(lang);
    let hsg = HintsBySecretByGuess::of(&words);
    let shg = SolutionsByHintByGuess::of(&words, &hsg);
    let cache = Cache::new(&words, &hsg, &shg);
    let game = Wordle::with(&words, &cache);

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
    let lang = English;
    let words = Words::new(lang);
    let hsg = HintsBySecretByGuess::of(&words);
    let shg = SolutionsByHintByGuess::of(&words, &hsg);
    let cache = Cache::new(&words, &hsg, &shg);
    let game = Wordle::with(&words, &cache);

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
fn test_get_hint() {
    let hint = "guest".to_word().calculate_hint(&"truss".to_word());
    assert_eq!("⬛🟨⬛🟩🟨", hint.to_string());

    let hint = "briar".to_word().calculate_hint(&"error".to_word());
    assert_eq!("⬛🟩⬛⬛🟩", hint.to_string());

    let hint = "sissy".to_word().calculate_hint(&"truss".to_word());
    assert_eq!("🟨⬛⬛🟩⬛", hint.to_string());

    let hint = "eject".to_word().calculate_hint(&"geese".to_word());
    assert_eq!("🟨⬛🟩⬛⬛", hint.to_string());

    let hint = "three".to_word().calculate_hint(&"beret".to_word());
    assert_eq!("🟨⬛🟩🟩🟨", hint.to_string());
}

#[ignore]
#[test]
fn lowest_total_number_of_remaining_solutions_only_counts_remaining_viable_solutions() {
    let secrets: BTreeSet<Secret> = ["augur", "briar", "friar", "lunar", "sugar"]
        .iter()
        .map(|w| w.to_word())
        .collect();
    let len = secrets.len() as f64;
    let guesses: Vec<Guess> = ["fubar", "rural", "aurar", "goier", "urial"]
        .iter()
        .map(|w| w.to_word())
        .collect();
    let lang = English;
    let mut words = Words::new(lang);
    words.guesses = guesses;
    words.secrets = secrets;
    let hsg = HintsBySecretByGuess::of(&words);
    let shg = SolutionsByHintByGuess::of(&words, &hsg);
    let cache = Cache::new(&words, &hsg, &shg);
    let game = Wordle::with(&words, &cache);

    let allowed = &game.words.guesses;
    let mut scores = fewest_remaining_solutions_for_game(&game);
    scores.sort_asc();
    // println!("scores {}", scores.to_string(5));
    assert_eq!(scores[0], (&allowed[0], 7.0 / len));
    assert_eq!(scores[1], (&allowed[1], 7.0 / len));
    assert_eq!(scores[2], (&allowed[4], 7.0 / len));
    assert_eq!(scores[3], (&allowed[2], 9.0 / len));
    assert_eq!(scores[4], (&allowed[3], 9.0 / len));
}
