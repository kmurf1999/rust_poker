use crate::hand_evaluator::Hand;
use crate::hand_range::HandRange;
use rand::seq::SliceRandom;
use rand::Rng;

/// Max player count
const MAX_PLAYERS: usize = 6;

/// Max combined range size
const MAX_SIZE: usize = 10000;

#[derive(Debug, Copy, Clone)]
/// One valid combination of hole card hands
pub struct Combo {
    /// Mask of all cards in combo used for rejection sampling
    pub mask: u64,
    /// Option vector of hands
    pub hands: [Hand; MAX_PLAYERS],
    /// tuple of (card_idx, card_idx, hand_weight)
    pub hole_cards: [(u8, u8, u8); MAX_PLAYERS],
}

impl Combo {
    fn new() -> Self {
        Combo {
            mask: 0,
            hands: [Hand::default(); MAX_PLAYERS],
            hole_cards: [(52, 52, 0); MAX_PLAYERS],
        }
    }
}

/// Structure to combine ranges in order to speed up rejection sampling
#[derive(Debug)]
pub struct CombinedRange {
    player_count: usize,
    players: [usize; MAX_PLAYERS],
    /// Array of valid hole card combinations
    combos: Vec<Combo>,
    size: usize,
}

impl Default for CombinedRange {
    fn default() -> Self {
        CombinedRange {
            player_count: 0,
            players: [0; MAX_PLAYERS],
            combos: Vec::new(),
            size: 0,
        }
    }
}

impl CombinedRange {
    /// Creates new combined range

    /// Creates a combined range from a hand range
    fn from_range(range: &HandRange, player_idx: usize) -> CombinedRange {
        let mut c_range = CombinedRange::default();
        c_range.player_count = 1;
        c_range.players[0] = player_idx;
        for r in &range.hands {
            let mut c = Combo::new();
            c.mask = (1u64 << r.0) | (1u64 << r.1);
            c.hands[0] = Hand::from_hole_cards(r.0, r.1);
            c.hole_cards[0] = (r.0, r.1, r.2);
            c_range.combos.push(c);
        }
        c_range.size = c_range.combos.len();
        c_range
    }
    /// Creates a combined range from a list of hand ranges
    pub fn from_ranges(ranges: &[HandRange]) -> Vec<CombinedRange> {
        let mut c_ranges: Vec<CombinedRange> = ranges
            .iter()
            .enumerate()
            .map(|(i, r)| CombinedRange::from_range(r, i))
            .collect();

        loop {
            let mut best_size = u64::MAX;
            let mut best_i: usize = 0;
            let mut best_j: usize = 0;
            for i in 0..c_ranges.len() {
                for j in 0..i {
                    let s = c_ranges[i].estimate_join_size(&c_ranges[j]);
                    if s < best_size {
                        best_size = s;
                        best_i = i;
                        best_j = j;
                    }
                }
            }
            if best_size < MAX_SIZE as u64 {
                let (head, tail) = c_ranges.split_at_mut(best_j + 1);
                head[best_j] = head[best_j].join(&mut tail[best_i - best_j - 1]);
                c_ranges.remove(best_i);
            } else {
                break;
            }
        }
        c_ranges
    }

    fn join(&mut self, other: &mut CombinedRange) -> CombinedRange {
        let mut c_range = CombinedRange::default();
        c_range.player_count = self.player_count + other.player_count;
        for i in 0..self.player_count {
            c_range.players[i] = self.players[i];
        }
        for i in 0..other.player_count {
            c_range.players[self.player_count + i] = other.players[i];
        }

        for c1 in &self.combos {
            for c2 in &other.combos {
                if (c1.mask & c2.mask) != 0 {
                    continue;
                }
                let mut combo = Combo::new();
                combo.mask = c1.mask | c2.mask;
                for i in 0..self.player_count {
                    combo.hole_cards[i] = c1.hole_cards[i];
                }
                for i in 0..other.player_count {
                    combo.hole_cards[self.player_count + i] = c2.hole_cards[i];
                }
                for i in 0..c_range.player_count {
                    combo.hands[i] =
                        Hand::from_hole_cards(combo.hole_cards[i].0, combo.hole_cards[i].1);
                }
                c_range.combos.push(combo);
            }
        }
        c_range.size = c_range.combos.len();
        c_range
    }
    fn estimate_join_size(&self, other: &CombinedRange) -> u64 {
        let mut size = 0u64;
        for c1 in &self.combos {
            for c2 in &other.combos {
                if (c1.mask & c2.mask) == 0 {
                    size += 1;
                }
            }
        }
        size
    }
    pub const fn player_count(&self) -> usize {
        self.player_count
    }
    pub const fn players(&self) -> &[usize] {
        &self.players
    }
    pub const fn combos(&self) -> &Vec<Combo> {
        &self.combos
    }
    pub const fn size(&self) -> usize {
        self.size
    }
    pub fn shuffle<R: Rng>(&mut self, rng: &mut R) {
        self.combos.shuffle(rng);
    }
}
