# RustPoker

Fast range vs. range equity calculation for poker written in rust

 - [Docs](https://docs.rs/rust_poker/0.1.0/rust_poker/)
 - [Crates.io](https://crates.io/crates/rust_poker)

## Hand Evaluator
 - Evaluates hands with any number of cards from 0 to 7
 - Higher score is better

### Usage

```rust
use rust_poker::evaluator::{Hand, CARDS, evaluate};
// cards are indexed 0->51 where index is 4 * rank + suit
let hand = Hand::empty() + CARDS[0] + CARDS[1];
let score = evaluate(&hand);
```

## Equity Calculator
 - Runs a multithreaded monte-carlo simulation to calculate range vs range equities
 - Supports up to 6 players

### Usage

```rust
use rust_poker::hand_range::{HandRange, get_card_mask};
use rust_poker::equity_calculator::EquityCalc;
let ranges = HandRange::from_str_arr(["AK,22+", "random"].to_vec());
let public_cards = get_card_mask("2h3d4c");
let n_games = 10000;
let n_threads = 4;
let equities = EquityCalc::start(&ranges, public_cards, n_threads, n_games);
```

# Credit

The hand evaluator and equity calculator library is a rust rewrite of **zekyll's** C++ equity calculator, [OMPEval](https://github.com/zekyll/OMPEval)
