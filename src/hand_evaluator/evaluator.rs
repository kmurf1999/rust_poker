use super::hand;
use crate::constants::*;

use std::num::Wrapping;
use std::fs::File;
use std::iter::repeat;
use bytepack::{LEPacker, LEUnpacker};

// should recalculate perfhash offsets
const RECALCULATE_OFFSETS: bool = false;
// filename to write and read perf hash offset table
const HASH_OFFSETS_FILENAME: &str = "offset_table.dat";

// divide value by 4096 to obtain the hand category
const HAND_CATEGORY_OFFSET: u16 = 0x1000;
const HAND_CATEGORY_SHIFT: u8 = 12;

// Hand Categories
const HIGH_CARD: u16 =       1 * HAND_CATEGORY_OFFSET;
const PAIR: u16 =            2 * HAND_CATEGORY_OFFSET;
const TWO_PAIR: u16 =        3 * HAND_CATEGORY_OFFSET;
const THREE_OF_A_KIND: u16 = 4 * HAND_CATEGORY_OFFSET;
const STRAIGHT: u16 =        5 * HAND_CATEGORY_OFFSET;
const FLUSH: u16 =           6 * HAND_CATEGORY_OFFSET;
const FULL_HOUSE: u16 =      7 * HAND_CATEGORY_OFFSET;
const FOUR_OF_A_KIND: u16 =  8 * HAND_CATEGORY_OFFSET;
const STRAIGHT_FLUSH: u16 =  9 * HAND_CATEGORY_OFFSET;

// minimum number of cards to populate table with
const MIN_CARDS: u8 = 2;
const MAX_CARDS: u8 = 7;

const PERF_HASH_ROW_SHIFT: usize = 12;
const PERF_HASH_COLUMN_MASK: usize = (1 << PERF_HASH_ROW_SHIFT) - 1;

// max rank key e.g. AAAAKKK
const MAX_KEY: usize = (4 * RANKS[12] + 3 * RANKS[11]) as usize + 1;

// calculated using perfect hashing
const RANK_TABLE_SIZE: usize = 86362;
const FLUSH_TABLE_SIZE: usize = 8192;

fn read_perf_hash_file(filename: &str) -> Vec<u32> {
    let mut file = File::open(filename).unwrap();
    let num_samples : u32 = file.unpack().unwrap();
    let mut samples : Vec<u32> = repeat(0u32).take(num_samples as usize).collect();
    file.unpack_exact(&mut samples[..]).unwrap();
    return samples;
}


/**
 * Used for building lookup table
 * returns key for 64-bit group of ranks
 */
fn get_key(ranks: u64, flush: bool) -> usize {
    let mut key: u64 = 0;
    for r in 0..RANK_COUNT {
        key += ((ranks >> r * 4) & 0xf)
            * (if flush { FLUSH_RANKS[usize::from(r)] } else { RANKS[usize::from(r)] });
    }
    return key as usize;
}

pub fn evaluate(hand: &hand::Hand) -> u16 {
    return LOOKUP_TABLE.evaluate(hand);
}

// create global static evaluator
lazy_static! {
    static ref LOOKUP_TABLE: Evaluator = Evaluator::init();
}

struct Evaluator {
    orig_lookup: Vec<u16>,
    rank_table: Vec<u16>,
    flush_table: Vec<u16>,
    perf_hash_offsets: Vec<u32>
}

impl Evaluator {
    pub fn init() -> Self {

        let rank_table: Vec<u16>;
        let orig_lookup: Vec<u16>;
        let perf_hash_offsets: Vec<u32>;

        if RECALCULATE_OFFSETS {
            rank_table = vec![0; MAX_KEY + 1];
            orig_lookup = vec![0; MAX_KEY + 1];
            perf_hash_offsets = vec![0; 100000];
        } else {
            orig_lookup = Vec::with_capacity(0);
            rank_table = vec![0; RANK_TABLE_SIZE];
            perf_hash_offsets = read_perf_hash_file(HASH_OFFSETS_FILENAME)
        }

        let mut eval = Evaluator {
            orig_lookup,
            perf_hash_offsets,
            rank_table,
            flush_table: vec![0; FLUSH_TABLE_SIZE],
        };

        // init lookup table
        eval.static_init();

        if RECALCULATE_OFFSETS {
            eval.recalculate_perfect_hash_offsets();
        }


        return eval;
    }

