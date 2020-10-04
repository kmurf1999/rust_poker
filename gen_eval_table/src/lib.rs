#![allow(clippy::too_many_arguments)]

extern crate read_write;

use read_write::VecIO;

use std::env;
use std::fs::File;
use std::io::Result;
use std::path::Path;

const HAND_CATEGORY_OFFSET: u16 = 0x1000;
// const HAND_CATEGORY_SHIFT: u8 = 12;
const RANK_COUNT: u8 = 13;
const MIN_CARDS: u8 = 2;
const MAX_CARDS: u8 = 7;

const PERF_HASH_ROW_SHIFT: usize = 12;
const PERF_HASH_COLUMN_MASK: usize = (1 << PERF_HASH_ROW_SHIFT) - 1;

const HIGH_CARD: u16 = HAND_CATEGORY_OFFSET;
const PAIR: u16 = 2 * HAND_CATEGORY_OFFSET;
const TWO_PAIR: u16 = 3 * HAND_CATEGORY_OFFSET;
const THREE_OF_A_KIND: u16 = 4 * HAND_CATEGORY_OFFSET;
const STRAIGHT: u16 = 5 * HAND_CATEGORY_OFFSET;
const FLUSH: u16 = 6 * HAND_CATEGORY_OFFSET;
const FULL_HOUSE: u16 = 7 * HAND_CATEGORY_OFFSET;
const FOUR_OF_A_KIND: u16 = 8 * HAND_CATEGORY_OFFSET;
const STRAIGHT_FLUSH: u16 = 9 * HAND_CATEGORY_OFFSET;

/// Tables of unique primes for hashing hands
pub const RANKS: &[u64; 13] = &[
    8192, 32769, 69632, 237568, 593920, 1531909, 3563520, 4300819, 4685870, 4690024, 4767972,
    4780561, 4801683,
];
/// Table of power of 2 flush ranks
pub const FLUSH_RANKS: &[u64; 13] = &[1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096];

const PERF_HASH_FILENAME: &str = "h_eval_offsets.dat";
const RANK_TABLE_FILENAME: &str = "h_eval_rank_table.dat";
const FLUSH_TABLE_FILENAME: &str = "h_eval_flush_table.dat";

const FLUSH_TABLE_SIZE: usize = 8192;
// const RANK_TABLE_SIZE: usize = 86362;

const MAX_KEY: usize = (4 * RANKS[12] + 3 * RANKS[11]) as usize;

fn get_biggest_straight(ranks: u64) -> u8 {
    let rank_mask: u64 =
        (0x1111111111111 & ranks) | (0x2222222222222 & ranks) >> 1 | (0x4444444444444 & ranks) >> 2;
    for i in (0..9).rev() {
        if ((rank_mask >> (4 * i)) & 0x11111u64) == 0x11111u64 {
            return i + 4;
        }
    }
    if (rank_mask & 0x1000000001111) == 0x1000000001111 {
        return 3;
    }
    0
}

fn get_key(ranks: u64, flush: bool) -> usize {
    let mut key: u64 = 0;
    for r in 0..RANK_COUNT {
        key += ((ranks >> (r * 4)) & 0xf)
            * (if flush {
                FLUSH_RANKS[usize::from(r)]
            } else {
                RANKS[usize::from(r)]
            });
    }
    key as usize
}

/// return true if asset file at path exists
// fn hash_file_exists(filename: &str) -> bool {
//     let out_dir = env::var("OUT_DIR").expect("CARGO_TARGET_DIR env var for perfect hash file not set");
//     // let fullpath = Path::new(&out_dir).join(ASSET_FOLDER).join(filename);
//     let fullpath = Path::new(&out_dir).join(filename);
//     if File::open(fullpath).is_ok() {
//         return true;
//     }
//     false
// }

struct EvalTableGenerator {
    rank_table: Vec<u16>,
    flush_table: Vec<u16>,
    orig_lookup: Vec<u16>,
    perf_hash_offsets: Vec<u32>,
}

