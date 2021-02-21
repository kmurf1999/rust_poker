use crossbeam::atomic::AtomicCell;

use fastdivide::DividerU64;
use std::cmp::Ordering;
use std::error::Error;
use std::result::Result;
use std::sync::Arc;
use std::sync::RwLock;

use rand::distributions::{Distribution, Uniform};
use rand::rngs::SmallRng;
use rand::{thread_rng, Rng, SeedableRng};

use super::CombinedRange;
use crate::constants::CARD_COUNT;
use crate::hand_evaluator::{evaluate, Hand, CARDS};
use crate::hand_range::HandRange;

// use super::combined_range::CombinedRange;

const MIN_PLAYERS: usize = 2;
const MAX_PLAYERS: usize = 6;
const BOARD_CARDS: u32 = 5;

/// Runs a monte carlo simulation to calculate range vs range equity
///
/// Returns the equity for each player
///
/// # Arguments
///
/// * `hand_ranges` Array of hand ranges
/// * `board_mask` 64 bit mask of public cards
/// * `n_threads` Number of threads to use in simulation
/// * 'stdev_target` Target std deviation for simulation
///
/// # Example
/// ```
/// use rust_poker::hand_range::{HandRange, get_card_mask};
/// use rust_poker::equity_calculator::approx_equity;
/// let ranges = HandRange::from_strings(["random".to_string(), "random".to_string()].to_vec());
/// let board_mask = get_card_mask("");
/// let equities = approx_equity(&ranges, board_mask, 4, 0.001);
/// ```
pub fn approx_equity(
    hand_ranges: &[HandRange],
    board_mask: u64,
    n_threads: u8,
    stdev_target: f64,
) -> Result<Vec<f64>, Box<dyn Error>> {
    assert!(hand_ranges.len() >= MIN_PLAYERS && hand_ranges.len() <= MAX_PLAYERS);
    assert!(board_mask.count_ones() <= BOARD_CARDS);

    let mut rng = thread_rng();

    let n_players = hand_ranges.len();

    let mut hand_ranges = hand_ranges.to_owned();
    hand_ranges
        .iter_mut()
        .for_each(|h| h.remove_conflicting_combos(board_mask));

    let mut combined_ranges = CombinedRange::from_ranges(&hand_ranges);
    for i in 0..combined_ranges.len() {
        assert!(combined_ranges[i].size() > 0);
        // if using monte carlo
        combined_ranges[i].shuffle(&mut rng);
    }

    let sim = Arc::new(Simulator::new(
        combined_ranges,
        board_mask,
        n_players,
        stdev_target,
    ));

    // spawn threads
    crossbeam::scope(|scope| {
        for _ in 0..n_threads {
            let sim = Arc::clone(&sim);
            let mut rng = SmallRng::from_rng(&mut rng).unwrap();
            scope.spawn(move |_| {
                sim.sim_random_walk_monte_carlo(&mut rng);
            });
        }
    })
    .unwrap();

    // get results and calculate equity
    let results = sim.results.read().unwrap();
    let mut equity = vec![0f64; n_players];
    let mut equity_sum = 0f64;

    for i in 0..n_players {
        equity[i] += results.wins[i];
        equity[i] += results.ties[i];
        equity_sum += equity[i];
    }
    for i in 0..n_players {
        equity[i] /= equity_sum;
    }

    Ok(equity)
}

/// stores total results of the simulation
#[derive(Debug)]
pub struct SimulationResults {
    wins: Vec<f64>,
    ties: Vec<f64>,
    wins_by_mask: Vec<f64>,
    eval_count: u64,
    batch_sum: f64,
    batch_sum2: f64,
    batch_count: f64,
    stdev: f64,
}

