use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use crate::cache::{Cache, WordIndex};
use crate::words::Language;

use super::*;

#[ignore] // ~4s for solutions only
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
    println!(
        "Best (lowest) variance:\n{}",
        words.scores_to_string(&variances, 5)
    );
    assert_eq!(variances.len(), words.guesses().len());

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
        words.scores_to_string(&remaining, 5)
    );
    assert_eq!(remaining.len(), words.guesses().len());

    // panic!(); // ~5s

    // Solutions only:
    // 139883 roate, 141217 raise, 141981 raile, 144227 soare, 147525 arise
    // All guesses:
    // Best (lowest) average remaining solution count:
    // 3745512 lares, 3789200 rales, 3923922 tares, 3941294 soare, 3953360 reais
    // 3963578 nares, 4017862 aeros, 4038902 rates, 4082728 arles, 4087910 serai

    let cache = Cache::new(&words);
    let solutions = words.secret_indices();
    let guessed = vec![];
    let totals = fewest_remaining_solutions(&words, &solutions, &guessed, &cache);

    println!(
        "Best (lowest) average remaining solution count:\n{}",
        words.scores_to_string(&totals, 5)
    );
    assert_eq!(totals.len(), words.guesses().len());

    // panic!(); // ~8s
    println!("\ndifferences between fewest total and expected remaining:");
    let (mut different, mut same) = (0, 0);
    for (i, ((low_i, count), (rem_i, remaining))) in totals.iter().zip(remaining.iter()).enumerate()
    {
        if low_i != rem_i {
            different += 1;
            println!(
                "{} ({}, {}) ({}, {})",
                i,
                words.get_string(*low_i),
                count,
                words.get_string(*rem_i),
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
    for (i, ((low_i, count), (var_i, variance))) in totals.iter().zip(variances.iter()).enumerate()
    {
        if low_i != var_i {
            different += 1;
            println!(
                "{} ({}, {}) ({}, {})",
                i,
                words.get_string(*low_i),
                count,
                words.get_string(*var_i),
                variance
            );
        } else {
            same += 1;
        }
    }
    println!("{} are same, {} are different", same, different);
}

// Previously used method, slightly less stable than lowest_total_number_of_remaining_solutions
fn variance_of_remaining_words(words: &Words) -> Vec<(WordIndex, f64)> {
    let average = words.secrets().count() as f64 / 243.0;
    let mut scores: Vec<(WordIndex, f64)> = words
        .guesses()
        .par_iter()
        .enumerate()
        .map(|(g, guess)| {
            let count_by_hint = count_by_hint(guess, words.secrets());
            let variance = count_by_hint
                .into_iter()
                .map(|count| (count as f64 - average).powf(2.0))
                .sum::<f64>() as f64
                / 243.0;
            (g as WordIndex, variance)
        })
        .collect();
    scores.sort_asc();
    scores
}

// Previously used method
fn expected_remaining_solution_counts(words: &Words) -> Vec<(WordIndex, f64)> {
    let total_solutions = words.secrets().count() as f64;
    let mut scores: Vec<_> = words
        .guesses()
        .par_iter()
        .enumerate()
        .map(|(g, guess)| {
            let count_by_hint = count_by_hint(guess, words.secrets());
            let expected_remaining_word_count = count_by_hint
                .into_iter()
                .map(|count| count.pow(2))
                .sum::<usize>() as f64
                / total_solutions;
            (g as WordIndex, expected_remaining_word_count)
        })
        .collect();
    scores.sort_asc();
    scores
}

// Allow because determine_hints expects &Guess not &[char]
fn count_by_hint<'a>(
    #[allow(clippy::ptr_arg)] guess: &Word,
    solutions: impl Iterator<Item = &'a Word>,
) -> [usize; 243] {
    let mut count_by_hint = [0; 243];
    for solution in solutions {
        let hint = guess.calculate_hint(solution);
        count_by_hint[hint.value() as usize] += 1;
    }
    count_by_hint
}

#[ignore] // ~6s English
#[test]
fn test_print_full_guess_tree() {
    print_full_guess_tree(English);
}
fn print_full_guess_tree(lang: Language) {
    let words = Words::new(lang);
    let secrets: SecretIndices = words.secret_indices();
    let cache = Cache::new(&words);
    let guessed = vec![];

    explore_tree(&words, &secrets, &guessed, &cache);
}
#[ignore]
#[test]
fn test_print_partial_guess_tree() {
    let lang = English;
    let words = Words::new(lang);
    let cache = Cache::new(&words);

    let secret = "piper".to_word();
    let guess1 = "roate".to_word(); // hint "🟨⬛⬛⬛🟨"
    let guess2 = "feued".to_word(); // hint "⬛⬛⬛🟩⬛"
    let secret_idx = words.index_of(&secret);
    let guess1_idx = words.index_of(&guess1);
    let guess2_idx = words.index_of(&guess2);

    let secrets1 = cache.solutions(guess1_idx, secret_idx);
    println!("{} roate secrets", secrets1.len());
    assert_eq!(102, secrets1.len());

    let secrets2 = cache.solutions(guess2_idx, secret_idx);
    println!("{} feued secrets", secrets2.len());
    assert_eq!(149, secrets2.len());

    let secrets: SecretIndices = secrets1.intersect(secrets2);
    println!(
        "{} intersected secrets {}",
        secrets.len(),
        words.indices_to_string(secrets.iter())
    );
    assert_eq!(18, secrets.len());

    let guessed = vec![guess1_idx, guess2_idx];
    explore_tree(&words, &secrets, &guessed, &cache);
}
fn explore_tree(words: &Words, secrets: &SecretIndices, guessed: &[WordIndex], cache: &Cache) {
    if guessed.len() == MAX_ATTEMPTS {
        println!(
            "            7. Still not found after 6 guesses {}. Secrets: {}",
            words.indices_to_string(guessed.iter()),
            words.indices_to_string(secrets.iter()),
        );
        return;
    } else if secrets.len() <= 2 {
        // 1 and 2 already printed info on how to proceed
        return;
    }
    let scores = fewest_remaining_solutions(words, secrets, guessed, cache);
    let guess = scores.lowest().unwrap();
    let mut guessed = guessed.to_vec();
    guessed.push(guess);

    let mut pairs: Vec<_> = cache
        .solutions_by_hint_for(guess)
        .iter()
        .enumerate()
        .filter(|(_hint, solutions)| !solutions.is_empty())
        .map(|(hint, solutions)| (hint as HintValue, solutions.intersect(secrets)))
        .filter(|(_hint, solutions)| !solutions.is_empty())
        .collect();
    pairs.sort_unstable_by(|(v1, s1), (v2, s2)| match s1.len().cmp(&s2.len()) {
        Ordering::Equal => v1.cmp(v2), // lower hint-value (more unknown) first
        fewer_elements_first => fewer_elements_first,
    });
    for (hint, secrets) in pairs {
        print_info(words, &guessed, hint, &secrets);
        explore_tree(words, &secrets, &guessed, cache)
    }
}
fn print_info(words: &Words, guessed: &[WordIndex], hint: HintValue, secrets: &SecretIndices) {
    let turn = guessed.len();
    let next = turn + 1;
    let after_next = turn + 2;
    let indent = "\t".repeat(turn - 1);
    let guess = words.get_string(*guessed.last().unwrap());
    let first = words.get_string(*secrets.iter().next().unwrap());
    let hint = Hints::from(hint);
    let count = secrets.len();
    print!("{indent}{turn}. guess {guess} + hint {hint} matches ");
    if secrets.len() == 1 {
        if guess == first {
            println!("{first}, solved as {turn}. guess!");
        } else {
            println!("{first}, use it as {next}. guess.");
        }
    } else if secrets.len() == 2 {
        let second = words.get_string(*secrets.iter().nth(1).unwrap());
        println!("{first} and {second}. Pick one at random to win by the {after_next}. guess.");
    } else if secrets.len() <= 5 {
        let sample = words.indices_to_string(secrets.iter());
        println!("{count} secrets {sample}.");
    } else {
        println!("{count} secrets, for example {first}.");
    }
}

#[ignore]
#[test]
fn test_trivial_turn_sums() {
    let lang = English;
    let words = Words::new(lang);
    let cache = Cache::new(&words);

    // if there's only one secret left, its score is 1
    let secrets1: SecretIndices = [0].into_iter().collect();
    let guessed = vec![];
    let picks = secrets1.len();
    let scores = turn_sums(&words, &secrets1, &guessed, &cache, picks, true);
    for (_, score) in scores {
        assert_eq!(score, 1);
    }

    // With 2 secrets left, their score (turn sum) is 3
    let secrets2: SecretIndices = [0, 1].into_iter().collect();
    let guessed = &vec![];
    let picks = secrets2.len();
    let scores = turn_sums(&words, &secrets2, guessed, &cache, picks, true);
    for (_, score) in scores {
        assert_eq!(score, 3);
    }
}

#[ignore]
#[test]
fn test_very_small_turn_sums() {
    let lang = English;
    let words = Words::new(lang);
    let guessed = vec![];
    let cache = Cache::new(&words);

    let log = true;
    let count = 4;
    let secrets: SecretIndices = words.secret_indices().into_iter().take(count).collect();
    let scores = turn_sums(&words, &secrets, &guessed, &cache, count, log);
    let (_, min) = scores.first().unwrap();
    let count = scores.iter().filter(|(_, s)| s == min).count();
    println!(
        "Turn sums {} ({} have value {}, avg {:.2})",
        words.scores_to_string(&scores, 5),
        count,
        min,
        *min as f64 / secrets.len() as f64
    );
    for (_, score) in scores {
        assert_eq!(score, 8);
    }
}

#[ignore]
#[test]
fn test_small_turn_sums() {
    let lang = English;
    let words = Words::new(lang);
    let guessed = vec![];
    let cache = Cache::new(&words);

    let log = true;
    let count = 500;
    let secrets: SecretIndices = words.secret_indices().into_iter().take(count).collect();
    let picks = 5;
    if log {
        println!(
            "\n{} secrets = {}",
            secrets.len(),
            words.secrets().collect::<Vec<_>>().sorted_string()
        );
    }
    let scores = turn_sums(&words, &secrets, &guessed, &cache, picks, log);
    let (_, min) = scores.first().unwrap();
    let count = scores.iter().filter(|(_, s)| s == min).count();
    println!(
        "Turn sums {} ({} have value {})",
        words.scores_to_string(&scores, 5),
        count,
        min
    );
}

// Times cached and un-cached (UC)
// picks:
//   1: 8060 'roate' (6s)
//   2: 8043 'roate', 8049 'raise' (11s)
//   5: 8007 'soare', 8020 'raile', 8024 'roate', 8030 'raise', 8062 'arise' (40s)
//  10: 7999 'soare', 8010 'roate', 8011 'raile', 8011 'raine', 8028 'raise',
//      8030 'arose', 8032 'irate', 8033 'orate', 8039 'ariel', 8054 'arise' (2min 4s)
//  20: 7981 'snare', 7995 'saner', 7997 'soare', 8002 'roate', 8004 'raine',
//      8007 'raile', 8012 'artel', 8017 'alter', 8019 'raise', 8021 'arose',
//      8022 'orate', 8026 'irate', 8026 'taler', 8036 'ariel', 8041 'ratel',
//      8048 'later', 8049 'arise', 8058 'arles', 8112 'realo', 8114 'aesir' (10min 31s)
//  30: 7929 'salet', 7930 'slate', 7930 'reast', 7980 'stare', 7981 'snare',
//      7982 'taser', 7994 'saner', 7997 'soare', 8002 'roate', 8004 'raine',
//      8004 'tares', 8005 'artel', 8007 'raile', 8014 'alter', 8014 'arose',
//      8015 'raise', 8018 'orate', 8020 'taler', 8023 'irate', 8026 'alert',
//      8036 'ariel', 8037 'ratel', 8043 'later', 8048 'arise', 8057 'arles',
//      8061 'lares', 8094 'oater', 8110 'realo', 8113 'aesir', 8146 'reais' (30min)
//  40: 7927 'crate', 7927 'reast', 7928 'salet', 7930 'slate', 7980 'stare',
//      7981 'snare', 7982 'taser', 7986 'toile', 7994 'saner', 7997 'soare',
//      8000 'roate', 8003 'artel', 8004 'raine', 8004 'tares', 8006 'raile',
//      8007 'saine', 8010 'strae', 8012 'arose', 8014 'alter', 8014 'seral',
//      8015 'raise', 8015 'orate', 8020 'taler', 8023 'irate', 8025 'alert',
//      8035 'ariel', 8035 'ratel', 8042 'later', 8044 'rates', 8048 'arise',
//      8057 'arles', 8060 'laser', 8061 'lares', 8068 'urate', 8071 'rales',
//      8094 'oater', 8109 'realo', 8113 'aesir', 8146 'reais', 8153 'serai' (90min)
//  70: 7920 'salet', 7924 'reast', 7927 'crate', 7928 'slate', 7928 'trace',
//      7941 'carle', 7943 'slane', 7949 'carte', 7968 'stale', 7969 'caret',
//      7970 'carse', 7974 'stare', 7975 'earst', 7978 'taser', 7980 'snare',
//      7986 'toile', 7993 'sorel', 7994 'saner', 7997 'soare', 8000 'roate',
//      8000 'tares', 8001 'resat', 8003 'artel', 8003 'liane', 8003 'raine',
//      8005 'antre', 8005 'raile', 8006 'saine', 8006 'strae', 8007 'tears',
//      8011 'arose', 8012 'seral', 8013 'alter', 8014 'earnt', 8015 'raise',
//      8015 'orate', 8016 'taler', 8018 'slier', 8020 'tales', 8022 'alert',
//      8023 'irate', 8027 'saice', 8031 'paire', 8032 'aisle', 8032 'coate',
//      8033 'ariel', 8034 'ratel', 8035 'learn', 8035 'litre', 8039 'rates',
//      8042 'later', 8046 'arise', 8057 'arles', 8059 'laser', 8061 'lares',
//      8063 'reals', 8068 'urate', 8070 'rales', 8082 'lears', 8089 'stoae',
//      8090 'nares', 8093 'oater', 8096 'oriel', 8102 'realo', 8107 'alure',
//      8113 'aesir', 8113 'terai', 8125 'aeros', 8146 'reais', 8152 'serai' (8h 16min)
#[ignore]
#[test]
fn test_medium_turn_sums() {
    let lang = English;
    let words = Words::new(lang);
    let secrets = words.secret_indices();
    let guessed = vec![];
    let cache = Cache::new(&words);

    let picks = 70;
    let log = false;
    let scores = turn_sums(&words, &secrets, &guessed, &cache, picks, log);
    println!("Turn sums: {}", words.scores_to_string(&scores, picks));
}

#[ignore] // ~7s for 1 top and 1 sub
#[test]
fn test_tree_depth() {
    let lang = English;
    let words = Words::new(lang);
    let secrets = words.secret_indices();
    let cache = Cache::new(&words);

    let guessed = [];

    let count_by_attempts = count_by_attempts(&words, &secrets, &guessed, &cache);
    print_stats(count_by_attempts.iter());

    // 'roate' with 1 top 1 sub, this can be seen in easy_mode_english.txt
    let expected = vec![
        (2, 44 + 11),        //                          44 w 2 +  11 w 50/50 of 2 or 3
        (3, 11 + 904 + 215), //  11 w 50/50 of 2 or 3 + 904 w 3 + 215 w 50/50 of 3 or 4
        (4, 215 + 844 + 31), // 215 w 50/50 of 3 or 4 + 944 w 4 +  31 w 50/50 of 4 or 5
        (5, 31 + 9),         //  31 w 50/50 of 4 or 5 +   9 w 5
    ]
    .into_iter()
    .collect();
    assert_eq!(count_by_attempts, expected);
}

fn count_by_attempts(
    words: &Words,
    secrets: &SecretIndices,
    guessed: &[WordIndex],
    cache: &Cache,
) -> BTreeMap<Attempt, Count> {
    if guessed.len() > MAX_ATTEMPTS {
        // avoid unsolvable paths my making them cost a lot
        return [(usize::MAX, 1)].into_iter().collect();
    }
    let top = 1;
    let sub = 1;
    let picks = if guessed.is_empty() { top } else { sub };
    fewest_remaining_solutions(words, secrets, guessed, cache)
        .into_par_iter()
        .take(picks)
        .map(|(guess, _score)| {
            let mut guessed = guessed.to_vec();
            guessed.push(guess);

            let mut counts_by_attempts: BTreeMap<Attempt, Count> = BTreeMap::new();

            for solutions in cache
                .solutions_by_hint_for(guess)
                .iter()
                .filter(|solutions| !solutions.is_empty())
                .map(|solutions| solutions.intersect(secrets))
                .filter(|solutions| !solutions.is_empty())
            {
                let attempt = guessed.len();
                match solutions.len() {
                    1 => {
                        if solutions.contains(&guess) {
                            // Solved on this turn!
                            *counts_by_attempts.entry(attempt).or_default() += 1;
                        } else {
                            // We guess this only viable solution on the next attempt
                            *counts_by_attempts.entry(attempt + 1).or_default() += 1;
                        }
                    }
                    2 => {
                        if solutions.contains(&guess) {
                            // If one of two solutions is our guess, there is a 50/50 chance of
                            // getting it on this guess, or the next one
                            *counts_by_attempts.entry(attempt).or_default() += 1;
                            *counts_by_attempts.entry(attempt + 1).or_default() += 1;
                        } else {
                            // If there are two solutions left, there is a 50/50 chance of
                            // getting it on the next guess, or the one after that
                            *counts_by_attempts.entry(attempt + 1).or_default() += 1;
                            *counts_by_attempts.entry(attempt + 2).or_default() += 1;
                        }
                    }
                    _ => {
                        // If there are more guesses, add all the result to the map
                        for (attempt, count) in
                            count_by_attempts(words, &solutions, &guessed, cache)
                        {
                            *counts_by_attempts.entry(attempt).or_default() += count
                        }
                    }
                }
            }
            let total = counts_by_attempts.iter().map(|(_, cnt)| cnt).sum::<usize>() as f64;
            let sum = attempts_sum(counts_by_attempts.iter());
            let average = sum as f64 / total;
            if total == 2315_f64 {
                println!(
                    "{:.3} avg ({} sum) for 1. guess {} with {:2} top and {:2} sub",
                    average,
                    sum,
                    words.get_string(guess),
                    top,
                    sub
                );
            }

            (counts_by_attempts, sum)
        })
        .min_by_key(|(_count_by_attempts, sum)| *sum)
        .map(|(count_by_attempts, _sum)| count_by_attempts)
        .unwrap()
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
    let cache = Cache::new(&words);
    let mut t_cached = start.elapsed();
    println!(
        "0. total time [ms]: {:4} cache initialization",
        t_cached.as_millis(),
    );

    let sum_with_cache = |words: &Words| -> usize {
        words
            .guess_indices()
            .par_iter()
            .map(|&g| {
                words
                    .secret_indices()
                    .iter()
                    .map(|&s| cache.hint(g, s) as usize)
                    .sum::<usize>()
            })
            .sum()
    };

    let sum_without_cache = |words: &Words| -> usize {
        words
            .guess_indices()
            .par_iter()
            .map(|&g| {
                words
                    .secret_indices()
                    .iter()
                    .map(|&s| words.get(g).calculate_hint(words.get(s)).value() as usize)
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

#[ignore] // ~ 3s
#[test]
// Top 5: 60.425 'roate', 61.000 'raise', 61.331 'raile', 62.301 'soare', 63.725 'arise'
fn run_fewest_remaining_solutions_with_depth_1() {
    let words = Words::new(English);
    let cache = Cache::new(&words);
    let solutions: BTreeSet<WordIndex> = words.secret_indices();
    let guessed = vec![];

    let start = Instant::now();
    let scores = fewest_remaining_solutions(&words, &solutions, &guessed, &cache);
    let elapsed = start.elapsed();
    println!("{:?} to calc {} scores", elapsed, scores.len());

    println!("Top 5: {}", words.scores_to_string(&scores, 5));

    let best = words.get(scores.first().unwrap().0).clone();
    assert_eq!("roate".to_word(), best);
}

#[ignore] // ~1h 45min
#[test]
// Top 16:  257.015 'roate', 263.165 'raile', 279.479 'ariel', 280.538 'raise', 281.829 'orate',
//          283.356 'soare', 285.917 'irate', 286.842 'arose', 293.042 'artel', 293.387 'taler',
//          298.339 'arles', 303.208 'raine', 308.310 'arise', 309.080 'realo', 309.359 'ratel',
//          339.410 'aesir'
fn run_fewest_remaining_solutions_with_depth_2() {
    let words = Words::new(English);
    let cache = Cache::new(&words);
    let solutions: BTreeSet<WordIndex> = words.secret_indices();
    let guessed = vec![];

    let picks = 16;
    let depth = 2;
    let start = Instant::now();
    let scores =
        fewest_remaining_solutions_recursive(&words, &solutions, &guessed, &cache, picks, depth);
    let elapsed = start.elapsed();
    println!("{:?} to calc {} scores", elapsed, scores.len());

    println!("Top {picks}: {}", words.scores_to_string(&scores, picks));

    let best = words.get(scores.first().unwrap().0).clone();
    assert_eq!("roate".to_word(), best);
}
fn fewest_remaining_solutions_recursive(
    words: &Words,
    solutions: &SecretIndices,
    guessed: &[WordIndex],
    cache: &Cache,
    picks: usize,
    depth: usize,
) -> Vec<(WordIndex, f64)> {
    if depth == 1 {
        return fewest_remaining_solutions(words, solutions, guessed, cache);
    }
    let is_first_turn = solutions.len() as WordIndex == words.secret_count();
    let first_words = if is_first_turn {
        fewest_remaining_solutions(words, solutions, guessed, cache)
            // println!("{}", words.scores_to_string(&scores, picks));
            .into_iter()
            .take(picks)
            .map(|(i, _s)| i)
            .collect()
    } else {
        words.guess_indices()
    };

    let solution_count = solutions.len() as f64;
    let mut scores: Vec<(WordIndex, f64)> = first_words
        .into_par_iter()
        .filter(|guess_idx| !guessed.contains(guess_idx))
        .map(|guess| {
            let mut guessed = guessed.to_vec();
            guessed.push(guess);
            let sum: usize = solutions
                .iter() //.take(200)
                .map(|&secret| {
                    if guess == secret {
                        0
                    } else {
                        let solutions = cache.solutions(guess, secret);
                        if is_first_turn {
                            let count = solutions.len();
                            let (score, _i) = fewest_remaining_solutions_recursive(
                                words,
                                solutions,
                                &guessed,
                                cache,
                                picks,
                                depth - 1,
                            )
                            .lowest_pair()
                            .unwrap();
                            (count as f64 * score) as usize
                        } else {
                            let solutions: SecretIndices =
                                solutions.intersection(solutions).cloned().collect();
                            let count = solutions.len() as f64;
                            let score = fewest_remaining_solutions_recursive(
                                words,
                                &solutions,
                                &guessed,
                                cache,
                                picks,
                                depth - 1,
                            )
                            .lowest_score()
                            .unwrap();
                            (count * score) as usize
                        }
                    }
                })
                .sum();
            // panic!();
            (guess as WordIndex, sum as f64 / solution_count)
        })
        .collect();

    scores.sort_unstable_by(|(a_idx, a_score), (b_idx, b_score)| {
        match a_score.partial_cmp(b_score) {
            Some(Ordering::Equal) | None => a_idx.cmp(b_idx),
            Some(by_value) => by_value,
        }
    });
    scores
}

#[ignore] // ~0.4s
#[test]
// Top 5: 31.302 'raine', 35.260 'taler', 36.212 'raten', 36.260 'laser', 36.633 'reale'
fn find_good_first_word_german() {
    let lang = At;
    let words = Words::new(lang);
    let cache = Cache::new(&words);
    let solutions: BTreeSet<WordIndex> = words.secret_indices();
    let guessed = vec![];
    let scores = fewest_remaining_solutions(&words, &solutions, &guessed, &cache);
    println!("Top 5 {}", words.scores_to_string(&scores, 5));
    let best = words.get(scores.first().unwrap().0).clone();
    assert_eq!("raine".to_word(), best);
}

// ~10s (i9) or ~13s (M1) or 6.5s (M1 Max) or 2.1s (M1 Ultra) for 5 single German words
// ~1min 51s (i9) or ~2min 21s (M1) or 67s (M1 Max) or 19.6s (M1 Ultra) for 5 single English words
//
// At:
// Best 1. guesses: 31.30 'raine', 35.26 'taler', 36.21 'raten', 36.26 'laser', 36.63 'reale'
// Best 2. guesses after 1. 'raine': 3.25 'holst', 3.32 'kults', 3.34 'lotus', 3.39 'stuhl', 3.52 'buhlt'
// Best 3. guesses after 1. 'raine' and 2. 'holst': 1.40 'dumpf', 1.43 'umgab', 1.46 'umweg', 1.49 'bekam', 1.50 'bezug'
// Best 4. guesses after 1. 'raine' and 2. 'holst' and 3. 'dumpf': 1.06 'biwak', 1.08 'abweg', 1.09 'bezog', 1.09 'bezug', 1.09 'beeck'
// Best 5. guesses after 1. 'raine' and 2. 'holst' and 3. 'dumpf' and 4. 'biwak': 1.01 'legen', 1.01 'leger', 1.01 'engen', 1.01 'enzen', 1.01 'genen'
//
// Ch:
// Best 1. guesses: 54.627 'tarne', 57.669 'raine', 59.246 'taler', 60.238 'altre', 61.313 'raten'
// Best 2. guesses after 1. 'tarne': 5.791 'bilds', 5.857 'selig', 5.911 'kilos', 5.925 'kulis', 5.946 'beils'
// Best 3. guesses after 1. 'tarne' and 2. 'bilds': 1.883 'umzog', 1.914 'keuch', 1.916 'hekto', 1.918 'humor', 1.951 'hugos'
// Best 4. guesses after 1. 'tarne' and 2. 'bilds' and 3. 'umzog': 1.185 'kehle', 1.189 'kehre', 1.190 'hecke', 1.198 'hüpft', 1.200 'heckt'
// Best 5. guesses after 1. 'tarne' and 2. 'bilds' and 3. 'umzog' and 4. 'kehle': 1.052 'prüft', 1.053 'hüpft', 1.053 'rümpf', 1.055 'äpfel', 1.059 'prüfe'
//
// De:
// Best 1. guesses: 41.094 'artel', 41.104 'sarte', 41.231 'taler', 41.231 'taler', 42.726 'alter'
// Best 2. guesses after 1. 'artel': 4.550 'minsk', 4.821 'bison', 4.821 'bison', 4.854 'minos', 4.863 'minus'
// Best 3. guesses after 1. 'artel' and 2. 'minsk': 1.478 'behuf', 1.547 'bewog', 1.563 'beuge', 1.583 'tough', 1.599 'bezug'
// Best 4. guesses after 1. 'artel' and 2. 'minsk' and 3. 'behuf': 1.087 'epode', 1.095 'ponge', 1.102 'geode', 1.102 'geode', 1.104 'dispo'
// Best 5. guesses after 1. 'artel' and 2. 'minsk' and 3. 'behuf' and 4. 'epode': 1.016 'zwang', 1.016 'zwang', 1.020 'zweig', 1.020 'zweig', 1.020 'zwerg'
//
// Uber:
// Best 1. guesses: 30.230 'senar', 31.443 'taler', 31.770 'artel', 32.099 'raten', 32.191 'sarte'
// Best 2. guesses after 1. 'senar': 3.282 'light', 3.389 'futil', 3.452 'litho', 3.505 'multi', 3.591 'tulio'
// Best 3. guesses after 1. 'senar' and 2. 'light': 1.327 'dumpf', 1.327 'kumpf', 1.366 'borke', 1.370 'kombi', 1.376 'krume'
// Best 4. guesses after 1. 'senar' and 2. 'light' and 3. 'dumpf': 1.050 'borke', 1.050 'kerbe', 1.050 'klebe', 1.052 'bowle', 1.054 'kerwe'
// Best 5. guesses after 1. 'senar' and 2. 'light' and 3. 'dumpf' and 4. 'borke': 1.011 'zween', 1.013 'twist', 1.013 'watte', 1.013 'weite', 1.015 'tweed'
//
// English (original / NY times):
// Best 1. guesses: 60.132 'roate', 60.744 'raise', 61.194 'raile', 62.058 'soare', 63.468 'arise'
// Best 2. guesses after 1. 'roate': 5.103 'linds', 5.152 'sling', 5.186 'clips', 5.270 'limns', 5.316 'blins'
// Best 3. guesses after 1. 'roate' and 2. 'linds': 1.641 'chump', 1.684 'bumph', 1.777 'crump', 1.799 'clump', 1.799 'bumpy'
// Best 4. guesses after 1. 'roate' and 2. 'linds' and 3. 'chump': 1.149 'gleby', 1.155 'gawky', 1.156 'gybed', 1.160 'beefy', 1.160 'befog'
// Best 5. guesses after 1. 'roate' and 2. 'linds' and 3. 'chump' and 4. 'gleby': 1.036 'wakfs', 1.045 'waift', 1.046 'swift', 1.049 'fatwa', 1.050 'fawns'
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
#[ignore]
#[test]
fn find_good_word_combos() {
    let lang = English;
    let words = Words::new(lang);
    let cache = Cache::new(&words);

    let solutions: BTreeSet<WordIndex> = words.secret_indices();
    let guessed = vec![];
    let scores = fewest_remaining_solutions(&words, &solutions, &guessed, &cache);
    println!("Best 1. guesses: {}", words.scores_to_string(&scores, 5));

    let top_pick_count = 1;
    for (guess1, _) in scores.into_iter().take(top_pick_count) {
        let guessed = [guess1];
        let scores = find_best_next_guesses(&words, &guessed, &cache);

        println!(
            "Best 2. guesses after 1. {}: {}",
            words.get_string(guess1),
            words.scores_to_string(&scores, 5)
        );

        for (guess2, _) in scores.into_iter().take(top_pick_count) {
            let guessed = [guess1, guess2];
            let scores = find_best_next_guesses(&words, &guessed, &cache);
            println!(
                "Best 3. guesses after 1. {} and 2. {}: {}",
                words.get_string(guess1),
                words.get_string(guess2),
                words.scores_to_string(&scores, 5)
            );

            for (guess3, _) in scores.into_iter().take(top_pick_count) {
                let guessed = [guess1, guess2, guess3];
                let scores = find_best_next_guesses(&words, &guessed, &cache);
                println!(
                    "Best 4. guesses after 1. {} and 2. {} and 3. {}: {}",
                    words.get_string(guess1),
                    words.get_string(guess2),
                    words.get_string(guess3),
                    words.scores_to_string(&scores, 5)
                );

                for (guess4, _) in scores.into_iter().take(top_pick_count) {
                    let guessed = [guess1, guess2, guess3, guess4];
                    let scores = find_best_next_guesses(&words, &guessed, &cache);
                    println!(
                        "Best 5. guesses after 1. {} and 2. {} and 3. {} and 4. {}: {}",
                        words.get_string(guess1),
                        words.get_string(guess2),
                        words.get_string(guess3),
                        words.get_string(guess4),
                        words.scores_to_string(&scores, 5)
                    );
                }
            }
        }
    }
}

fn find_best_next_guesses(
    words: &Words,
    guessed_indices: &[WordIndex],
    cache: &Cache,
) -> Vec<(WordIndex, f64)> {
    let first = *guessed_indices.iter().next().unwrap();
    let secret_count = words.secret_indices().len() as f64;
    let mut scores: Vec<_> = words
        .guess_indices()
        .into_par_iter()
        .filter(|next_idx| !guessed_indices.contains(next_idx))
        .map(|next_idx| {
            let count: usize = words
                .secret_indices()
                .into_iter()
                .map(|secret_idx| {
                    let solutions1 = cache.solutions(first, secret_idx);
                    let solutions2 = cache.solutions(next_idx, secret_idx);

                    // apply first and next guess
                    let mut solutions = solutions1.intersect(solutions2);

                    // Apply other previous guesses
                    for other_idx in guessed_indices.iter().skip(1).cloned() {
                        let other_solutions = cache.solutions(other_idx, secret_idx);
                        solutions = solutions.intersect(other_solutions);
                    }
                    solutions.len()
                })
                .sum();
            (next_idx as WordIndex, count as f64 / secret_count)
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

#[ignore] // ~26 min (i9) or ~39 min (M1) or 21.7min (M1 Max) or 53 min (M1 Ultra)
#[test]
// 3.55 average attempts; 2: 40, 3: 999, 4: 1234, 5: 42
fn autoplay_word_that_results_in_fewest_remaining_solutions() {
    autoplay_and_print_stats(WordThatResultsInFewestRemainingSolutions);
}

#[ignore] // 1min 38s (i9) or 1min 53s (M1) or 74s (M1 Max) or 237s (M1 Ultra)
#[test]
// 3.36 average attempts; 2: 42, 3: 685, 4: 429, 5: 15
fn autoplay_word_that_results_in_fewest_remaining_solutions_german() {
    autoplay_and_print_stats_with_language(WordThatResultsInFewestRemainingSolutions, At);
}

#[ignore]
#[test]
// Average attempts = 3.564; 0 (0.000%) failed games (> 6 attempts):
// 2: 41, 3: 530, 4: 502, 5: 94, 6: 4
fn autoplay_german_raine_holst_dumpf_biwak_legen() {
    let strategy = FixedGuessList::new(vec!["raine", "holst", "dumpf", "biwak", "legen"]);
    autoplay_and_print_stats_with_language(strategy, At);
}

#[ignore]
#[test]
// Average attempts = 3.564; 0 (0.000%) failed games (> 6 attempts):
// 2: 41, 3: 530, 4: 502, 5: 94, 6: 4
fn autoplay_german_raine_holst_dumpf_biwak() {
    let strategy = FixedGuessList::new(vec!["raine", "holst", "dumpf", "biwak"]);
    autoplay_and_print_stats_with_language(strategy, At);
}

#[ignore]
#[test]
// Average attempts = 3.749; 9 (0.546%) failed games (> 6 attempts):
// 2: 82, 3: 649, 4: 610, 5: 229, 6: 70, 7: 6, 8: 3
fn autoplay_german_tarne_helis_gudok_zamba_fiept() {
    let strategy = FixedGuessList::new(vec!["tarne", "helis", "gudok", "zamba", "fiept"]);
    autoplay_and_print_stats_with_language(strategy, At);
}

#[ignore]
#[test]
// Average attempts = 3.763; 7 (0.424%) failed games (> 6 attempts):
// 2: 79, 3: 623, 4: 631, 5: 249, 6: 60, 7: 6, 8: 1
fn autoplay_german_tarne_helis_gudok_zamba() {
    let strategy = FixedGuessList::new(vec!["tarne", "helis", "gudok", "zamba"]);
    autoplay_and_print_stats_with_language(strategy, At);
}

#[ignore]
#[test]
// 4.071 average attempts; 2: 41, 3: 438, 4: 1185, 5: 617, 6: 34
fn autoplay_tubes_fling_champ_wordy() {
    let strategy = FixedGuessList::new(vec!["tubes", "fling", "champ", "wordy"]);
    autoplay_and_print_stats(strategy);
}

#[ignore]
#[test]
// 4.332 average attempts; 2: 27, 3: 364, 4: 944, 5: 788, 6: 178, 7: 14; 14 (0.60%) failures
fn autoplay_crwth_faxed_vulgo_zinky_jambs() {
    let strategy = FixedGuessList::new(vec!["crwth", "faxed", "vulgo", "zinky", "jambs"]);
    autoplay_and_print_stats(strategy);
}

#[ignore]
#[test]
// 3.821 average attempts; 1: 1, 2: 48, 3: 710, 4: 1201, 5: 317, 6: 37, 7: 1; 1 (0.04%) failures
fn autoplay_crane_sloth_pudgy() {
    let strategy = FixedGuessList::new(vec!["crane", "sloth", "pudgy"]);
    autoplay_and_print_stats(strategy);
}

#[ignore]
#[test]
// 4.107 average attempts; 1: 1, 2: 31, 3: 429, 4: 1164, 5: 641, 6: 47, 7: 2; 2 (0.09%) failures
fn autoplay_spade_lucky_brown_fight() {
    let strategy = FixedGuessList::new(vec!["spade", "lucky", "brown", "fight"]);
    autoplay_and_print_stats(strategy);
}

#[ignore]
#[test]
// 3.982 average attempts; 1: 1, 2: 52, 3: 537, 4: 1194, 5: 466, 6: 58, 7: 7; 7 (0.30%) failures
fn autoplay_stale_dough_brink() {
    let strategy = FixedGuessList::new(vec!["stale", "dough", "brink"]);
    autoplay_and_print_stats(strategy);
}

#[ignore]
#[test]
// 4.549 average attempts; 1: 1, 2: 19, 3: 258, 4: 821, 5: 889, 6: 301, 7: 26; 26 (1.12%) failures
fn autoplay_fjord_waltz_nymph_quick_vexes() {
    let strategy = FixedGuessList::new(vec!["fjord", "waltz", "nymph", "quick", "vexes"]);
    autoplay_and_print_stats(strategy);
}

#[ignore]
#[test]
// 4.495 average attempts; 1: 1, 2: 19, 3: 257, 4: 828, 5: 988, 6: 210, 7: 12; 12 (0.52%) failures
fn autoplay_fjord_waltz_psych_imbue() {
    let strategy = FixedGuessList::new(vec!["fjord", "waltz", "psych", "imbue"]);
    autoplay_and_print_stats(strategy);
}

#[ignore]
#[test]
// 4.599 average attempts; 1: 1, 2: 32, 3: 198, 4: 683, 5: 1159, 6: 234, 7: 8; 8 (0.35%) failures
fn autoplay_glyph_jocks_fixed_brawn() {
    let strategy = FixedGuessList::new(vec!["glyph", "jocks", "fixed", "brawn"]);
    autoplay_and_print_stats(strategy);
}

#[ignore]
#[test]
// 4.259 average attempts; 2: 30, 3: 457, 4: 924, 5: 713, 6: 169, 7: 22; 22 (0.95%) failures
fn autoplay_glent_brick_jumpy_vozhd_waqfs() {
    let strategy = FixedGuessList::new(vec!["glent", "brick", "jumpy", "vozhd", "waqfs"]);
    autoplay_and_print_stats(strategy);
}

#[ignore]
#[test]
// 4.256 average attempts; 2: 30, 3: 457, 4: 924, 5: 715, 6: 172, 7: 17; 17 (0.73%) failures
fn autoplay_glent_brick_jumpy_vozhd() {
    let strategy = FixedGuessList::new(vec!["glent", "brick", "jumpy", "vozhd"]);
    autoplay_and_print_stats(strategy);
}

#[ignore]
#[test]
// Average attempts = 3.829; 7 (0.302%) failed games (> 6 attempts):
// 2: 57, 3: 770, 4: 1077, 5: 342, 6: 62, 7: 6, 8: 1
fn autoplay_roate_linds_chump_gawky_befit() {
    let strategy = FixedGuessList::new(vec!["roate", "linds", "chump", "gawky", "befit"]);
    autoplay_and_print_stats(strategy);
}

#[ignore]
#[test]
// Average attempts = 3.816; 2 (0.086%) failed games (> 6 attempts):
// 2: 47, 3: 796, 4: 1065, 5: 353, 6: 52, 7: 2
fn autoplay_roate_linds_chump_gawky() {
    let strategy = FixedGuessList::new(vec!["roate", "linds", "chump", "gawky"]);
    autoplay_and_print_stats(strategy);
}

#[ignore]
#[test]
// Average attempts = 3.807; 5 (0.216%) failed games (> 6 attempts):
// 2: 62, 3: 789, 4: 1073, 5: 321, 6: 65, 7: 4, 8: 1
fn autoplay_roate_linds_chump_gleby_wakfs() {
    let strategy = FixedGuessList::new(vec!["roate", "linds", "chump", "gleby", "wakfs"]);
    autoplay_and_print_stats(strategy);
}

#[ignore]
#[test]
// Average attempts = 3.809; 4 (0.173%) failed games (> 6 attempts):
// 2: 65, 3: 769, 4: 1085, 5: 340, 6: 52, 7: 4
fn autoplay_roate_linds_chump_gleby() {
    let strategy = FixedGuessList::new(vec!["roate", "linds", "chump", "gleby"]);
    autoplay_and_print_stats(strategy);
}

#[ignore]
#[test]
// Average attempts = 3.994; 21 (0.907%) failed games (> 6 attempts):
// 2: 53, 3: 738, 4: 873, 5: 496, 6: 134, 7: 19, 8: 2
fn autoplay_soare_until_pygmy_whack() {
    let strategy = FixedGuessList::new(vec!["soare", "until", "pygmy", "whack"]);
    autoplay_and_print_stats(strategy);
}

#[ignore]
#[test]
// Average attempts = 4.879; 219 (9.460%) failed games (> 6 attempts):
// 1: 1, 2: 18, 3: 317, 4: 592, 5: 622, 6: 546, 7: 200, 8: 18, 9: 1
fn autoplay_quick_brown_foxed_jumps_glazy_vetch() {
    let strategy = FixedGuessList::new(vec!["quick", "brown", "foxed", "jumps", "glazy", "vetch"]);
    autoplay_and_print_stats(strategy);
}

#[ignore]
#[test]
// Average attempts = 4.024; 2 (0.086%) failed games (> 6 attempts):
// 1: 1, 2: 51, 3: 500, 4: 1177, 5: 513, 6: 71, 7: 2
fn autoplay_brake_dying_clots_whump() {
    let strategy = FixedGuessList::new(vec!["brake", "dying", "clots", "whump"]);
    autoplay_and_print_stats(strategy);
}

#[ignore]
#[test]
// Average attempts = 3.910; 1 (0.043%) failed games (> 6 attempts):
// 1: 1, 2: 64, 3: 636, 4: 1114, 5: 443, 6: 56, 7: 1
fn autoplay_maple_sight_frown_ducky() {
    let strategy = FixedGuessList::new(vec!["maple", "sight", "frown", "ducky"]);
    autoplay_and_print_stats(strategy);
}

#[ignore]
#[test]
// Average attempts = 4.034; 5 (0.216%) failed games (> 6 attempts):
// 1: 1, 2: 49, 3: 550, 4: 1081, 5: 544, 6: 85, 7: 5
fn autoplay_fiend_paths_crumb_glows() {
    let strategy = FixedGuessList::new(vec!["fiend", "paths", "crumb", "glows"]);
    autoplay_and_print_stats(strategy);
}

#[ignore]
#[test]
// Average attempts = 3.785; 11 (0.475%) failed games (> 6 attempts):
// 2: 67, 3: 854, 4: 974, 5: 364, 6: 45, 7: 7, 8: 4
fn autoplay_reals_point_ducky() {
    let strategy = FixedGuessList::new(vec!["reals", "point", "ducky"]);
    autoplay_and_print_stats(strategy);
}

#[ignore]
#[test]
// Average attempts = 3.831; 8 (0.346%) failed games (> 6 attempts):
// 2: 72, 3: 725, 4: 1113, 5: 344, 6: 53, 7: 6, 8: 1, 9: 1
fn autoplay_laser_pitch_mound() {
    let strategy = FixedGuessList::new(vec!["laser", "pitch", "mound"]);
    autoplay_and_print_stats(strategy);
}

#[ignore]
#[test]
// Average attempts = 3.743; 23 (0.994%) failed games (> 6 attempts):
// 1: 1, 2: 135, 3: 853, 4: 924, 5: 304, 6: 75, 7: 16, 8: 5, 9: 2
fn autoplay_most_frequent_global_characters() {
    autoplay_and_print_stats(MostFrequentGlobalCharacter);
}

#[ignore]
#[test]
// Average attempts = 3.715; 26 (1.123%) failed games (> 6 attempts):
// 1: 1, 2: 130, 3: 919, 4: 888, 5: 268, 6: 83, 7: 18, 8: 7, 10: 1
fn autoplay_most_frequent_global_characters_high_variety_word() {
    autoplay_and_print_stats(MostFrequentGlobalCharacterHighVarietyWord);
}

#[ignore]
#[test]
// Average attempts = 3.778; 22 (0.950%) failed games (> 6 attempts):
// 1: 1, 2: 148, 3: 773, 4: 955, 5: 343, 6: 73, 7: 19, 8: 2, 9: 1
fn autoplay_most_frequent_characters_per_pos() {
    autoplay_and_print_stats(MostFrequentCharacterPerPos);
}

#[ignore]
#[test]
// Average attempts = 3.667; 19 (0.821%) failed games (> 6 attempts):
// 1: 1, 2: 148, 3: 903, 4: 929, 5: 263, 6: 52, 7: 15, 8: 2, 9: 2
fn autoplay_most_frequent_characters_per_pos_high_variety_word() {
    autoplay_and_print_stats(MostFrequentCharacterPerPosHighVarietyWord);
}

#[ignore]
#[test]
// Average attempts = 3.951; 54 (2.333%) failed games (> 6 attempts):
// 2: 50, 3: 829, 4: 873, 5: 390, 6: 119, 7: 34, 8: 15, 9: 5
fn autoplay_most_frequent_characters_of_words() {
    autoplay_and_print_stats(MostFrequentCharactersOfRemainingWords);
}

#[ignore]
#[test]
// Average attempts = 3.861; 32 (1.382%) failed games (> 6 attempts):
// 2: 78, 3: 813, 4: 932, 5: 380, 6: 80, 7: 22, 8: 8, 9: 2
fn autoplay_most_frequent_unused_characters() {
    let words = Words::new(English);
    autoplay_and_print_stats(MostFrequentUnusedCharacters::new(words.guesses()));
}
struct MostFrequentUnusedCharacters<'w> {
    combined_global_char_count_sums_by: HashMap<&'w Word, usize>,
}
impl<'w> MostFrequentUnusedCharacters<'w> {
    #[allow(clippy::ptr_arg)] // for global_character_counts_in defined for Vec<Guess> not &[Guess]
    fn new(guesses: &'w Vec<Word>) -> Self {
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
    fn pick(&self, game: &Wordle) -> Option<Word> {
        if game.solutions.len() < 10 {
            return None;
        };
        let words_with_most_new_chars: Vec<&Word> =
            words_with_most_new_chars(&game.guessed_chars(), game.allowed())
                .into_iter()
                .map(|(_, word)| word)
                .collect();

        let mut scores: Vec<(&Word, usize)> = words_with_most_new_chars
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
        scores.highest().cloned()
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
fn autoplay_most_other_words_in_at_least_one_open_position() {
    autoplay_and_print_stats(MatchingMostOtherWordsInAtLeastOneOpenPosition);
}

#[ignore]
#[test]
// Average attempts = 3.794; 29 (1.253%) failed games (> 6 attempts):
// 1: 1, 2: 114, 3: 856, 4: 886, 5: 341, 6: 88, 7: 23, 8: 6
fn autoplay_most_other_words_in_at_least_one_open_position_high_variety_word() {
    autoplay_and_print_stats(MatchingMostOtherWordsInAtLeastOneOpenPositionHighVarietyWord);
}

fn autoplay_and_print_stats<S: TryToPickWord + Sync>(strategy: S) {
    autoplay_and_print_stats_with_language(strategy, English);
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
    let game = Wordle::with(English);

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
    let game = Wordle::with(English);
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
// Takes around 2s (parallel) guessing with 12'972 combined words in 2'315 solutions.
#[test]
fn test_word_from_combined_list_that_results_in_fewest_remaining_possible_solution_words() {
    let game = Wordle::with(English);
    let word = WordThatResultsInFewestRemainingSolutions.pick(&game);

    // Using 12'972 combined words on 2'315 solutions
    // Worst:
    // 862 zoppo, 870 kudzu, 871 susus, 878 yukky, 883 fuffy,
    // 886 gyppy, 901 jugum, 903 jujus, 925 qajaq, 967 immix

    // Remaining best:
    // 67 raine, 66 arose, 65 ariel, 64 orate, 64 irate,
    // 64 arise, 62 soare, 61 raile, 61 raise, 60 roate

    assert_eq!(word.unwrap().to_string(), "'roate'");
}

#[ignore]
#[test]
fn test_pick_word_that_exactly_matches_most_others_in_at_least_one_open_position() {
    let game = Wordle::with(English);
    let word = MatchingMostOtherWordsInAtLeastOneOpenPosition.pick(&game);
    assert_eq!(word.unwrap().to_string(), "'sauce'");
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
fn test_calculate_hint() {
    assert_eq!("⬛🟨⬛🟩🟨", "guest".calculate_hint(&"truss").to_string());
    assert_eq!("⬛🟩⬛⬛🟩", "briar".calculate_hint(&"error").to_string());
    assert_eq!("🟨⬛⬛🟩⬛", "sissy".calculate_hint(&"truss").to_string());
    assert_eq!("🟨⬛🟩⬛⬛", "eject".calculate_hint(&"geese").to_string());
    assert_eq!("🟨⬛🟩🟩🟨", "three".calculate_hint(&"beret").to_string());

    assert_eq!("⬛⬛🟨⬛🟨", "speed".calculate_hint(&"abide").to_string());
    assert_eq!("🟨⬛🟨🟨⬛", "speed".calculate_hint(&"erase").to_string());
    assert_eq!("🟩⬛🟩⬛⬛", "speed".calculate_hint(&"steal").to_string());
    assert_eq!("⬛🟨🟩🟨⬛", "speed".calculate_hint(&"crepe").to_string());
}

#[ignore]
#[test]
fn lowest_total_number_of_remaining_solutions_only_counts_remaining_viable_solutions() {
    let secrets = ["augur", "briar", "friar", "lunar", "sugar"]
        .iter()
        .map(|w| w.to_word())
        .collect();
    let guesses = ["fubar", "rural", "urial", "aurar", "goier"]
        .iter()
        .map(|w| w.to_word());

    let words = Words::of(guesses, secrets, English);
    let allowed: Vec<_> = words.guess_indices();
    let secret_count = words.secret_count() as f64;
    let cache = Cache::new(&words);
    let guessed = vec![];
    let scores = fewest_remaining_solutions(&words, &words.secret_indices(), &guessed, &cache);

    assert_eq!(scores[0], (allowed[0], 6.0 / secret_count));
    assert_eq!(scores[1], (allowed[1], 6.0 / secret_count));
    assert_eq!(scores[2], (allowed[2], 6.0 / secret_count));
    assert_eq!(scores[3], (allowed[3], 6.0 / secret_count));
    assert_eq!(scores[4], (allowed[4], 6.0 / secret_count));
    assert_eq!(scores[5], (allowed[5], 7.0 / secret_count));
    assert_eq!(scores[6], (allowed[6], 7.0 / secret_count));
    assert_eq!(scores[7], (allowed[7], 7.0 / secret_count));
    assert_eq!(scores[8], (allowed[8], 9.0 / secret_count));
    assert_eq!(scores[9], (allowed[9], 9.0 / secret_count));
}
