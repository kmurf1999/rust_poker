#![feature(test)]

/// # Rust Poker
/// A poker library
///
/// Currently supports
///  - monte carlo range vs. range equity calculations
///  - fast hand evaluation
///
/// ## Equity Calculator
///
/// ```
/// use rust_poker::hand_range::{HandRange, get_card_mask};
/// use rust_poker::equity_calculator::calc_equity;
/// let ranges = HandRange::from_strings(["AK,22+".to_string(), "random".to_string()].to_vec());
/// let public_cards = get_card_mask("2h3d4c");
/// let n_games = 10000;
/// let n_threads = 4;
/// let equities = calc_equity(&ranges, public_cards, n_threads, n_games);
/// ```
///
/// ## Hand Evaluator
///
/// ```
/// use rust_poker::hand_evaluator::{Hand, CARDS, evaluate};
/// // cards are indexed 0->51 where index is 4 * rank + suit
/// let hand = Hand::empty() + CARDS[0] + CARDS[1];
/// let score = evaluate(&hand);
/// ```

#[macro_use]
extern crate lazy_static;
extern crate test;
extern crate rand;
extern crate crossbeam;

mod hand_indexer;

pub use hand_indexer::hand_indexer_s;

pub mod hand_range;
pub mod constants;
pub mod hand_evaluator;

pub mod equity_calculator;
