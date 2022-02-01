# Wordle helper
Helps with smart word suggestions when playing the [Wordle](https://www.powerlanguage.co.uk/wordle/) game. It can also automatically evaluate different strategies or combinations thereof. 

And it's an excuse for me to mess around with Rust. This code is still a work in progress and rather messy, I wouldn't recommend using it unless you're me. 😉

That said, you might find the [word lists](data/word_lists) or [full decision trees](data/full_decision_trees) helpful.

## Interactive mode (`play()`):
A typical run (with lots of debug output) looks like this:

```sh
❯ cargo run --release

2315 words left
Best (fewest remaining solutions): 139883 roate, 141217 raise, 141981 raile, 144227 soare, 147525 arise
Enter your guess, or press enter to use the suggestion 'roate':

Enter feedback using upper-case for correct and lower-case for wrong positions,
or any non-alphabetic for illegal:
r
Inserting 'r' as illegal @ 0
Inserting globally illegal char 'o'
Inserting globally illegal char 'a'
Inserting globally illegal char 't'
Inserting globally illegal char 'e'

63 words left
Best (fewest remaining solutions): 1496 ureic, 1695 urite, 1705 urine, 1723 curie, 1817 arise
Enter your guess, or press enter to use the suggestion 'ureic':

Enter feedback using upper-case for correct and lower-case for wrong positions,
or any non-alphabetic for illegal:
_R_ic
Inserting 'r' as correct character @ 1
Inserting 'i' as illegal @ 3
Inserting 'c' as illegal @ 4
Inserting globally illegal char 'u'
Inserting globally illegal char 'e'

5 words left: brick, crick, crimp, crisp, prick
Words with most of wanted chars {'s', 'k', 'b', 'm', 'p'} are:
  4 skimp, 4 kemps, 4 kembs, 4 bumps, 3 zimbs
Best (fewest remaining solutions): 8 cripe, 10 crimp, 11 prick, 11 price, 11 crisp
Enter your guess, or press enter to use the suggestion 'cripe':
skimp
Enter feedback using upper-case for correct and lower-case for wrong positions,
or any non-alphabetic for illegal:
__IMP
Inserting 'i' as correct character @ 2
Inserting 'm' as correct character @ 3
Inserting 'p' as correct character @ 4
Inserting globally illegal char 's'
Inserting globally illegal char 'k'

The only word left in the list is 'crimp'
```

### Interactive mode in German
```sh
❯ cargo run --release german

1171 words left
Best (fewest remaining solutions): 36655 'raine', 41291 'taler', 42405 'raten', 42461 'laser', 42897 'reale'
Enter your guess, or press enter to use the suggestion 'raine':

Enter feedback using upper-case for correct and lower-case for wrong positions,
or any non-alphabetic for illegal:
__INE
Inserting 'i' as correct character @ 2
Inserting 'n' as correct character @ 3
Inserting 'e' as correct character @ 4
Inserting globally illegal char 'r'
Inserting globally illegal char 'a'

2 words left: 'deine', 'leine'
Enter your guess, or press enter to use the suggestion 'deine':
     
Enter feedback using upper-case for correct and lower-case for wrong positions,
or any non-alphabetic for illegal:
DEINE
Inserting 'd' as correct character @ 0
Inserting 'e' as correct character @ 1
Inserting 'i' as correct character @ 2
Inserting 'n' as correct character @ 3
Inserting 'e' as correct character @ 4

The word is 'deine'
```

## Simulation mode (`autoplay()`):
A typical simulation might produce output like the following text, but for all 2315 possible solutions:
```
2315 solutions left, 1. guess 'roate', hint '⬛⬛🟩⬛⬛', secret 'aback'
  71 solutions left, 2. guess 'slick', hint '⬛⬛⬛🟩🟩', secret 'aback'
   4 solutions left, 3. guess 'aback', hint '🟩🟩🟩🟩🟩', secret 'aback'

…

2315 solutions left, 1. guess 'roate', hint ⬛⬛🟨🟨⬛, secret 'taunt'
  50 solutions left, 2. guess 'clint', hint ⬛⬛⬛🟩🟩, secret 'taunt'
   6 solutions left, 3. guess 'dight', hint ⬛⬛⬛⬛🟩, secret 'taunt'
   3 solutions left, 4. guess 'juves', hint ⬛🟨⬛⬛⬛, secret 'taunt'
   1 solutions left, 5. guess 'taunt', hint 🟩🟩🟩🟩🟩, secret 'taunt'

…

2315 solutions left, 1. guess 'roate', hint '⬛🟩🟨⬛⬛', secret 'zonal'
  16 solutions left, 2. guess 'liman', hint '🟨⬛⬛🟩🟨', secret 'zonal'
   1 solutions left, 3. guess 'zonal', hint '🟩🟩🟩🟩🟩', secret 'zonal'

Average attempts = 3.547; 2: 39, 3: 1015, 4: 1216, 5: 45
```

There is also a full (around 8k lines!) [full example here](data/simulation_example.txt).