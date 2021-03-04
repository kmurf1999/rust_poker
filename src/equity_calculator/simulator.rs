use crossbeam::atomic::AtomicCell;

use fastdivide::DividerU64;
use std::cmp::Ordering;
use std::collections::HashMap;
use thiserror::Error;

use std::error::Error;
use std::result::Result;
use std::sync::{Arc, Mutex, RwLock};

use rand::distributions::{Distribution, Uniform};
use rand::rngs::SmallRng;
use rand::{thread_rng, Rng, SeedableRng};

use super::CombinedRange;
use crate::constants::{CARD_COUNT, RANK_MASK, SUIT_COUNT, SUIT_MASK};
use crate::hand_evaluator::{evaluate, evaluate_without_flush, Hand, CARDS};
use crate::hand_range::HandRange;

// use super::combined_range::CombinedRange;

const MIN_PLAYERS: usize = 2;
const MAX_PLAYERS: usize = 6;
const BOARD_CARDS: u32 = 5;

#[derive(Debug, Error)]
pub enum SimulatorError {
    #[error("too many players")]
    TooManyPlayers,
    #[error("too few players")]
    TooFewPlayers,
    #[error("Too many board cards")]
    TooManyBoardCards,
    #[error("conflicting ranges")]
    ConflictingRanges,
}

