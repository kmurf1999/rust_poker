use crossbeam::atomic::AtomicCell;

use std::sync::Arc;
use std::sync::RwLock;

use rand::distributions::Uniform;
use rand::rngs::SmallRng;
use rand::{thread_rng, Rng, SeedableRng};

use crate::constants::CARD_COUNT;
use crate::hand_evaluator::{evaluate, Hand, CARDS};
use crate::hand_range::HandRange;

// use super::combined_range::CombinedRange;

const MIN_PLAYERS: usize = 2;
const MAX_PLAYERS: usize = 6;

/// Runs a monte carlo simulation to calculate range vs range equity
///
/// Returns the equity for each player
///
/// # Arguments
///
/// * `hand_ranges` Array of hand ranges
/// * `board_mask` 64 bit mask of public cards
/// * `n_threads` Number of threads to use in simulation
/// * `sim_count` Number of games to simulate
///
/// # Example
/// ```
/// use rust_poker::hand_range::{HandRange, get_card_mask};
/// use rust_poker::equity_calculator::calc_equity;
/// let ranges = HandRange::from_strings(["random".to_string(), "random".to_string()].to_vec());
/// let board_mask = get_card_mask("");
/// let equities = calc_equity(&ranges, board_mask, 4, 1000);
/// ```
pub fn calc_equity(
    hand_ranges: &[HandRange],
    board_mask: u64,
    n_threads: u8,
    sim_count: u64,
) -> Vec<f64> {
    if hand_ranges.len() < MIN_PLAYERS || hand_ranges.len() > MAX_PLAYERS {
        panic!("Invalid number of hand_ranges");
    }

    let sim = Arc::new(Simulator::new(hand_ranges, board_mask, sim_count));

    // TODO really bad way to do this,
    // but if ranges are overlapping completely return 50% equity
    // for c in &sim.combined_ranges {
    //     if c.combos.len() == 0 {
    //         println!(
    //             "r1 {} r2 {} board {}",
    //             hand_ranges[0].char_vec.iter().collect::<String>(),
    //             hand_ranges[1].char_vec.iter().collect::<String>(),
    //             mask_to_string(board_mask)
    //         );
    //         return vec![0f64; sim.n_players];
    //     }
    // }

    let mut rng = thread_rng();

    crossbeam::scope(|scope| {
        for _ in 0..n_threads {
            let sim = Arc::clone(&sim);
            let mut rng = SmallRng::from_rng(&mut rng).unwrap();
            scope.spawn(move |_| {
                sim.run_monte_carlo(&mut rng);
            });
        }
    })
    .unwrap();

    // get results
    let results = sim.results.read().unwrap();

    // calc equities
    let mut equities = vec![0.0; sim.n_players];
    let mut equity_sum = 0.0;
    for i in 0..sim.n_players {
        equities[i] += results.wins[i] as f64;
        equities[i] += results.ties[i];
        equity_sum += equities[i];
    }
    for i in 0..sim.n_players {
        equities[i] /= equity_sum;
    }
    equities
}

/// structure to store results of a single thread
struct ResultsBatch {
    wins: Vec<f64>,
    ties: Vec<f64>,
    n_games: u64,
}

impl ResultsBatch {
    fn init(n_players: usize) -> ResultsBatch {
        ResultsBatch {
            wins: vec![0f64; n_players],
            ties: vec![0f64; n_players],
            n_games: 0,
        }
    }
}

/// stores total results of the simulation
pub struct Results {
    wins: Vec<f64>,
    ties: Vec<f64>,
    n_games: u64,
}

impl Results {
    fn init(n_players: usize) -> Results {
        Results {
            wins: vec![0f64; n_players],
            ties: vec![0f64; n_players],
            n_games: 0,
        }
    }
}

/// equity calculator main structure
struct Simulator {
    hand_ranges: Vec<HandRange>,
    // combined_ranges: Vec<CombinedRange>,
    board_mask: u64,
    fixed_board: Hand,
    n_players: usize,
    stopped: AtomicCell<bool>, // is stopped
    results: RwLock<Results>,
    sim_count: u64, // target number of games
}

impl Simulator {
    fn new(hand_ranges: &[HandRange], board_mask: u64, sim_count: u64) -> Simulator {
        let mut hand_ranges = hand_ranges.to_owned();
        let n_players = hand_ranges.len();
        hand_ranges
            .iter_mut()
            .for_each(|h| h.remove_conflicting_combos(board_mask));

        Simulator {
            hand_ranges,
            // combined_ranges: CombinedRange::from_ranges(&hand_ranges),
            board_mask,
            fixed_board: Hand::from_bit_mask(board_mask),
            n_players,
            stopped: AtomicCell::new(false),
            results: RwLock::new(Results::init(n_players)),
            sim_count,
        }
    }

