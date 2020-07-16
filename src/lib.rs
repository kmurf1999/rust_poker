#![feature(test)]

#[macro_use]
extern crate lazy_static;
extern crate test;
extern crate rand;
extern crate crossbeam;


mod hand_indexer;
pub use hand_indexer::hand_indexer_t;

pub mod hand_range;
pub mod constants;
pub mod hand_evaluator;
pub mod equity_calculator;
// pub use common;
// pub use hand_evaluator;
// pub use equity_calculator;
