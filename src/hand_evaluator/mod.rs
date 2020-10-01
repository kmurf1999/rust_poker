extern crate bytepack;
extern crate byteorder;

mod hand;
mod evaluator;

pub use evaluator::evaluate;
pub use hand::{Hand, CARDS};