impl SimulationResults {
    fn init(n_players: usize) -> SimulationResults {
        SimulationResults {
            wins: vec![1f64; n_players],
            ties: vec![1f64; n_players],
            wins_by_mask: vec![1f64; 1 << n_players],
            eval_count: 0,
            batch_count: 0f64,
            batch_sum: 0f64,
            batch_sum2: 0f64,
            stdev: 0f64,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct HandWithIndex {
    pub cards: (u8, u8, u8),
    pub player_idx: usize,
}

impl Default for HandWithIndex {
    fn default() -> Self {
        HandWithIndex {
            cards: (52, 52, 100),
            player_idx: 0,
        }
    }
}

impl Ord for HandWithIndex {
    fn cmp(&self, other: &Self) -> Ordering {
        if (self.cards.0 >> 2) != (other.cards.0 >> 2) {
            return (self.cards.0 >> 2).cmp(&(other.cards.0 >> 2));
        }
        if (self.cards.1 >> 2) != (other.cards.1 >> 2) {
            return (self.cards.1 >> 2).cmp(&(other.cards.1 >> 2));
        }
        if (self.cards.0 & 3) != (other.cards.0 & 3) {
            return (self.cards.0 & 3).cmp(&(other.cards.0 & 3));
        }
        return (self.cards.1 & 3).cmp(&(other.cards.1 & 3));
    }
}

impl PartialOrd for HandWithIndex {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// structure to store results of a single thread
struct SimulationResultsBatch {
    wins_by_mask: Vec<f64>,
    player_ids: Vec<usize>,
    eval_count: u64,
}

impl SimulationResultsBatch {
    fn init(n_players: usize) -> SimulationResultsBatch {
        let mut player_ids = vec![0usize; n_players];
        for i in 0..n_players {
            player_ids[i] = i;
        }
        SimulationResultsBatch {
            wins_by_mask: vec![1f64; 1 << n_players],
            player_ids,
            eval_count: 0,
        }
    }
}

/// equity calculator main structure
struct Simulator {
    combined_ranges: Vec<CombinedRange>,
    board_mask: u64,
    fixed_board: Hand,
    n_players: usize,
    max_combo_weight: f64,
    stopped: AtomicCell<bool>, // is stopped
    results: RwLock<SimulationResults>,
    stdev_target: f64,
}

impl Simulator {
    fn new(
        combined_ranges: Vec<CombinedRange>,
        board_mask: u64,
        n_players: usize,
        stdev_target: f64,
    ) -> Simulator {
        let fixed_board = Hand::from_bit_mask(board_mask);
        Simulator {
            combined_ranges,
            board_mask,
            fixed_board,
            max_combo_weight: (100f64).powf(n_players as f64),
            n_players,
            stopped: AtomicCell::new(false),
            results: RwLock::new(SimulationResults::init(n_players)),
            stdev_target,
        }
    }

    fn enumerate_all(&self) {
        let mut enum_pos = 0u64;
        let mut enum_end = 0u64;
        let mut stats = SimulationResultsBatch::init(self.n_players);
        let fast_dividers: Vec<DividerU64> = self
            .combined_ranges
            .iter()
            .map(|c| DividerU64::divide_by(c.size() as u64))
            .collect();
        let preflop_combos = self.get_preflop_combo_count();
        let postflop_combos = self.get_postflop_combo_count();
        let use_lookup = postflop_combos > 500;

        // let randomize_order = postflop_combos > 10000 && preflop_combos <= 2 * MAX_LOOKUP_SIZE;
        loop {
            enum_pos += 1;
            if enum_pos >= enum_end {
                let batch_size = std::cmp::max(2000000 / postflop_combos, 1);
                let (e, p) = self.reserve_batch(batch_size);
                enum_end = e;
                enum_pos = p;
                if enum_pos >= enum_end {
                    break;
                }
            }

            let mut ok = true;
            let mut used_cards_mask = self.board_mask;
            let mut player_hands = [HandWithIndex::default(); MAX_PLAYERS];
            for i in 0..self.combined_ranges.len() {
                let quotient = fast_dividers[i].divide(enum_pos);
                let remainder = enum_pos - quotient * self.combined_ranges[i].size() as u64;
                let random_enum_pos = quotient;
                let combo = &self.combined_ranges[i].combos()[remainder as usize];
                if (used_cards_mask & combo.mask) != 0 {
                    ok = false;
                    break;
                }
                used_cards_mask |= combo.mask;
                for j in 0..self.combined_ranges[i].player_count() {
                    let player_idx = self.combined_ranges[i].players()[j];
                    player_hands[player_idx].cards = combo.hole_cards[player_idx];
                    player_hands[player_idx].player_idx = player_idx;
                }
            }

            if ok {
                let mut board_mask = self.board_mask;
                if use_lookup {
                    player_hands[0..self.n_players].sort();
                    for i in 0..self.n_players {
                        stats.player_ids[i] = player_hands[i].player_idx;
                    }
                    self.transform_suits(&mut player_hands, self.n_players, &mut board_mask);
                } else {
                }
            }
        }
    }

    fn transform_suits(
        &self,
        player_hands: &mut [HandWithIndex],
        n_players: usize,
        board_mask: &mut u64,
    ) {
    }

    fn reserve_batch(&self, batch_size: u64) -> (u64, u64) {
        (0, 0)
    }

    fn get_preflop_combo_count(&self) -> u64 {
        let mut combo_count = 1u64;
        for c in &self.combined_ranges {
            combo_count *= c.size() as u64;
        }
        combo_count
    }

    fn get_postflop_combo_count(&self) -> u64 {
        let mut cards_in_deck = u64::from(CARD_COUNT);
        cards_in_deck -= u64::from(self.board_mask.count_ones());
        cards_in_deck -= 2 * self.n_players as u64;
        let board_cards_remaining = 5 - u64::from(self.board_mask.count_ones());
        let mut postflop_combos = 1u64;
        for i in 0..board_cards_remaining {
            postflop_combos *= cards_in_deck - i;
        }
        for i in 0..board_cards_remaining {
            postflop_combos /= i + 1;
        }
        postflop_combos
    }

    fn sim_random_walk_monte_carlo<R: Rng>(&self, rng: &mut R) {
        let mut batch = SimulationResultsBatch::init(self.n_players);
        let card_dist: Uniform<u8> = Uniform::from(0..CARD_COUNT);
        let combo_dists: Vec<Uniform<usize>> = (0..self.combined_ranges.len())
            .into_iter()
            .map(|i| Uniform::from(0..self.combined_ranges[i].size()))
            .collect();
        let combined_range_dist = Uniform::from(0..self.combined_ranges.len());
        let mut used_cards_mask = 0u64;
        let mut player_hands = [Hand::default(); MAX_PLAYERS];
        let mut combo_indexes = [0usize; MAX_PLAYERS];
        let mut combo_weights = [1u8; MAX_PLAYERS];
        let cards_remaining = 5 - self.fixed_board.count();

        if self.randomize_hole_cards(
            &mut used_cards_mask,
            &mut combo_indexes,
            &mut player_hands,
            &mut combo_weights,
            rng,
            &combo_dists,
        ) {
            loop {
                let mut board = self.fixed_board;
                let mut weight = 1u64;
                for c in &combo_weights {
                    weight *= u64::from(*c);
                }
                randomize_board(
                    rng,
                    &mut board,
                    used_cards_mask,
                    cards_remaining,
                    &card_dist,
                );
                self.evaluate_hands(&player_hands, weight, &board, &mut batch);

                if (batch.eval_count & 0xfff) == 0 {
                    self.update_results(&batch, false);
                    if self.stopped.load() {
                        break;
                    }
                    batch = SimulationResultsBatch::init(self.n_players);
                    if !self.randomize_hole_cards(
                        &mut used_cards_mask,
                        &mut combo_indexes,
                        &mut player_hands,
                        &mut combo_weights,
                        rng,
                        &combo_dists,
                    ) {
                        break;
                    }
                }

                let combined_range_idx = combined_range_dist.sample(rng);
                let combined_range = &self.combined_ranges[combined_range_idx];
                let mut combo_idx = combo_indexes[combined_range_idx];
                used_cards_mask -= combined_range.combos()[combo_idx].mask;
                let mut mask;
                loop {
                    if combo_idx == 0 {
                        combo_idx = combined_range.size();
                    }
                    combo_idx -= 1;
                    mask = combined_range.combos()[combo_idx].mask;
                    if (mask & used_cards_mask) == 0 {
                        break;
                    }
                }
                used_cards_mask |= mask;
                for i in 0..combined_range.player_count() {
                    let player_idx = combined_range.players()[i];
                    player_hands[player_idx] = combined_range.combos()[combo_idx].hands[i];
                    combo_weights[player_idx] = combined_range.combos()[combo_idx].hole_cards[i].2;
                }
                combo_indexes[combined_range_idx] = combo_idx;
            }
        }
        self.update_results(&batch, true);
    }

    fn randomize_hole_cards<R: Rng>(
        &self,
        used_cards_mask: &mut u64,
        combo_indexes: &mut [usize],
        player_hands: &mut [Hand],
        combo_weights: &mut [u8],
        rng: &mut R,
        combo_dists: &[Uniform<usize>],
    ) -> bool {
        let mut ok;
        for _ in 0..1000 {
            ok = true;
            *used_cards_mask = self.board_mask;
            for i in 0..self.combined_ranges.len() {
                let combo_idx = combo_dists[i].sample(rng);
                combo_indexes[i] = combo_idx;
                let combo = &self.combined_ranges[i].combos()[combo_idx];
                if (*used_cards_mask & combo.mask) != 0 {
                    ok = false;
                    break;
                }
                for j in 0..self.combined_ranges[i].player_count() {
                    let player_idx = self.combined_ranges[i].players()[j];
                    player_hands[player_idx] = combo.hands[j];
                    combo_weights[player_idx] = combo.hole_cards[j].2;
                }
                *used_cards_mask |= combo.mask;
            }
            if ok {
                return true;
            }
        }
        false
    }

    // simulate monte carlo simulation
    // produces results batch
    fn sim_monte_carlo<R: Rng>(&self, rng: &mut R) {
        let mut batch = SimulationResultsBatch::init(self.n_players);
        let card_dist: Uniform<u8> = Uniform::from(0..CARD_COUNT);
        let combo_dists: Vec<Uniform<usize>> = (0..self.combined_ranges.len())
            .into_iter()
            .map(|i| Uniform::from(0..self.combined_ranges[i].size()))
            .collect();
        let cards_remaining = 5 - self.fixed_board.count();

        loop {
            let mut player_hands = vec![Hand::default(); self.n_players];
            // player_hand_indexes = Vec::with_capacity(self.n_players);
            let mut weights = vec![1u64; self.n_players];
            let mut used_cards_mask = self.board_mask;
            let mut ok = true;

            for i in 0..self.combined_ranges.len() {
                let combo_idx = combo_dists[i].sample(rng);
                let combo = self.combined_ranges[i].combos()[combo_idx];
                if (combo.mask & used_cards_mask) != 0 {
                    ok = false;
                    break;
                }
                for j in 0..self.combined_ranges[i].player_count() {
                    let player_idx = self.combined_ranges[i].players()[j];
                    player_hands[player_idx] = combo.hands[j];
                    weights[player_idx] = u64::from(combo.hole_cards[j].2);
                }
                used_cards_mask |= combo.mask;
            }
            let mut combo_weight = 1u64;
            for w in &weights {
                combo_weight *= *w;
            }
            if !ok {
                continue;
            }
            let mut board: Hand = self.fixed_board;
            randomize_board(
                rng,
                &mut board,
                used_cards_mask,
                cards_remaining,
                &card_dist,
            );
            self.evaluate_hands(&player_hands, combo_weight, &board, &mut batch);

            if batch.eval_count >= 10000 {
                // update results
                self.update_results(&batch, false);
                batch = SimulationResultsBatch::init(self.n_players);
                if self.stopped.load() {
                    break;
                }
            }
        }

        self.update_results(&batch, true);
    }

    fn update_results(&self, batch: &SimulationResultsBatch, finished: bool) {
        // get lock
        let mut results = self.results.write().unwrap();
        let mut batch_hands = 0f64;
        let mut batch_equity = 0f64;
        for i in 0..(1 << self.n_players) {
            let winner_count: u32 = (i as u32).count_ones();
            batch_hands += batch.wins_by_mask[i];
            let mut actual_player_mask = 0;
            for j in 0..self.n_players {
                if (i & (1 << j)) != 0 {
                    if winner_count == 1 {
                        results.wins[batch.player_ids[j]] += batch.wins_by_mask[i];
                        if batch.player_ids[j] == 0 {
                            batch_equity += batch.wins_by_mask[i];
                        }
                    } else {
                        results.ties[batch.player_ids[j]] +=
                            batch.wins_by_mask[i] / winner_count as f64;
                        if batch.player_ids[j] == 0 {
                            batch_equity += batch.wins_by_mask[i] / winner_count as f64;
                        }
                    }
                    actual_player_mask |= 1 << batch.player_ids[j];
                }
            }
            results.wins_by_mask[actual_player_mask] += batch.wins_by_mask[i];
        }
        batch_equity /= batch_hands + 1e-9;

        results.eval_count += batch.eval_count;
        results.batch_sum += batch_equity;
        results.batch_sum2 += batch_equity * batch_equity;
        results.batch_count += 1.0;

        results.stdev = (1e-9 + results.batch_sum2
            - results.batch_sum * results.batch_sum / results.batch_count)
            .sqrt()
            / results.batch_count;

        // calc variance
        if !finished && results.stdev < self.stdev_target {
            self.stopped.store(true);
        }
    }

    fn evaluate_hands(
        &self,
        player_hands: &[Hand],
        weight: u64,
        board: &Hand,
        results: &mut SimulationResultsBatch,
    ) {
        // evaulate hands
        let mut winner_mask: u8 = 0;
        let mut best_score: u16 = 0;
        let mut player_mask: u8 = 1;
        for i in 0..self.n_players {
            // one-hot-encoded player
            let hand: Hand = *board + player_hands[i];
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
        results.wins_by_mask[usize::from(winner_mask)] += weight as f64 / self.max_combo_weight;
        results.eval_count += 1;
    }
}

fn randomize_board<R: Rng>(
    rng: &mut R,
    board: &mut Hand,
    mut used_cards_mask: u64,
    cards_remaining: u32,
    card_dist: &Uniform<u8>,
) {
    // randomize board
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
        let ranges = HandRange::from_strings(["KK".to_string(), "AA@1,QQ".to_string()].to_vec());
        let equity = approx_equity(&ranges, 0, THREADS, 0.001).unwrap();
        assert!(equity[0] > 0.811 - ERROR);
        assert!(equity[0] < 0.811 + ERROR);
    }

    #[test]
    fn test_preflop_accuracy() {
        const ERROR: f64 = 0.01;
        const THREADS: u8 = 4;
        let ranges = HandRange::from_strings(["88".to_string(), "random".to_string()].to_vec());
        let equity = approx_equity(&ranges, 0, THREADS, 0.001).unwrap();
        println!("{:?}", equity);
        assert!(equity[0] > 0.6916 - ERROR);
        assert!(equity[0] < 0.6916 + ERROR);
    }

    #[bench]
    fn bench_random_random(b: &mut Bencher) {
        // best score with these params
        // 1,892,190 ns/iter (+/- 176,130)
        const ERROR: f64 = 0.05;
        const THREADS: u8 = 4;
        let ranges = HandRange::from_strings(["random".to_string(), "random".to_string()].to_vec());
        let board_mask = get_card_mask("");
        b.iter(|| {
            let equity = approx_equity(&ranges, board_mask, THREADS, 0.001).unwrap();
            assert!(equity[0] > 0.5 - ERROR);
            assert!(equity[0] < 0.5 + ERROR);
        });
    }
}
