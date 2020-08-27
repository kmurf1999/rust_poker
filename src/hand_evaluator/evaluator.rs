use super::hand;

use bytepack::LEUnpacker;
use std::env;
use std::fs::File;
use std::num::Wrapping;
use std::path::Path;

/// filename to write and read perf hash offset table
const PERF_HASH_FILENAME: &str = "h_eval_offsets.dat";
const RANK_TABLE_FILENAME: &str = "h_eval_rank_table.dat";
const FLUSH_TABLE_FILENAME: &str = "h_eval_flush_table.dat";

const PERF_HASH_ROW_SHIFT: usize = 12;

fn read_vector_from_file<Precision: bytepack::Packed>(
    filename: &str,
) -> Result<Vec<Precision>, std::io::Error> {
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR env var for perfect hash file not set");
    let fullpath = Path::new(&out_dir).join(filename);
    let mut file = File::open(fullpath)?;
    let mut buffer: Vec<Precision> = Vec::new();
    file.unpack_to_end(&mut buffer).unwrap();
    Ok(buffer)
}

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
        Self {
            rank_table: read_vector_from_file::<u16>(RANK_TABLE_FILENAME).unwrap(),
            flush_table: read_vector_from_file::<u16>(FLUSH_TABLE_FILENAME).unwrap(),
            perf_hash_offsets: read_vector_from_file::<u32>(PERF_HASH_FILENAME).unwrap(),
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
    use test::Bencher;

    const HAND_CATEGORY_SHIFT: u8 = 12;

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
