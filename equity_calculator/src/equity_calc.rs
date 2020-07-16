use crossbeam::atomic::AtomicCell;

use std::sync::RwLock;
use std::sync::Arc;

use rand::distributions::{Uniform};
use rand::{SeedableRng, thread_rng, Rng};
use rand::rngs::SmallRng;
use rand::seq::SliceRandom;

use hand_evaluator::{evaluate, Hand, CARDS};
use common::{CARD_COUNT, HandRange};

use crate::combined_range::CombinedRange;

const MAX_PLAYERS: usize = 6;

// structure to store results of a single thread
struct ResultsBatch {
    wins_by_mask: [u32; 64],
    // wins_by_hand: [f64; CARD_TABLE_SIZE],
    n_games: u32
}

impl ResultsBatch {
    fn new() -> ResultsBatch {
        ResultsBatch {
            wins_by_mask: [0; 64],
            n_games: 0,
            // wins_by_hand: [0f64; CARD_TABLE_SIZE],
        }
    }
}

// stores total results of simulation
pub struct Results {
    wins: [u32; MAX_PLAYERS],
    ties: [f64; MAX_PLAYERS],
    wins_by_mask: [u32; 64],
    n_games: u64,
    // wins_by_hand: [f64; CARD_TABLE_SIZE],
}

impl Results {
    fn new() -> Results {
        Results {
            wins: [0; MAX_PLAYERS],
            ties: [0.0; MAX_PLAYERS],
            wins_by_mask: [0; 64],
            n_games: 0,
            // wins_by_hand: [0f64; CARD_TABLE_SIZE],
        }
    }
}

// equity calculator main structure
pub struct EquityCalc {
    // hand_ranges: Vec<CardRange>,
    combined_ranges: Vec<CombinedRange>,
    board_mask: u64,
    fixed_board: Hand,
    n_players: usize,
    stopped: AtomicCell<bool>, // is stopped
    results: RwLock<Results>,
    sim_count: u64 // target number of games
}

impl EquityCalc {
    fn new(hand_ranges: &Vec<HandRange>, board_mask: u64, sim_count: u64) -> EquityCalc {

        let mut hand_ranges = hand_ranges.to_owned();
        remove_invalid_combos(&mut hand_ranges, board_mask);

        EquityCalc {
            // hand_ranges: hand_ranges.to_vec(),
            combined_ranges: CombinedRange::from_ranges(&hand_ranges),
            board_mask: board_mask,
            fixed_board: get_board_from_bit_mask(board_mask),
            n_players: hand_ranges.len(),
            stopped: AtomicCell::new(false),
            results: RwLock::new(Results::new()),
            sim_count: sim_count
        }
    }
    /**
     * @param n_games: min number of simulated games to run
     */
    pub fn start(hand_ranges: &Vec<HandRange>, board_mask: u64, n_threads: u8, sim_count: u64) -> Vec<f64> {

        let sim = Arc::new(EquityCalc::new(hand_ranges, board_mask, sim_count));

        // TODO really bad way to do this,
        // but if ranges are overlapping completely return 50% equity
        for c in &sim.combined_ranges {
            if c.combos.len() == 0 {
                panic!("invalid params");
            }
        }

        let mut rng = thread_rng();

        crossbeam::scope(|scope| {
            for _ in 0..n_threads {
                let sim = Arc::clone(&sim);
                let mut rng = SmallRng::from_rng(&mut rng).unwrap();
                scope.spawn(move |_| {
                    sim.run_monte_carlo(&mut rng);
                });
            }
        }).unwrap();

        // get results
        let results =  sim.results.read().unwrap();

        // calc equities
        let mut equities = vec![0.0; sim.n_players];
        for i in 0..sim.n_players {
            equities[i] += results.wins[i] as f64;
            equities[i] += results.ties[i];
            equities[i] /= results.n_games as f64;
        }
        return equities;
    }