impl EvalTableGenerator {
    fn new() -> Self {
        Self {
            rank_table: vec![0u16; MAX_KEY + 1],
            flush_table: vec![0; FLUSH_TABLE_SIZE],
            orig_lookup: vec![0u16; MAX_KEY + 1],
            perf_hash_offsets: vec![0u32; 1000000],
        }
    }
    fn start(&mut self) {
        self.generate_tables();
        self.calc_perfect_hash_offsets();
        self.write_files().unwrap();
    }
    fn generate_tables(&mut self) {
        let rc = RANK_COUNT;
        let mut hand_value = HIGH_CARD;
        self.populate(0, 0, &mut hand_value, rc, 0, 0, 0, false);

        hand_value = PAIR;
        for r in 0..rc {
            // 2u64 << 4 * rank, means pair for each rank
            self.populate(2u64 << (4 * r), 2, &mut hand_value, rc, 0, 0, 0, false);
        }

        hand_value = TWO_PAIR;
        for r1 in 0..rc {
            for r2 in 0..r1 {
                // (2u64 << 4 * r1) + (2u64 << 4 * r2)
                // each two pair combination
                self.populate(
                    (2u64 << (4 * r1)) + (2u64 << (4 * r2)),
                    4,
                    &mut hand_value,
                    rc,
                    r2,
                    0,
                    0,
                    false,
                );
            }
        }

        hand_value = THREE_OF_A_KIND;
        for r in 0..rc {
            // each three of a kind combo
            self.populate(3u64 << (4 * r), 3, &mut hand_value, rc, 0, r, 0, false);
        }

        hand_value = STRAIGHT;
        // A-5
        self.populate(0x1000000001111u64, 5, &mut hand_value, rc, rc, rc, 3, false);
        for r in 4..rc {
            // every other straight
            self.populate(
                0x11111u64 << (4 * (r - 4)),
                5,
                &mut hand_value,
                rc,
                rc,
                rc,
                r,
                false,
            );
        }

        hand_value = FLUSH;
        self.populate(0, 0, &mut hand_value, rc, 0, 0, 0, true);

        // println!("ADDING FULL HOUSES");
        hand_value = FULL_HOUSE;
        for r1 in 0..rc {
            for r2 in 0..rc {
                if r2 != r1 {
                    // r1's full of r2
                    self.populate(
                        (3u64 << (4 * r1)) + (2u64 << (4 * r2)),
                        5,
                        &mut hand_value,
                        rc,
                        r2,
                        r1,
                        rc,
                        false,
                    );
                }
            }
        }

        // println!("ADDING FOUR OF A KINDS");
        hand_value = FOUR_OF_A_KIND;
        for r in 0..rc {
            self.populate(4u64 << (4 * r), 4, &mut hand_value, rc, rc, rc, rc, false);
        }

        // println!("ADDING STRAIGHT flush_table");
        hand_value = STRAIGHT_FLUSH;
        // A-5
        self.populate(0x1000000001111u64, 5, &mut hand_value, rc, 0, 0, 3, true);
        for r in 4..rc {
            self.populate(
                0x11111u64 << (4 * (r - 4)),
                5,
                &mut hand_value,
                rc,
                0,
                0,
                r,
                true,
            );
        }
    }

    fn populate(
        &mut self,
        ranks: u64,
        n_cards: u8,
        hand_value: &mut u16,
        end_rank: u8,
        max_pair: u8,
        max_trips: u8,
        max_straight: u8,
        flush: bool,
    ) {
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
                self.orig_lookup[key] = *hand_value;
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
            if get_biggest_straight(new_ranks) > max_straight {
                continue;
            }

            self.populate(
                new_ranks,
                n_cards + 1,
                hand_value,
                r + 1,
                max_pair,
                max_trips,
                max_straight,
                flush,
            );
        }
    }
    fn calc_perfect_hash_offsets(&mut self) {
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

        for (i, row) in rows.iter_mut().enumerate() {
            row.0 = i;
        }
        rows.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

        let mut max_idx = 0usize;
        for row in &rows {
            // for i in 0..rows.len() {
            let mut offset = 0usize;
            loop {
                let mut ok = true;
                for x in &row.1 {
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

            self.perf_hash_offsets[row.0] =
                (offset as i32 - (row.0 << PERF_HASH_ROW_SHIFT) as i32) as u32;

            for key in &row.1 {
                let new_idx = (*key & PERF_HASH_COLUMN_MASK) + offset;
                max_idx = if new_idx > max_idx { new_idx } else { max_idx };
                self.rank_table[new_idx] = self.orig_lookup[*key];
            }
        }
        // free_memory
        self.perf_hash_offsets.resize(rows.len(), 0);
        self.rank_table.resize(max_idx + 1, 0);
        self.orig_lookup = Vec::with_capacity(0);
    }
    fn write_files(&mut self) -> Result<()> {
        // create folder
        let out_dir = env::var("OUT_DIR").expect("OUT_DIR env var for perfect hash file not set");
        let dir = Path::new(&out_dir);
        // let dir = Path::new();
        // std::fs::create_dir(dir.clone())?;
        // write offsets
        let hash_offsets_path = dir.join(PERF_HASH_FILENAME);
        let mut hash_offsets_file = File::create(hash_offsets_path)?;
        hash_offsets_file.write_vec_to_file::<u32>(&self.perf_hash_offsets)?;
        // write rank table
        let rank_table_path = dir.join(RANK_TABLE_FILENAME);
        let mut rank_table_file = File::create(rank_table_path)?;
        rank_table_file.write_vec_to_file::<u16>(&self.rank_table)?;
        // write flush table
        let flush_table_path = dir.join(FLUSH_TABLE_FILENAME);
        let mut flush_table_file = File::create(flush_table_path)?;
        flush_table_file.write_vec_to_file::<u16>(&self.flush_table)?;

        Ok(())
    }
}

pub fn gen_eval_table() {
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR env var for perfect hash file not set");
    let fullpath = Path::new(&out_dir);

    if fullpath.join(PERF_HASH_FILENAME).exists()
    && fullpath.join(RANK_TABLE_FILENAME).exists()
    && fullpath.join(FLUSH_TABLE_FILENAME).exists() {
        return;
    }

    let mut generator = EvalTableGenerator::new();
    generator.start();
}