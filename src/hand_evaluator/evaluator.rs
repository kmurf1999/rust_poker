use super::hand;

use read_write::unpack_vec_from_asset;

use rust_embed::RustEmbed;
use std::num::Wrapping;
use std::fs::File;
use std::io::prelude::*;
use std::slice;
use std::mem::{transmute, size_of, forget};
use std::io::{Write, Result, Error, ErrorKind};

/// Must point to same directory as `gen_eval_table`
#[derive(RustEmbed)]
#[folder = "$OUT_DIR/assets"]
struct Asset;

/// filename to write and read perf hash offset table
const PERF_HASH_FILENAME: &str = "h_eval_offsets.dat";
const RANK_TABLE_FILENAME: &str = "h_eval_rank_table.dat";
const FLUSH_TABLE_FILENAME: &str = "h_eval_flush_table.dat";

const PERF_HASH_ROW_SHIFT: usize = 12;

trait Packer {
    fn write_vec_to_file<T>(&mut self, data: &Vec<T>) -> Result<()>;
    fn read_vec_from_file<T>(&mut self) -> Result<Vec<T>>;
}

impl Packer for File {
    fn write_vec_to_file<T>(&mut self, data: &Vec<T>) -> Result<()> {
        unsafe {
            self.write_all(slice::from_raw_parts(transmute::<*const T, *const u8>(data.as_ptr()), data.len() * size_of::<T>()))?;
        }
        Ok(())
    }
    fn read_vec_from_file<T>(&mut self) -> Result<Vec<T>> {
        let mut buffer: Vec<T> = Vec::new();
        let length = buffer.len();
        let capacity = buffer.capacity();
        unsafe {
            let mut converted = Vec::<u8>::from_raw_parts(buffer.as_mut_ptr() as *mut u8, length * size_of::<T>(), capacity * size_of::<T>());
            match self.read_to_end(&mut converted) {
                Ok(size) => {
                    if converted.len() % size_of::<T>() != 0 {
                        converted.truncate(length * size_of::<T>());
                        forget(converted);
                        return Err(Error::new(
                            ErrorKind::UnexpectedEof,
                            format!("read_file() returned a number of bytes ({}) which is not a multiple of size ({})", size, size_of::<T>())
                        ));
                    }
                },
                Err(e) => {
                    converted.truncate(length * size_of::<T>());
                    forget(converted);
                    return Err(e);
                }
            }
            let new_length = converted.len() / size_of::<T>();
            let new_capacity = converted.len() / size_of::<T>();
            buffer = Vec::from_raw_parts(converted.as_mut_ptr() as *mut T, new_length, new_capacity);
            forget(converted);
            Ok(buffer)
        }
    }
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
            rank_table: unpack_vec_from_asset::<u16>(Asset::get(RANK_TABLE_FILENAME)).unwrap(),
            flush_table: unpack_vec_from_asset::<u16>(Asset::get(FLUSH_TABLE_FILENAME)).unwrap(),
            perf_hash_offsets: unpack_vec_from_asset::<u32>(Asset::get(PERF_HASH_FILENAME)).unwrap()
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
