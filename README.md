# RustPoker

Fast range vs. range equity calculation for poker written in rust

## Hand Evaluator
 - Evaluates hands with any number of cards from 0 to 7
 - Higher score is better

### Usage

rust```
use rust_poker::evaluator::{evaluate, Hand, CARDS};

let hand = Hand::empty() + CARDS[0] + CARDS[1];
let score = evaluate(&hand);
```

# Credit

The hand evaluator and equity calculator library is a rust rewrite of **zekyll's** C++ equity calculator, [OMPEval](https://github.com/zekyll/OMPEval)