    // simulate monte carlo simulation
    // produces results batch
    fn run_monte_carlo<R: Rng>(&self, rng: &mut R) {
        let mut batch = ResultsBatch::init(self.n_players);
        let mut player_hand_indexes: Vec<usize>;
        let mut used_cards_mask: u64;
        let mut ok: bool;
        let mut board: Hand;
        let update_interval = if self.sim_count < 10000 { 0xff } else { 0xfff };

        let card_dist: Uniform<u8> = Uniform::from(0..CARD_COUNT);

        loop {
            player_hand_indexes = Vec::with_capacity(self.n_players);
            used_cards_mask = 0;
            ok = true;

            for i in 0..self.n_players {
                let combo_idx = rng.gen_range(0, self.hand_ranges[i].hands.len());
                let combo = &self.hand_ranges[i].hands[combo_idx];
                let combo_mask = (1u64 << combo.0) | (1u64 << combo.1);
                if (combo_mask & used_cards_mask) != 0 {
                    ok = false;
                    break;
                }
                used_cards_mask |= combo_mask;
                player_hand_indexes.push(combo_idx);
            }
            if !ok {
                continue;
            }

            board = self.fixed_board;
            randomize_board(
                rng,
                &mut board,
                self.board_mask | used_cards_mask,
                &card_dist,
            );
            self.evaluate_hands(&mut batch, &board, &player_hand_indexes);

            if (batch.n_games & update_interval) == 0 {
                // update results
                self.update_results(&batch, false);
                batch = ResultsBatch::init(self.n_players);
                if self.stopped.load() {
                    break;
                }
            }
        }

        self.update_results(&batch, true);
    }

    fn update_results(&self, batch: &ResultsBatch, finished: bool) {
        // get lock
        let mut results = self.results.write().unwrap();
        for i in 0..self.n_players {
            results.wins[i] += batch.wins[i];
            results.ties[i] += batch.ties[i];
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

    fn evaluate_hands(
        &self,
        results: &mut ResultsBatch,
        board: &Hand,
        player_hand_indexes: &[usize],
    ) {
        // evaulate hands
        let mut winner_mask: u8 = 0;
        let mut best_score: u16 = 0;
        let mut player_mask: u8 = 1;
        for i in 0..self.n_players {
            // one-hot-encoded player
            let combo = &self.hand_ranges[i].hands[player_hand_indexes[i]];
            let hand: Hand = *board + CARDS[usize::from(combo.0)] + CARDS[usize::from(combo.1)];
            let score = evaluate(&hand);
            if score > best_score {
                // add to wins by hand mask
                best_score = score;
                winner_mask = player_mask;
            } else if score == best_score {
                winner_mask |= player_mask;
            }
            player_mask <<= 1;
        }
        let n_winners = winner_mask.count_ones();
        for i in 0..self.n_players {
            if ((1u8 << i) & winner_mask) != 0 {
                if n_winners == 1 {
                    results.wins[i] +=
                        self.hand_ranges[i].hands[player_hand_indexes[i]].2 as f64 / 100.0;
                } else {
                    results.ties[i] += (self.hand_ranges[i].hands[player_hand_indexes[i]].2 as f64
                        / 100.0)
                        / n_winners as f64;
                }
            }
        }
        results.n_games += 1;
    }
}

fn randomize_board<R: Rng>(
    rng: &mut R,
    board: &mut Hand,
    mut used_cards_mask: u64,
    card_dist: &Uniform<u8>,
) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hand_range::{get_card_mask, HandRange};
    use test::Bencher;

    #[test]
    fn test_weighted_ranges() {
        const ERROR: f64 = 0.01;
        const THREADS: u8 = 4;
        const SIM_COUNT: u64 = 30000;
        let ranges =
            HandRange::from_strings(["AA@50,22@100".to_string(), "QQ".to_string()].to_vec());
        let eq = calc_equity(&ranges, 0, THREADS, SIM_COUNT);
        assert!(eq[0] > 0.367 - ERROR);
        assert!(eq[0] < 0.367 + ERROR);
    }

    #[bench]
    fn bench_random_random(b: &mut Bencher) {
        // best score with these params
        // 3,900,681 ns/iter
        const ERROR: f64 = 0.01;
        const THREADS: u8 = 4;
        const SIM_COUNT: u64 = 30000;
        let ranges = HandRange::from_strings(["random".to_string(), "random".to_string()].to_vec());
        let board_mask = get_card_mask("");
        b.iter(|| {
            let eq = calc_equity(&ranges, board_mask, THREADS, SIM_COUNT);
            assert!(eq[0] > 0.5 - ERROR);
            assert!(eq[0] < 0.5 + ERROR);
        });
    }
}
