# Wordle helper
Helps with suggestions when playing a wordle game, can automatically try strategies etc.

Very messy, do not use 😉

A typical run (with lots of debug output) looks like this:

```
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

Part of a typical simulation looks like this:
```
Fixed guesses ["roate", "linds", "chump", "gawky"]

2315 solutions left, 1. guess 'roate', hint '⬛⬛🟩⬛⬛', secret 'aback'
  71 solutions left, 2. guess 'linds', hint '⬛⬛⬛⬛⬛', secret 'aback'
   6 solutions left, 3. guess 'chump', hint '🟨⬛⬛⬛⬛', secret 'aback'
After 3 guesses: The only word left in the list is 'aback'

…

2315 solutions left, 1. guess 'roate', hint '⬛⬛🟨🟨⬛', secret 'taunt'
  50 solutions left, 2. guess 'linds', hint '⬛⬛🟨⬛⬛', secret 'taunt'
   6 solutions left, 3. guess 'chump', hint '⬛⬛🟩⬛⬛', secret 'taunt'
   4 solutions left, 4. guess 'gawky', hint '⬛🟩⬛⬛⬛', secret 'taunt'
   3 solutions left, 5. guess 'vaunt', hint '⬛🟩🟩🟩🟩', secret 'taunt'
   2 solutions left, 6. guess 'taunt', hint '🟩🟩🟩🟩🟩', secret 'taunt'
After 6 guesses: The word is 'taunt'

…

2315 solutions left, 1. guess 'roate', hint '⬛🟩🟨⬛⬛', secret 'zonal'
  16 solutions left, 2. guess 'linds', hint '🟨⬛🟩⬛⬛', secret 'zonal'
After 2 guesses: The only word left in the list is 'zonal'

Average attempts = 3.066; 0 (0.000%) failed games (> 6 attempts):
1: 23, 2: 518, 3: 1156, 4: 522, 5: 93, 6: 3
```

[Full example](data/simulation_example.txt)
