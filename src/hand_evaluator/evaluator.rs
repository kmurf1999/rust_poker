use super::hand;

use read_write::VecIO;

// use rust_embed::RustEmbed;

use std::num::Wrapping;
use std::fs::File;
use std::env;

/// Must point to same directory as `gen_eval_table`
// #[derive(RustEmbed)]
// #[folder = "eval_table"]
// struct Asset;

/// filename to write and read perf hash offset table
// const OUT_DIR: &str = "STATIC_ASSET_DIR";
// const PERF_HASH_FILENAME: &str = "h_eval_offsets.dat";
// const RANK_TABLE_FILENAME: &str = "h_eval_rank_table.dat";
// const FLUSH_TABLE_FILENAME: &str = "h_eval_flush_table.dat";

const PERF_HASH_ROW_SHIFT: usize = 12;

/// Evaluates a single hand and returns score
pub fn evaluate(hand: &hand::Hand) -> u16 {
    LOOKUP_TABLE.evaluate(hand)
}

lazy_static! {
    /// Global static lookup table used for evaluation
    static ref LOOKUP_TABLE: Evaluator = Evaluator::load();
}

/// Singleton structure
struct Evaluator {
    /// Stores scores of non flush hands
    rank_table: Vec<u16>,
    /// Stores scores of flush hands
    flush_table: Vec<u16>,
    /// Stores offsets to rank table
    perf_hash_offsets: Vec<u32>,
}

impl Evaluator {
    pub fn load() -> Self {
        let perf_hash_file = concat!(env!("OUT_DIR"), "/h_eval_offsets.dat");
        let flush_table_file = concat!(env!("OUT_DIR"), "/h_eval_flush_table.dat");
        let rank_table_file = concat!(env!("OUT_DIR"), "/h_eval_rank_table.dat");
        Self {
            rank_table: File::open(rank_table_file)
                .unwrap()
                .read_vec_from_file::<u16>().unwrap(),
            flush_table: File::open(flush_table_file)
                .unwrap()
                .read_vec_from_file::<u16>().unwrap(),
            perf_hash_offsets: File::open(perf_hash_file)
                .unwrap()
                .read_vec_from_file::<u32>().unwrap(),
        }
    }

    pub fn evaluate(&self, hand: &hand::Hand) -> u16 {
        if hand.has_flush() {
            self.flush_table[hand.get_flush_key()]
        } else {
            self.rank_table[self.perf_hash(hand.get_rank_key())]
        }
    }

    fn perf_hash(&self, key: usize) -> usize {
        // works because of overflow
        (Wrapping(key as u32) + Wrapping(self.perf_hash_offsets[key >> PERF_HASH_ROW_SHIFT])).0
            as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::HAND_CATEGORY_SHIFT;
    use test::Bencher;

    #[bench]
    fn bench_lookup(b: &mut Bencher) {
        let hand = hand::Hand::empty() + hand::CARDS[0] + hand::CARDS[1];
        b.iter(|| evaluate(&hand));
    }

    #[test]
    fn test_2222() {
        let hand =
            hand::Hand::empty() + hand::CARDS[0] + hand::CARDS[1] + hand::CARDS[2] + hand::CARDS[3];
        assert_eq!(8, evaluate(&hand) >> HAND_CATEGORY_SHIFT);
        assert_eq!(32769, evaluate(&hand));
    }
}
