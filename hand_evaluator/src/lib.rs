#![feature(test)]

#[macro_use]
extern crate lazy_static;
extern crate bytepack;
extern crate test;

mod evaluator;
mod hand;
mod constants;

pub use evaluator::evaluate as evaluate;
pub use hand::{Hand, CARDS};