    // fn sim_random_walk<R: Rng>(&self, rng: &mut R) {
    //     // self.fixed board
    //     let mut batch = ResultsBatch::new();
    //     let card_dist: Uniform<u8> = Uniform::from(0..CARD_COUNT);
    //     let combined_range_dist = Uniform::from(0..self.combined_ranges.len());
    //     let mut combo_dists: Vec<Uniform<usize>> =
    //         Vec::with_capacity(self.combined_ranges.len());
    //     for i in 0..self.combined_ranges.len() {
    //         combo_dists.push(Uniform::from(0..self.combined_ranges[i].combos.len()));
    //     }
    //     let mut used_cards_mask: u64 = 0;
    //     let mut player_hands: [Option<Hand>; MAX_PLAYERS] = [None; MAX_PLAYERS];
    //     let mut combos_indexes: [usize; MAX_PLAYERS] = [0; MAX_PLAYERS];

    //     if self.randomize_hole_cards(&mut used_cards_mask, &mut combos_indexes, &mut player_hands, rng, &combo_dists) {
    //         loop {
    //             let mut board = self.fixed_board;
    //             randomize_board(rng, &mut board, used_cards_mask, &card_dist);
    //             self.evaluate_hands(&mut batch, &board, &player_hands);

    //             // update stats
    //             if (batch.n_games & 0xfff) == 0 {
    //                 // update results
    //                 self.update_results(&batch, false);
    //                 batch = ResultsBatch::new();
    //                 if self.stopped.load() == true {
    //                     break;
    //                 }

    //                 // full refresh
    //                 self.randomize_hole_cards(&mut used_cards_mask,
    //                         &mut combos_indexes, &mut player_hands,
    //                         rng, &combo_dists);
    //             }

    //             // change one player hand
    //             let c_range_idx = rng.sample(combined_range_dist);
    //             let c_range = &self.combined_ranges[c_range_idx];
    //             let mut combo_idx = combos_indexes[c_range_idx];
    //             used_cards_mask -= c_range.combos[combo_idx].card_mask;

    //             let mut mask: u64;
    //             loop {
    //                 if combo_idx == 0 {
    //                     combo_idx = c_range.combos.len();
    //                 }
    //                 combo_idx -= 1;
    //                 mask = c_range.combos[combo_idx].card_mask;

    //                 if (mask & used_cards_mask) == 0 {
    //                     break;
    //                 }
    //             }
    //             used_cards_mask |= mask;
    //             for i in 0..c_range.players {
    //                 if c_range.combos[combo_idx].hands[i].is_some() {
    //                     player_hands[i] = c_range.combos[combo_idx].hands[i];
    //                 }
    //             }
    //             combos_indexes[c_range_idx] = combo_idx;
    //         }
    //     }
    // }

    // simulate monte carlo simulation
    // produces results batch
    fn run_monte_carlo<R: Rng>(&self, rng: &mut R) {
        let mut batch = ResultsBatch::new();
        let mut player_hands: [Option<Hand>; MAX_PLAYERS];
        let mut used_cards_mask: u64;
        let mut ok: bool;
        let mut board: Hand;
        let update_interval = if self.sim_count < 10000 { 0xff } else { 0xfff };

        let card_dist: Uniform<u8> = Uniform::from(0..CARD_COUNT);

        loop {
            player_hands = [None; MAX_PLAYERS];
            used_cards_mask = 0;
            ok = true;
            for c in &self.combined_ranges {
                let combo = &c.combos.choose(rng).unwrap();
                if (combo.card_mask & used_cards_mask) != 0 {
                    ok = false;
                    break;
                }
                for i in 0..self.n_players {
                    if combo.hands[i].is_some() {
                        player_hands[i] = combo.hands[i];
                    }
                }
                used_cards_mask |= combo.card_mask;
            }
            if !ok {
                continue;
            }

            board = self.fixed_board;
            randomize_board(rng, &mut board,
                    self.board_mask | used_cards_mask, &card_dist);
            self.evaluate_hands(&mut batch, &board, &player_hands);

            if (batch.n_games & update_interval) == 0 {
                // update results
                self.update_results(&batch, false);
                batch = ResultsBatch::new();
                if self.stopped.load() == true {
                    break;
                }
            }
        }

        self.update_results(&batch, true);

    }

    /**
     * Randomize hole cards using rejection sampling
     */
    // fn randomize_hole_cards<R: Rng>(&self, used_cards_mask: &mut u64,
    //         combo_indexes: &mut [usize; MAX_PLAYERS],
    //         player_hands: &mut [Option<Hand>; MAX_PLAYERS],
    //         rng: &mut R, combo_dists: &Vec<Uniform<usize>>) -> bool {

