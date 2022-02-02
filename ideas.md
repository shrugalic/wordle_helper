Wordle helper ideas:
--------------------
- Implement strategy that minimizes number of total guesses?
- What can be learned from finding good paths for all words? [How To Always Win Wordle](https://www.youtube.com/watch?v=Xv7JBbOiBkI)

Done:
-----
- Look at failures of certain strategies
  - Many are suboptimal when 4 characters are known, and there are multiple options for the remaining characters
- Try more strategies with auto-play
- Don't lock the know positions right away, use them to evaluate new characters, to make them must-have or illegal.
- Possibly use larger wordlist when guessing
- Re-evaluate strategies:
  - Use all allowed words as guesses, but only use the smaller solutions list to judge it.
  - There are guesses that yield more information, even though they cannot be solutions.
- Allow specifying multiple strategies (fallback)
- Combine "most unplayed characters" with other strategies, such as "least possible words left after guess"
- Store results of some calculations to not repeat them every time
- Tried many other word combinations
- Implement strategy that starts with a few good words almost regardless of outcome, such as: TUBES, FLING, CHAMP, WORDY from [Why I ALWAYS Guess the Same Four Words](https://youtu.be/l92g6Yy8t5g)
  - What are possible next words?
    - Frequencies over all combined:
      - 1505 * 'k'
      - 694 * 'v'
      - 434 * 'z'
      - 291 * 'j'
      - 288 * 'x'
      - 112 * 'q'
    - VODKA covers the next most frequent 'k' and 'v'
      - JAZZY covers the next most frequent 'z' and 'j', no follow-up
      - ZAXES instead covers 'z' and 'x', would leave QAJAQ
      - SQUIZ instead covers 'z' and 'q', would leave JAXIE
- Play the game automatically to report statistics on various strategies
  - Global most frequent chars: Average rounds 3.067; 9 (0.389%) failed games (> 6 rounds)
  - Most frequent per position: Average rounds 3.098; 7 (0.302%) failed games (> 6 rounds)
- Allow feedback in a single entry
- Parallelize slow computations (using rayon)
- Implement different strategies nicely, maybe with strategy pattern using trait?
- Implement a strategy that splits the solution space most evenly [What is the best guess for Wordle?](https://youtu.be/BN-Yan03m8s)
- Implement a strategy that results in the smallest possible pool of still possible solutions, as demonstrated in [Using Statistics To Beat Wordle](https://youtu.be/B2AVF3_qdHY)
  - The result of both is the same!

Best from all allowed words (lower is better):
- 315.13 serai
- 314.73 arles
- 311.36 rates
- 309.73 aeros
- 305.55 nares
- 304.76 reais
- 303.83 soare
- 302.49 tares
- 292.11 rales
- 288.74 lares

Best from solutions (lower is better):
- 71.57 slate   
- 71.29 stare   
- 71.10 snare   
- 70.22 later   
- 70.13 saner   
- 69.99 alter   
- 66.02 arose   
- 63.78 irate   
- 63.73 arise   
- 61.00 raise
