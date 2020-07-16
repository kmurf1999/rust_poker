use crate::hand_range::HandRange;
use crate::hand_evaluator::Hand;

// max player count
const MAX_PLAYERS: usize = 6;

// max combined range size
const MAX_SIZE: usize = 10000;

#[derive(Copy, Clone)]
pub struct Combo {
    // mask of all cards in combo used for rejection sampling
    pub card_mask: u64,
    // hands
    pub hands: [Option<Hand>; MAX_PLAYERS],
}

impl Combo {
    // merge a combo with another combo
    fn merge(&self, other: &Combo) -> Combo {
        let mut hands: [Option<Hand>; MAX_PLAYERS] = [None; MAX_PLAYERS];
        for i in 0..MAX_PLAYERS {
            if self.hands[i].is_some() {
                hands[i] = self.hands[i];
            }
            if other.hands[i].is_some() {
                hands[i] = other.hands[i];
            }
        }
        Combo {
            card_mask: self.card_mask | other.card_mask,
            hands: hands
        }
    }
}

pub struct CombinedRange {
    // TODO player idx is incorrect
    pub players: usize,
    pub combos: Vec<Combo>
}

impl CombinedRange {
    pub fn new() -> CombinedRange {
        CombinedRange {
            players: 0,
            combos: Vec::new()
        }
    }
    pub fn from_range(range: &HandRange, player_idx: usize) -> CombinedRange {
        let mut c_range = CombinedRange::new();

        c_range.players = 1;
        for r in &range.hands {
            let mut c = Combo {
                card_mask: (1u64 << r.0) | (1u64 << r.1),
                hands: [None; 6]
            };
            c.hands[player_idx] = Some(Hand::from_hole_cards(r.0, r.1));
            c_range.combos.push(c);
        }

        return c_range;
    }
    pub fn from_ranges(ranges: &Vec<HandRange>) -> Vec<CombinedRange> {
        let mut c_ranges: Vec<CombinedRange> = ranges.iter()
            .enumerate()
            .map(|(i, r)| CombinedRange::from_range(r, i))
            .collect();

        loop {
            let mut best_size = u64::MAX;
            let mut best_i: usize = 0;
            let mut best_j: usize = 0; for i in 0..c_ranges.len() {
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

        return c_ranges;
    }

    pub fn join(&mut self, other: &mut CombinedRange) -> CombinedRange {
        let mut c_range = CombinedRange::new();
        c_range.players = self.players + other.players;

        for c1 in &self.combos {
            for c2 in &other.combos {
                if (c1.card_mask & c2.card_mask) != 0 {
                    continue;
                }
                c_range.combos.push(c1.merge(&c2));
            }
        }

        return c_range;
    }
    pub fn estimate_join_size(&self, other: &CombinedRange) -> u64 {
        let mut size = 0u64;
        for c1 in &self.combos {
            for c2 in &other.combos {
                if (c1.card_mask & c2.card_mask) == 0 {
                    size += 1;
                }
            }
        }
        return size;
    }
}
