#![feature(test)]

extern crate bytepack;

mod hand;
mod evaluator;

pub use evaluator::evaluate;
pub use hand::{Hand, CARDS};
