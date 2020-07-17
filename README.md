# RustPoker

Fast range vs. range equity calculation for poker written in rust

 - [Docs](https://docs.rs/rust_poker/0.1.0/rust_poker/)
 - [Crates.io](https://crates.io/crates/rust_poker)

## Hand Evaluator
 - Evaluates hands with any number of cards from 0 to 7
 - Higher score is better

### Example

```rust
use rust_poker::equity_calculator;
use rust_poker::hand_range::HandRange;
let n_threads = 4;
let n_games = 10000;
```



## Equity Calculator
 - Runs a multithreaded monte-carlo simulation to calculate range vs range equities
 - Supports up to 6 players

# Credit

The hand evaluator and equity calculator library is a rust rewrite of **zekyll's** C++ equity calculator, [OMPEval](https://github.com/zekyll/OMPEval)