/// Calculates exact range vs range equities
///
/// Returns the equity for each player
///
/// # Arguments
///
/// * `hand_ranges` Array of hand ranges
/// * `board_mask` 64 bit mask of public cards
/// * `n_threads` Number of threads to use in simulation
///
/// # Example
/// ```
/// use rust_poker::hand_range::{HandRange, get_card_mask};
/// use rust_poker::equity_calculator::exact_equity;
/// let ranges = HandRange::from_strings(["AA".to_string(), "random".to_string()].to_vec());
/// let board_mask = get_card_mask("");
/// let equities = exact_equity(&ranges, board_mask, 4);
/// ```
pub fn exact_equity(
    hand_ranges: &[HandRange],
    board_mask: u64,
    n_threads: u8,
) -> Result<Vec<f64>, SimulatorError> {
    if hand_ranges.len() < MIN_PLAYERS {
        return Err(SimulatorError::TooFewPlayers);
    }
    if hand_ranges.len() > MAX_PLAYERS {
        return Err(SimulatorError::TooManyPlayers);
    }
    if board_mask.count_ones() > BOARD_CARDS {
        return Err(SimulatorError::TooManyBoardCards);
    }

    let mut hand_ranges = hand_ranges.to_owned();
    hand_ranges
        .iter_mut()
        .for_each(|h| h.remove_conflicting_combos(board_mask));
    let combined_ranges = CombinedRange::from_ranges(&hand_ranges);
    for cr in &combined_ranges {
        if cr.size() == 0 {
            return Err(SimulatorError::ConflictingRanges);
        }
    }
    let sim = Arc::new(Simulator::new(
        hand_ranges,
        combined_ranges,
        board_mask,
        true,
        0.0,
    ));
    // spawn threads
    crossbeam::scope(|scope| {
        for _ in 0..n_threads {
            let sim = Arc::clone(&sim);
            scope.spawn(move |_| {
                sim.enumerate_all();
            });
        }
    })
    .unwrap();
    // get results and calculate equity
    let results = sim.results.read().unwrap();
    Ok(results.get_equity())
}

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
) -> Result<Vec<f64>, SimulatorError> {
    if hand_ranges.len() < MIN_PLAYERS {
        return Err(SimulatorError::TooFewPlayers);
    }
    if hand_ranges.len() > MAX_PLAYERS {
        return Err(SimulatorError::TooManyPlayers);
    }
    if board_mask.count_ones() > BOARD_CARDS {
        return Err(SimulatorError::TooManyBoardCards);
    }

    let mut rng = thread_rng();
    let mut hand_ranges = hand_ranges.to_owned();
    hand_ranges
        .iter_mut()
        .for_each(|h| h.remove_conflicting_combos(board_mask));
    let mut combined_ranges = CombinedRange::from_ranges(&hand_ranges);
    for cr in &mut combined_ranges {
        if cr.size() == 0 {
            return Err(SimulatorError::ConflictingRanges);
        }
        cr.shuffle(&mut rng);
    }
    let sim = Arc::new(Simulator::new(
        hand_ranges,
        combined_ranges,
        board_mask,
        false,
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
    Ok(results.get_equity())
}

fn calculate_preflop_id(player_hands: &[HandWithIndex], n_players: usize) -> u64 {
    let mut preflop_id = 0u64;
    for hand in &player_hands[0..n_players] {
        preflop_id *= 1327;
        let mut cards = hand.cards;
        if cards.0 < cards.1 {
            std::mem::swap(&mut cards.0, &mut cards.1);
        }
        preflop_id += ((u64::from(cards.0) * u64::from(cards.0 - 1)) >> 1) + u64::from(cards.1) + 1;
    }
    preflop_id
}

/// stores total results of the simulation
#[derive(Debug)]
pub struct SimulationResults {
    wins: Vec<u64>,
    ties: Vec<f64>,
    wins_by_mask: Vec<u64>,
    eval_count: u64,
    batch_sum: f64,
    batch_sum2: f64,
    batch_count: f64,
    stdev: f64,
}

impl SimulationResults {
    fn init(n_players: usize) -> SimulationResults {
        SimulationResults {
            wins: vec![0u64; n_players],
            ties: vec![0f64; n_players],
            wins_by_mask: vec![0u64; 1 << n_players],
            eval_count: 0,
            batch_count: 0f64,
            batch_sum: 0f64,
            batch_sum2: 0f64,
            stdev: 0f64,
        }
    }
    fn get_equity(&self) -> Vec<f64> {
        let mut equity = vec![0f64; self.wins.len()];
        let mut equity_sum = 0f64;
        for i in 0..self.wins.len() {
            equity[i] += self.wins[i] as f64;
            equity[i] += self.ties[i];
            equity_sum += equity[i];
        }
        for e in &mut equity {
            *e /= equity_sum;
        }
        equity
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
        (self.cards.1 & 3).cmp(&(other.cards.1 & 3))
    }
}

impl PartialOrd for HandWithIndex {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// structure to store results of a single thread
#[derive(Debug, Copy, Clone)]
struct SimulationResultsBatch {
    wins_by_mask: [u64; 1 << MAX_PLAYERS],
    player_ids: [usize; MAX_PLAYERS],
    eval_count: u64,
}

impl SimulationResultsBatch {
    fn init(n_players: usize) -> SimulationResultsBatch {
        let mut player_ids = [0usize; MAX_PLAYERS];
        for i in 0..n_players {
            player_ids[i] = i;
        }
        SimulationResultsBatch {
            wins_by_mask: [0u64; 1 << MAX_PLAYERS],
            player_ids,
            eval_count: 0,
        }
    }
}

/// equity calculator main structure
#[derive(Debug)]
struct Simulator {
    hand_ranges: Vec<HandRange>,
    /// used to reduce rejection sampling
    combined_ranges: Vec<CombinedRange>,
    /// initial board as 64bit mask
    board_mask: u64,
    /// initial board used for evaluating
    fixed_board: Hand,
    /// number of players
    n_players: usize,
    /// has monte carlo sim stopped
    stopped: AtomicCell<bool>,
    /// final results
    results: RwLock<SimulationResults>,
    /// lookup table used for preflop combo -> results
    lookup_table: RwLock<HashMap<(u64, u64), SimulationResultsBatch>>,
    /// target stdev from each batch for monte carlo
    stdev_target: f64,
    /// should calculate exact equity
    calc_exact: bool,
    /// preflop combo position for exact equity calculation
    enum_pos: Mutex<u64>,
}

impl Simulator {
    fn new(
        hand_ranges: Vec<HandRange>,
        combined_ranges: Vec<CombinedRange>,
        board_mask: u64,
        calc_exact: bool,
        stdev_target: f64,
    ) -> Simulator {
        let fixed_board = Hand::from_bit_mask(board_mask);
        let n_players = hand_ranges.len();
        Simulator {
            hand_ranges,
            combined_ranges,
            board_mask,
            fixed_board,
            n_players,
            calc_exact,
            stopped: AtomicCell::new(false),
            enum_pos: Mutex::new(0u64),
            results: RwLock::new(SimulationResults::init(n_players)),
            lookup_table: RwLock::new(HashMap::new()),
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
        // let preflop_combos = self.get_preflop_combo_count();
        let postflop_combos = self.get_postflop_combo_count();
        let use_lookup = postflop_combos > 500;

        // let randomize_order = postflop_combos > 10000 && preflop_combos <= 2 * MAX_LOOKUP_SIZE;
        loop {
            // println!("{}", enum_pos);
            if enum_pos >= enum_end {
                let batch_size = std::cmp::max(2000000 / postflop_combos, 1);
                let (p, e) = self.reserve_batch(batch_size);
                enum_pos = p;
                enum_end = e;
                if enum_pos >= enum_end {
                    break;
                }
            }

            let mut rand_enum_pos = enum_pos;

            let mut ok = true;
            let mut used_cards_mask = self.board_mask;
            let mut player_hands = [HandWithIndex::default(); MAX_PLAYERS];
            for i in 0..self.combined_ranges.len() {
                let quotient = fast_dividers[i].divide(rand_enum_pos);
                let remainder = rand_enum_pos - quotient * self.combined_ranges[i].size() as u64;
                rand_enum_pos = quotient;
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
                let mut weight = 1u64;
                for hand in &player_hands[0..self.n_players] {
                    weight *= u64::from(hand.cards.2);
                }
                let mut board_mask = self.board_mask;
                if use_lookup {
                    player_hands[0..self.n_players].sort();
                    for i in 0..self.n_players {
                        stats.player_ids[i] = player_hands[i].player_idx;
                    }
                    self.transform_suits(&mut player_hands, self.n_players, &mut board_mask);
                    used_cards_mask = board_mask;
                    for j in 0..self.n_players {
                        used_cards_mask |=
                            (1u64 << player_hands[j].cards.0) | (1u64 << player_hands[j].cards.1);
                    }

                    let preflop_id = calculate_preflop_id(&player_hands, self.n_players);
                    if self.lookup_results((preflop_id, weight), &mut stats) {
                        for i in 0..self.n_players {
                            stats.player_ids[i] = player_hands[i].player_idx;
                        }
                        stats.eval_count = 0;
                        // stats.unique_preflop_combos = 0;
                    } else {
                        // stats.unique_preflop_combos += 1;
                        let board = Hand::from_bit_mask(board_mask);
                        self.enumerate_board(
                            &player_hands,
                            weight,
                            &board,
                            used_cards_mask,
                            &mut stats,
                        );
                        self.store_results((preflop_id, weight), &stats);
                    }
                } else {
                    // stats.unique_preflop_combos += 1;
                    self.enumerate_board(
                        &player_hands,
                        weight,
                        &self.fixed_board,
                        used_cards_mask,
                        &mut stats,
                    );
                }
            }

            if stats.eval_count >= 10000 || use_lookup {
                self.update_results(&stats, false);
                stats = SimulationResultsBatch::init(self.n_players);
                if self.stopped.load() {
                    break;
                }
            }
            enum_pos += 1;
        }

        self.update_results(&stats, true);
    }

    fn lookup_results(&self, id: (u64, u64), stats: &mut SimulationResultsBatch) -> bool {
        let table = self.lookup_table.read().unwrap();
        match table.get(&id) {
            Some(s) => {
                *stats = *s;
                true
            }
            None => false,
        }
    }

    fn store_results(&self, id: (u64, u64), stats: &SimulationResultsBatch) {
        let mut table = self.lookup_table.write().unwrap();
        table.insert(id, *stats);
    }

    fn enumerate_board(
        &self,
        player_hands: &[HandWithIndex],
        weight: u64,
        board: &Hand,
        used_cards_mask: u64,
        stats: &mut SimulationResultsBatch,
    ) {
        let mut hands = [Hand::default(); MAX_PLAYERS];
        for i in 0..self.n_players {
            hands[i] = Hand::from_hole_cards(player_hands[i].cards.0, player_hands[i].cards.1);
        }

        let cards_remaining = (BOARD_CARDS - board.count()) as u8;
        if cards_remaining == 0 {
            self.evaluate_hands(&hands, weight, board, stats, true);
            return;
        }

        let mut deck = [0u8; 52];
        let mut n_deck = 0;
        for i in (0..CARD_COUNT).rev() {
            if (used_cards_mask & (1u64 << i)) == 0 {
                deck[n_deck] = i;
                n_deck += 1;
            }
        }

        let mut suit_counts = [0u8; 4];
        for i in 0..self.n_players {
            if (player_hands[i].cards.0 & 3) == (player_hands[i].cards.1 & 3) {
                suit_counts[usize::from(player_hands[i].cards.0 & 3)] =
                    std::cmp::max(2, suit_counts[usize::from(player_hands[i].cards.0 & 3)]);
            } else {
                suit_counts[usize::from(player_hands[i].cards.0 & 3)] =
                    std::cmp::max(1, suit_counts[usize::from(player_hands[i].cards.0 & 3)]);
                suit_counts[usize::from(player_hands[i].cards.1 & 3)] =
                    std::cmp::max(1, suit_counts[usize::from(player_hands[i].cards.1 & 3)]);
            }
        }
        for i in 0..SUIT_COUNT {
            suit_counts[usize::from(i)] += board.suit_count(i) as u8;
        }

        self.enumerate_board_rec(
            &hands,
            stats,
            &board,
            &mut deck,
            n_deck,
            &mut suit_counts,
            cards_remaining,
            0,
            weight,
        );
    }

    fn enumerate_board_rec(
        &self,
        hands: &[Hand],
        stats: &mut SimulationResultsBatch,
        board: &Hand,
        deck: &mut [u8],
        n_deck: usize,
        suit_counts: &mut [u8],
        cards_remaining: u8,
        start: usize,
        weight: u64,
    ) {
        if cards_remaining == 1 {
            if (suit_counts[0] < 4)
                && (suit_counts[1] < 4)
                && (suit_counts[2] < 4)
                && (suit_counts[3] < 4)
            {
                let mut i = start;
                while i < n_deck {
                    let mut multiplier = 1;
                    let new_board = *board + CARDS[usize::from(deck[i])];
                    let rank = deck[i] >> 2;
                    i += 1;
                    while i < n_deck && deck[i] >> 2 == rank {
                        multiplier += 1;
                        i += 1;
                    }
                    self.evaluate_hands(hands, weight * multiplier, &new_board, stats, false);
                }
            } else {
                let mut last_rank = u8::MAX;
                for i in start..n_deck {
                    let mut multiplier = 1;
                    if suit_counts[usize::from(deck[i] & 3)] < 4 {
                        let rank = deck[i] >> 2;
                        if rank == last_rank {
                            continue;
                        }
                        for j in i + 1..n_deck {
                            if deck[j] >> 2 != rank {
                                break;
                            }
                            if suit_counts[usize::from(deck[j] & 3)] < 4 {
                                multiplier += 1;
                            }
                        }
                        last_rank = rank;
                    }
                    let new_board = *board + CARDS[usize::from(deck[i])];
                    self.evaluate_hands(hands, weight * multiplier, &new_board, stats, true);
                }
            }
            return;
        }
        let mut i = start;
        while i < n_deck {
            let mut new_board = *board;
            let suit = deck[i] & 3;
            if (suit_counts[usize::from(suit)] + cards_remaining) < 5 {
                let mut irrelevant_count = 1;
                let rank = deck[i] >> 2;
                for j in i + 1..n_deck {
                    if deck[j] >> 2 != rank {
                        break;
                    }
                    let suit2 = deck[j] & 3;
                    if (suit_counts[usize::from(suit2)] + cards_remaining) < 5 {
                        if j != i + irrelevant_count {
                            deck.swap(j, i + irrelevant_count);
                        }
                        irrelevant_count += 1;
                    }
                }

                for repeats in 1..std::cmp::min(irrelevant_count, usize::from(cards_remaining)) + 1
                {
                    const BINOM_COEFF: [[u64; 5]; 5] = [
                        [0, 0, 0, 0, 0],
                        [0, 1, 0, 0, 0],
                        [1, 2, 1, 0, 0],
                        [1, 3, 3, 1, 0],
                        [1, 4, 6, 4, 1],
                    ];
                    let new_weight = BINOM_COEFF[irrelevant_count][repeats] * weight;
                    new_board += CARDS[usize::from(deck[i + repeats - 1])];
                    if repeats == usize::from(cards_remaining) {
                        self.evaluate_hands(&hands, new_weight, &new_board, stats, true);
                    } else {
                        self.enumerate_board_rec(
                            hands,
                            stats,
                            &new_board,
                            deck,
                            n_deck,
                            suit_counts,
                            cards_remaining - repeats as u8,
                            i + irrelevant_count,
                            new_weight,
                        );
                    }
                }

                i += irrelevant_count - 1;
            } else {
                // new_board.mask += u64::from(deck[i]);
                new_board += CARDS[usize::from(deck[i])];
                suit_counts[usize::from(suit)] += 1;
                self.enumerate_board_rec(
                    hands,
                    stats,
                    &new_board,
                    deck,
                    n_deck,
                    suit_counts,
                    cards_remaining - 1,
                    i + 1,
                    weight,
                );
                suit_counts[usize::from(suit)] -= 1;
            }
            i += 1;
        }
    }

    fn transform_suits(
        &self,
        player_hands: &mut [HandWithIndex],
        n_players: usize,
        board_mask: &mut u64,
    ) -> u8 {
        let mut transform = [u8::MAX; 4];
        let mut suit_count = 0;
        let mut new_board_cards = 0u64;
        for i in 0..CARD_COUNT {
            if ((*board_mask >> i) & 1) != 0 {
                let suit = i & SUIT_MASK;
                if transform[usize::from(suit)] == u8::MAX {
                    transform[usize::from(suit)] = suit_count;
                    suit_count += 1;
                }
                let new_card = (i & RANK_MASK) | transform[usize::from(suit)];
                new_board_cards |= 1u64 << new_card;
            }
        }
        *board_mask = new_board_cards;
        for i in 0..n_players {
            let mut suit;
            suit = player_hands[i].cards.0 & SUIT_MASK;
            if transform[usize::from(suit)] == u8::MAX {
                transform[usize::from(suit)] = suit_count;
                suit_count += 1;
            }
            player_hands[i].cards.0 =
                (player_hands[i].cards.0 & RANK_MASK) | transform[usize::from(suit)];
            suit = player_hands[i].cards.1 & SUIT_MASK;
            if transform[usize::from(suit)] == u8::MAX {
                transform[usize::from(suit)] = suit_count;
                suit_count += 1;
            }
            player_hands[i].cards.1 =
                (player_hands[i].cards.1 & RANK_MASK) | transform[usize::from(suit)];
        }

        suit_count
    }

    fn reserve_batch(&self, batch_size: u64) -> (u64, u64) {
        let total_batch_count = self.get_preflop_combo_count();
        let mut enum_pos = self.enum_pos.lock().unwrap();
        let start = *enum_pos;
        let end = std::cmp::min(total_batch_count, *enum_pos + batch_size);
        *enum_pos = end;
        (start, end)
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
        cards_in_deck -= u64::from(self.fixed_board.count());
        cards_in_deck -= 2 * self.n_players as u64;
        let board_cards_remaining = 5 - u64::from(self.fixed_board.count());
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
                self.evaluate_hands(&player_hands, weight, &board, &mut batch, true);

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

    fn update_results(&self, batch: &SimulationResultsBatch, finished: bool) {
        // get lock
        let mut results = self.results.write().unwrap();
        let mut batch_hands = 0u64;
        let mut batch_equity = 0f64;
        for i in 0..(1 << self.n_players) {
            let winner_count: u64 = u64::from((i as u32).count_ones());
            batch_hands += batch.wins_by_mask[i];
            let mut actual_player_mask = 0;
            for j in 0..self.n_players {
                if (i & (1 << j)) != 0 {
                    if winner_count == 1 {
                        results.wins[batch.player_ids[j]] += batch.wins_by_mask[i];
                        if batch.player_ids[j] == 0 {
                            batch_equity += batch.wins_by_mask[i] as f64;
                        }
                    } else {
                        results.ties[batch.player_ids[j]] +=
                            (batch.wins_by_mask[i] / winner_count) as f64;
                        if batch.player_ids[j] == 0 {
                            batch_equity += (batch.wins_by_mask[i] / winner_count) as f64;
                        }
                    }
                    actual_player_mask |= 1 << batch.player_ids[j];
                }
            }
            results.wins_by_mask[actual_player_mask] += batch.wins_by_mask[i];
        }
        batch_equity /= (batch_hands as f64) + 1e-9;

        results.eval_count += batch.eval_count;
        if !self.calc_exact {
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
    }

    fn evaluate_hands(
        &self,
        player_hands: &[Hand],
        weight: u64,
        board: &Hand,
        results: &mut SimulationResultsBatch,
        flush_possible: bool,
    ) {
        // evaulate hands
        let mut winner_mask: u8 = 0;
        let mut best_score: u16 = 0;
        let mut player_mask: u8 = 1;
        for i in 0..self.n_players {
            let hand: Hand = *board + player_hands[i];
            let score = if flush_possible {
                evaluate(&hand)
            } else {
                evaluate_without_flush(&hand)
            };
            match (score > best_score, score == best_score) {
                (true, false) => {
                    // add to wins by hand mask
                    best_score = score;
                    winner_mask = player_mask;
                }
                (false, true) => {
                    winner_mask |= player_mask;
                }
                _ => {}
            }
            player_mask <<= 1;
        }
        results.wins_by_mask[usize::from(winner_mask)] += weight;
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
    fn test_approx_weighted() {
        const ERROR: f64 = 0.01;
        const THREADS: u8 = 4;
        let ranges = HandRange::from_strings(["KK".to_string(), "AA@1,QQ".to_string()].to_vec());
        let equity = approx_equity(&ranges, 0, THREADS, 0.001).unwrap();
        println!("{:?}", equity);
        assert!(equity[0] > 0.8130232455484216 - ERROR);
        assert!(equity[0] < 0.8130232455484216 + ERROR);
    }

    #[test]
    fn test_exact_weighted() {
        const THREADS: u8 = 8;
        let ranges = HandRange::from_strings(["KK".to_string(), "AA@1,QQ".to_string()].to_vec());
        let board_mask = get_card_mask("");
        let equity = exact_equity(&ranges, board_mask, THREADS).unwrap();
        println!("{:?}", equity);
        assert_eq!(equity[0], 0.8130232455484216);
    }

    #[test]
    fn test_preflop_accuracy() {
        const THREADS: u8 = 8;
        let ranges = HandRange::from_strings(["AA".to_string(), "random".to_string()].to_vec());
        let board_mask = get_card_mask("");
        let equity = exact_equity(&ranges, board_mask, THREADS).unwrap();
        println!("{:?}", equity);
        assert_eq!(equity[0], 0.8520371330210104);
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

    #[bench]
    fn bench_approx_river(b: &mut Bencher) {
        // best score with these params
        // 409,370 ns/iter (+/- 335,357)
        const THREADS: u8 = 4;
        let ranges = HandRange::from_strings(["ah2c".to_string(), "88+".to_string()].to_vec());
        let board_mask = get_card_mask("");
        b.iter(|| {
            approx_equity(&ranges, board_mask, THREADS, 0.001).unwrap();
        });
    }

    #[bench]
    fn bench_exact_river(b: &mut Bencher) {
        // best score with these params
        // 107,971 ns/iter (+/- 7,578)
        const THREADS: u8 = 4;
        let ranges = HandRange::from_strings(["ah2c".to_string(), "88+".to_string()].to_vec());
        let board_mask = get_card_mask("5hJsTc9d4s");
        b.iter(|| {
            exact_equity(&ranges, board_mask, THREADS).unwrap();
        });
    }
}