    pub fn evaluate(&self, hand: &hand::Hand) -> u16 {
        if hand.has_flush() {
            return self.flush_table[hand.get_flush_key()];
        } else {
            return self.rank_table[self.perf_hash(hand.get_rank_key())];
        }
    }

    fn perf_hash(&self, key: usize) -> usize {
        // works because of overflow
        return (Wrapping(key as u32) + Wrapping(self.perf_hash_offsets[key >> PERF_HASH_ROW_SHIFT])).0 as usize
    }

    fn static_init(&mut self) {
        let rc = RANK_COUNT;

        // println!("ADDING HIGH CARD");
        let mut hand_value: u16 = HIGH_CARD;
        self.populate(0, 0, &mut hand_value, rc, 0, 0, 0, false);

        // println!("ADDING PAIRS");
        hand_value = PAIR;
        for r in 0..rc {
            // 2u64 << 4 * rank, means pair for each rank
            self.populate(2u64 << 4 * r, 2, &mut hand_value, rc, 0, 0, 0, false);
        }

        // println!("ADDING TWO PAIRS");
        hand_value = TWO_PAIR;
        for r1 in 0..rc {
            for r2 in 0..r1 {
                // (2u64 << 4 * r1) + (2u64 << 4 * r2)
                // each two pair combination
                self.populate((2u64 << 4 * r1) + (2u64 << 4 * r2), 4, &mut hand_value, rc, r2, 0, 0, false);
            }
        }

        // println!("ADDING THREE OF A KINDS");
        hand_value = THREE_OF_A_KIND;
        for r in 0..rc {
            // each three of a kind combo
            self.populate(3u64 << 4 * r, 3, &mut hand_value, rc, 0, r, 0, false);
        }

        // println!("ADDING STRAIGHTS");
        hand_value = STRAIGHT;
        // A-5
        self.populate(0x1000000001111u64, 5, &mut hand_value, rc, rc, rc, 3, false);
        for r in 4..rc {
            // every other straight
            self.populate(0x11111u64 << 4 * (r - 4), 5, &mut hand_value, rc, rc, rc, r, false);
        }

        // println!("ADDING flush_tableES");
        hand_value = FLUSH;
        self.populate(0, 0, &mut hand_value, rc, 0, 0, 0, true);

        // println!("ADDING FULL HOUSES");
        hand_value = FULL_HOUSE;
        for r1 in 0..rc {
            for r2 in 0..rc {
                if r2 != r1 {
                    // r1's full of r2
                    self.populate((3u64 << 4 * r1) + (2u64 << 4 * r2), 5, &mut hand_value, rc, r2, r1, rc, false);
                }
            }
        }

        // println!("ADDING FOUR OF A KINDS");
        hand_value = FOUR_OF_A_KIND;
        for r in 0..rc {
            self.populate(4u64 << 4 * r, 4, &mut hand_value, rc, rc, rc, rc, false);
        }

        // println!("ADDING STRAIGHT flush_table");
        hand_value = STRAIGHT_FLUSH;
        // A-5
        self.populate(0x1000000001111u64, 5, &mut hand_value, rc, 0, 0, 3, true);
        for r in 4..rc {
            self.populate(0x11111u64 << 4 * (r - 4), 5, &mut hand_value, rc, 0, 0, r, true);
        }
    }

