# RustPoker

[![Build Status](https://travis-ci.org/kmurf1999/rust_poker.svg?branch=master)](https://travis-ci.org/kmurf1999/rust_poker)
[![docs.rs](https://docs.rs/rust_poker/badge.svg)](https://docs.rs/rust_poker)
[![crates.io](https://img.shields.io/crates/v/rust_poker.svg)](https://crates.io/crates/rust_poker)

A poker library written in rust.

 - Multithreaded range vs range equity calculation
 - Fast hand evaluation
 - Efficient hand indexing


## Installation

Add this to your `Cargo.toml`:
```
[dependencies]
rust_poker = "0.1.7"
```
**Note**: The first build of an application using `rust_poker` will take extra time to generate the hand evaluation table

## Hand Evaluator

Evaluates the strength of any poker hand using up to 7 cards.

### Usage

```rust
use rust_poker::hand_evaluator::{Hand, CARDS, evaluate};
// cards are indexed 0->51 where index is 4 * rank + suit
let hand = Hand::empty() + CARDS[0] + CARDS[1];
let score = evaluate(&hand);
println!("score: {}", score);
```

## Equity Calculator

Calculates the range vs range equities for up to 6 different ranges specified by equilab-like range strings.

### Usage

```rust
use rust_poker::hand_range::{HandRange, get_card_mask};
use rust_poker::equity_calculator::calc_equity;
let ranges = HandRange::from_strings(["AK,22+".to_string(), "random".to_string()].to_vec());
let public_cards = get_card_mask("2h3d4c".to_string());
let n_games = 10000;
let n_threads = 4;
let equities = calc_equity(&ranges, public_cards, n_threads, n_games);
println!("player 1 equity: {}", equities[0]);
```

# Credit

The hand evaluator and equity calculator library is a rust rewrite of **zekyll's** C++ equity calculator, [OMPEval](https://github.com/zekyll/OMPEval)