    //     let mut n = 0;
    //     let mut ok = false;
    //     while ok == false && n < 1000 {
    //         ok = true;
    //         *used_cards_mask = self.board_mask;
    //         for i in 0..self.combined_ranges.len() {
    //             let combo_idx: usize = rng.sample(combo_dists[i]);
    //             combo_indexes[i] = combo_idx;
    //             let combo = self.combined_ranges[i].combos[combo_idx];
    //             if (*used_cards_mask & combo.card_mask) != 0 {
    //                 ok = false;
    //                 break;
    //             }
    //             for j in 0..self.n_players {
    //                 if combo.hands[j].is_some() {
    //                     player_hands[j] = combo.hands[j];
    //                 }
    //             }
    //             *used_cards_mask |= combo.card_mask;
    //         }
    //         n += 1;
    //     }
    //     return n < 1000;
    // }

    fn update_results(&self, batch: &ResultsBatch, finished: bool) {
        // let mut batch_equity: f64 = 0.0;
        // let mut batch_hands = 0;
        // get lock
        let mut results = self.results.write().unwrap();

        // copy wins by hand
        // for i in 0..CARD_TABLE_SIZE {
        //     results.wins_by_hand[i] += batch.wins_by_hand[i];
        // }
        //
        for i in 0..(1 << self.n_players) {
            // batch_hands += batch.wins_by_mask[i];
            // number of winners
            let winner_count = (i as u8).count_ones();
            let mut player_mask: usize = 0;
            for j in 0..self.n_players {
                // player id is j
                if (i & (1 << j)) != 0 {
                    if winner_count == 1 {
                        // update total wins for player j
                        results.wins[j] += batch.wins_by_mask[i];
                        // if j == 0 {
                        //     // count equity for first player
                        //     batch_equity += batch.wins_by_mask[i] as f64;
                        // }
                    } else {
                        results.ties[j] += batch.wins_by_mask[i] as f64
                            / winner_count as f64;
                        // if j == 0 {
                        //     // count equity for first player
                        //     batch_equity += batch.wins_by_mask[i] as f64
                        //         / winner_count as f64;
                        // }
                    }
                    player_mask |= 1 << j;
                }
            }
            results.wins_by_mask[player_mask] += batch.wins_by_mask[i];
        }
        results.n_games += batch.n_games as u64;

        // batch_equity = batch_equity / batch_hands as f64;

        // calc variance
        if !finished {
            // results.n_batches += 1;
            // results.equity_sum += batch_equity;
            // results.equity_sum_sq += batch_equity * batch_equity;

            // let std_dev = (1e-9 + results.equity_sum_sq - results.equity_sum * results.equity_sum / results.n_batches as f64).sqrt() / results.n_batches as f64;

            if results.n_games > self.sim_count {
                // if std_dev < STD_TARGET {
                self.stopped.store(true);
                // }
            }
        }
    }

    fn evaluate_hands(&self, results: &mut ResultsBatch, board: &Hand, player_hands: &[Option<Hand>; MAX_PLAYERS]) {
        // evaulate hands
        let mut winner_mask: u8 = 0;
        let mut best_score: u16 = 0;
        let mut player_mask: u8 = 1;
        for i in 0..self.n_players {
            // one-hot-encoded player
            let hand: Hand = *board + player_hands[i].unwrap();
            let score = evaluate(&hand);
            // println!("{} {} {} {:#066b}", i, score, score / 4096, hand.get_mask());
            if score > best_score {
                // add to wins by hand mask
                best_score = score;
                winner_mask = player_mask;
            } else if score == best_score {
                winner_mask |= player_mask;
            }
            player_mask <<= 1;
        }
        // let winner_count: f64 = winner_mask.count_ones() as f64;
        // for i in 0..self.n_players {
        //     if (winner_mask & (1 << i)) != 0 {
        //         results.wins_by_hand[pair(hand_to_tuple(&player_hands[i as usize].unwrap())) as usize] += 1.0 / winner_count;
        //     }
        // }
        results.wins_by_mask[usize::from(winner_mask)] += 1;
        results.n_games += 1;
    }
}