    fn populate(&mut self, ranks: u64, n_cards: u8, hand_value: &mut u16,
                end_rank: u8, max_pair: u8, max_trips: u8,
                max_straight: u8, flush: bool) {

        // only increment counter for 0-5 card combos
        if (n_cards <= 5) && (n_cards >= MIN_CARDS) {
            *hand_value += 1;
        }

        // write hand value to lookup table
        if (n_cards >= MIN_CARDS) || (flush && n_cards >= 5) {
            let key = get_key(ranks, flush);

            if flush {
                self.flush_table[key] = *hand_value;
            } else {
                if RECALCULATE_OFFSETS {
                    self.orig_lookup[key] = *hand_value;
                } else {
                    // Can't call perf_hash again
                    // it will generate second borrow
                    self.rank_table[
                        (Wrapping(key as u32)
                         + Wrapping(self.perf_hash_offsets[key >> PERF_HASH_ROW_SHIFT])).0 as usize
                    ] = *hand_value;

                }
            }

            if n_cards == MAX_CARDS {
                return;
            }
        }

        // iterate next card rank
        for r in 0..end_rank {
            let new_ranks = ranks + (1u64 << (4 * r));
            // check that hand doesn't improve
            let rank_count = (new_ranks >> (r * 4)) & 0xf;

            if (rank_count == 2) && (r >= max_pair) {
                continue;
            }
            if (rank_count == 3) && (r >= max_trips) {
                continue;
            }
            if rank_count >= 4 {
                // cant be more than 1 pair of quads for each rank
                continue;
            }
            if Evaluator::get_biggest_straight(new_ranks) > max_straight {
                continue;
            }

            self.populate(new_ranks, n_cards + 1, hand_value,
                    r + 1, max_pair, max_trips,
                    max_straight, flush);
        }
        return;
    }

    // return index of highest straight card or 0 when no straight
    fn get_biggest_straight(ranks: u64) -> u8 {
        let rank_mask: u64 = (0x1111111111111 & ranks) | (0x2222222222222 & ranks) >> 1 | (0x4444444444444 & ranks) >> 2;
        for i in (0..9).rev() {
            if ((rank_mask >> 4 * i) & 0x11111u64) == 0x11111u64 {
                return i + 4;
            }
        }
        if (rank_mask & 0x1000000001111) == 0x1000000001111 {
            return 3;
        }
        return 0;
    }

    fn recalculate_perfect_hash_offsets(&mut self) {

        let mut rows: Vec<(usize, Vec<usize>)> = Vec::new();
        for i in 0..(MAX_KEY + 1) {
            if self.orig_lookup[i] != 0 {
                let row_idx = i >> PERF_HASH_ROW_SHIFT;
                if row_idx >= rows.len() {
                    rows.resize(row_idx + 1, (0, Vec::new()));
                }
                rows[row_idx].1.push(i);
            }
        }

        for i in 0..rows.len() {
            rows[i].0 = i;
        }

        rows.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

        let mut max_idx = 0usize;
        for i in 0..rows.len() {
            let mut offset = 0usize;
            loop {
                let mut ok = true;
                for x in &rows[i].1 {
                    let val = self.rank_table[(*x & PERF_HASH_COLUMN_MASK) + offset];
                    if val != 0 && val != self.orig_lookup[*x] {
                        ok = false;
                        break;
                    }
                }
                if ok {
                    break;
                }
                offset += 1;
            }

            self.perf_hash_offsets[rows[i].0] = (offset as i32 - (rows[i].0 << PERF_HASH_ROW_SHIFT) as i32) as u32;

            for key in &rows[i].1 {
                let new_idx = (*key & PERF_HASH_COLUMN_MASK) + offset;
                max_idx = if new_idx > max_idx { new_idx } else { max_idx };
                self.rank_table[new_idx] = self.orig_lookup[*key];
            }
        }

        // write perf_hash_offsets to file
        // let fullpath = env::var("PWD").unwrap() + "/static/" + HASH_OFFSETS_FILENAME;
        let mut file = File::create(HASH_OFFSETS_FILENAME).unwrap();
        file.pack(rows.len() as u32).unwrap();
        file.pack_all(&self.perf_hash_offsets[0..rows.len()]).unwrap();

        // free memory
        self.rank_table.resize(max_idx + 1, 0);
        self.orig_lookup = Vec::with_capacity(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test::Bencher;

    #[bench]
    fn bench_lookup(b: &mut Bencher) {
        let hand = hand::Hand::empty()
            + hand::CARDS[0]
            + hand::CARDS[1];
        b.iter(|| evaluate(&hand));
    }

    #[test]
    fn test_2222() {
        let hand = hand::Hand::empty()
            + hand::CARDS[0]
            + hand::CARDS[1]
            + hand::CARDS[2]
            + hand::CARDS[3];
        assert_eq!(8, evaluate(&hand) >> HAND_CATEGORY_SHIFT);
        assert_eq!(32769, evaluate(&hand));
    }
}

