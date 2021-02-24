#![feature(test)]

/// # Rust Poker
/// A texas holdem poker library
///
/// Currently supports
///  - monte carlo range vs. range equity calculations
///  - full enumeration for exact equities
///  - fast hand evaluation
///
/// ## Equity Calculator
///
/// ```
/// use rust_poker::hand_range::{HandRange, get_card_mask};
/// use rust_poker::equity_calculator::approx_equity;
/// let ranges = HandRange::from_strings(["AK,22+".to_string(), "AA,KK,QQ@50".to_string()].to_vec());
/// let public_cards = get_card_mask("2h3d4c");
/// let std_dev_target = 0.01;
/// let n_threads = 4;
/// let equities = approx_equity(&ranges, public_cards, n_threads, std_dev_target);
/// ```
///
/// ## Hand Evaluator
///
/// ```
/// use rust_poker::hand_evaluator::{Hand, CARDS, evaluate};
/// // cards are indexed 0->51 where index is 4 * rank + suit
/// let hand = Hand::default() + CARDS[0] + CARDS[1];
/// let score = evaluate(&hand);
/// ```

#[macro_use]
extern crate lazy_static;
extern crate crossbeam;
extern crate rand;
extern crate serde;
extern crate serde_json;
extern crate test;
// extern crate rust_embed;

#[cfg(all(feature = "indexer"))]
extern crate hand_indexer;
#[cfg(all(feature = "indexer"))]
pub use hand_indexer::{HandIndex, HandIndexer};

pub use read_write;

pub mod constants;
pub mod hand_evaluator;
pub mod hand_range;
pub mod range_filter;

pub mod equity_calculator;