fn randomize_board<R: Rng>(rng: &mut R, board: &mut Hand,
        mut used_cards_mask: u64, card_dist: &Uniform<u8>) {
    // randomize board
    let cards_remaining = 5 - board.count();
    for _ in 0..cards_remaining {
        let mut card: u8;
        let mut card_mask: u64;
        loop {
            card = rng.sample(card_dist);
            card_mask = 1u64 << card;
            if (used_cards_mask & card_mask) == 0 {
                break;
            }
        }
        used_cards_mask |= card_mask;
        *board += CARDS[usize::from(card)];
    }
}

// construct a Hand object from board mask
fn get_board_from_bit_mask(mask: u64) -> Hand {
    let mut board = Hand::empty();
    for c in 0..usize::from(CARD_COUNT) {
        if (mask & (1u64 << c)) != 0 {
            board += CARDS[c];
        }
    }
    return board;
}

// remove combos from hand ranges that conflict with board
pub fn remove_invalid_combos(ranges: &mut Vec<HandRange>, board_mask: u64) {
    for r in ranges {
        r.hands.retain(|x| (((1u64 << x.0) | (1u64 << x.1)) & board_mask) == 0);
    }
}

// creates a unique id for two cards
// http://szudzik.com/ElegantPairing.pdf
// fn pair(h: (u32, u32)) -> u32 {
//     if h.0 < h.1 {
//         return (h.1 * h.1) + h.0;
//     } else {
//         return (h.0 * h.0) + h.0 + h.1;
//     }
// }

// returns the cards from a paired id
// fn unpair(z: u32) -> (u32, u32) {
//     let sqrt_floor = (z as f64).sqrt() as u32;
//     if (z - (sqrt_floor * sqrt_floor)) < sqrt_floor {
//         return (z - (sqrt_floor * sqrt_floor), sqrt_floor);
//     } else {
//         return (sqrt_floor, z - (sqrt_floor * sqrt_floor) - sqrt_floor);
//     }
// }

// hole cards to tuple (0->51, 0->52)
// fn hand_to_tuple(h: &Hand) -> (u32, u32) {
//     let mask = h.get_mask();
//     let c1_pos = mask.trailing_zeros();
//     let c2_pos = 63 - mask.leading_zeros();
//     let c1 = 4 * (c1_pos % 16) + (3 - (c1_pos / 16));
//     let c2 = 4 * (c2_pos % 16) + (3 - (c2_pos / 16));
//     if c1 > c2 {
//         return (c1, c2);
//     } else {
//         return (c2, c1);
//     }
// }

#[cfg(test)]
mod tests {
    use super::*;
    use common::{ HandRange, get_card_mask };
    use test::Bencher;

    #[test]
    fn test_remove_invalid_combos() {
        let mut ranges = HandRange::from_str_arr(["AA", "random"].to_vec());
        let board_mask = get_card_mask("Ah2s3c");
        assert_eq!(ranges[0].hands.len(), 6);
        remove_invalid_combos(&mut ranges, board_mask);
        assert_eq!(ranges[0].hands.len(), 3);
    }

    #[bench]
    fn bench_random_random(b: &mut Bencher) {
        // best score with these params
        // 2,900,681 ns/iter
        const ERROR: f64 = 0.01;
        const THREADS: u8 = 4;
        const SIM_COUNT: u64 = 30000;
        let ranges = HandRange::from_str_arr(["random", "random"].to_vec());
        let board_mask = get_card_mask("");
        b.iter(|| {
            let eq = EquityCalc::start(&ranges, board_mask, THREADS, SIM_COUNT);
            assert!(eq[0] >  0.5 - ERROR);
            assert!(eq[0] <  0.5 + ERROR);
        });
    }

    #[test]
    fn test_get_board_from_bit_mask() {
        // 4-bit rank groups
        let board_mask = get_card_mask("2s2h2d");
        // 16 bit suit groups
        let board = get_board_from_bit_mask(board_mask);

        assert_eq!(0b1000000000000000100000000000000010000000000000000, board.get_mask());
        assert_eq!(0b111, board_mask);
    }
}
