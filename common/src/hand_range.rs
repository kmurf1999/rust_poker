/*
 * Creates hand ranges from string
 *
 * Ranges are stored a vector of 8bit tuples
 * the value of the tuple is 4 * rank + suit
 */

use std::cmp::Ordering;
use std::iter::FromIterator;

use crate::constants::*;

// a single hand (0->51, 0->51)
#[derive(Debug, Clone, Copy)]
pub struct Combo(pub u8, pub u8);

pub static RANKS: &'static [char; 13] = &[
    '2', '3', '4', '5', '6', '7', '8', '9',
    'T', 'J', 'Q', 'K', 'A'
];

pub static SUITS: &'static [char; 4] = &[
    's', 'h', 'd', 'c'
];


impl Combo {
    pub fn to_string(&self) -> String {
        let chars: Vec<char> = vec![
            RANKS[usize::from(self.0 >> 2)],
            SUITS[usize::from(self.0 & 3)],
            RANKS[usize::from(self.1 >> 2)],
            SUITS[usize::from(self.1 & 3)]
        ];
        return String::from_iter(chars);
    }
}

// for sorting hands
impl Ord for Combo {
    fn cmp(&self, other: &Self) -> Ordering {
        if (self.0 >> 2) != (other.0 >> 2) {
            // compare first ranks
            return (&self.0 >> 2).cmp(&(other.0 >> 2));
        }
        if (self.1 >> 2) != (other.1 >> 2) {
            // compare second ranks
            return (&self.1 >> 2).cmp(&(other.1 >> 2));
        }
        if (self.0 & 3) != (other.0 & 3) {
            // compare first suit
            return (&self.0 & 3).cmp(&(other.0 & 3));
        }
        // compare second suit
        return (&self.1 & 3).cmp(&(other.1 & 3));
    }
}

impl PartialOrd for Combo {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/**
 * compares if two hands are the same
 */
impl PartialEq for Combo {
    fn eq(&self, other: &Self) -> bool {
        let h1: u16 = (u16::from(self.0) << 8) | u16::from(self.1);
        let h2: u16 = (u16::from(other.0) << 8) | u16::from(other.1);
        return h1 == h2;
    }
}

impl Eq for Combo {}

// a range of hands
#[derive(Debug, Clone)]
pub struct HandRange {
    pub hands: Vec<Combo>,
    char_vec:  Vec<char>
}

impl HandRange {
    // PUBLIC
    /**
     * default constructor
     * creates an empty range
     */
    fn new() -> Self {
        HandRange {
            hands: Vec::new(),
            char_vec: Vec::new()
        }
    }

    pub fn from_str_arr(arr: Vec<&str>) -> Vec<Self> {
        return arr.iter().map(|x| HandRange::from_str(x)).collect();
    }

    /**
     * Create card range from range string
     */
    pub fn from_str(text: &str) -> Self {
        let mut range: HandRange = HandRange::new();

        if text == "random" {
            range.add_all();
        } else {
            range.char_vec = text.to_lowercase().chars().collect();
            range.char_vec.push(' ');
            let mut i: usize = 0;
            while range.parse_hand(&mut i) && range.parse_char(&mut i, ',') {}
            range.remove_duplicates();
        }

        return range;
    }

    // PRIVATE
    fn parse_hand(&mut self, i: &mut usize) -> bool {

        let backtrack = *i;

        let explicit_suits: bool;
        let mut r1: u8 = u8::MAX;
        let mut r2: u8 = u8::MAX;
        let mut s1: u8 = u8::MAX;
        let mut s2: u8 = u8::MAX;

        if !self.parse_rank(i, &mut r1) {
            return false;
        }
        explicit_suits = self.parse_suit(i, &mut s1);
        if !self.parse_rank(i, &mut r2) {
            println!("HERE");
            *i = backtrack;
            return false;
        }
        if explicit_suits && !self.parse_suit(i, &mut s2) {
            *i = backtrack;
            return false;
        } if explicit_suits {
            let c1 = 4 * r1 + s1;
            let c2 = 4 * r2 + s2;
            if c1 == c2 {
                *i = backtrack;
                return false;
            }
            self.add_combo(c1, c2);
        } else {
            let mut suited = true;
            let mut offsuited = true;
            if self.parse_char(i, 'o') {
                suited = false;
            } else if self.parse_char(i, 's') {
                offsuited = false;
            }
            if self.parse_char(i, '+') {
                self.add_combos_plus(r1, r2, suited, offsuited);
            } else {
                self.add_combos(r1, r2, suited, offsuited);
            }
        }

        return true;
    }

    fn parse_char(&mut self, i: &mut usize, c: char) -> bool {
        if self.char_vec[*i] == c {
            *i += 1;
            return true;
        } else {
            return false;
        }
    }

    fn parse_rank(&mut self, i: &mut usize, rank: &mut u8) -> bool {
        *rank = char_to_rank(self.char_vec[*i]);
        if *rank == u8::MAX {
            return false;
        }
        *i += 1;
        return true;
    }

    fn parse_suit(&mut self, i: &mut usize, suit: &mut u8) -> bool {
        *suit = char_to_suit(self.char_vec[*i]);
        if *suit == u8::MAX {
            return false;
        }
        *i += 1;
        return true;
    }


    /**
     * adds a single combo
     */
    fn add_combo(&mut self, c1: u8, c2: u8) {
        // error: if out of bounds
        if c1 > 51 || c2 > 51 {
            return;
        }
        // error: if same two cards
        if c1 == c2 { return; }
        let h: Combo;
        // card >> 2 rips the suit bits off, so we can compare rank
        // card & 3, only views the last 2 bits so we can compare suit
        if c1 >> 2 < c2 >> 2 || (c1 >> 2 == c2 >> 2 && (c1 & 3) < (c2 & 3)) {
            h = Combo(c2, c1);
        } else {
            h = Combo(c1, c2);
        }
        self.hands.push(h);
    }

    /**
     * add combos rank1, rank2 -> 12
     */
    fn add_combos_plus(&mut self, rank1: u8, rank2: u8, suited: bool, offsuited: bool) {
        if rank1 == rank2 {
            // add paired hands 22->AA
            for r in rank1..13 {
                self.add_combos(r, r, suited, offsuited);
            }
        } else {
            // add other A2+ (includes A2,A3,..AK,AA)
            for r in rank2..=rank1 {
                self.add_combos(rank1, r, suited, offsuited);
            }
        }
    }

    /**
     * add suited and/or offsuit combos
     */
    fn add_combos(&mut self, rank1: u8, rank2: u8, suited: bool, offsuited: bool) {
        if suited && rank1 != rank2 {
            // add suited combos
            for suit in 0..4 {
                self.add_combo(4 * rank1 + suit, 4 * rank2 + suit);
            }
        }
        if offsuited {
            // add off suit combos
            for suit1 in 0..4 {
                for suit2 in (suit1 + 1)..4 {
                    self.add_combo(4 * rank1 + suit1, 4 * rank2 + suit2);
                    if rank1 != rank2 {
                        self.add_combo(4 * rank1 + suit2, 4 * rank2 + suit1);
                    }
                }
            }
        }
    }

    /**
     * add all combos to range
     */
    fn add_all(&mut self) {
        for c1 in 0..CARD_COUNT {
            for c2 in 0..c1 {
                self.add_combo(c1, c2);
            }
        }
    }

    /**
     * remove duplicate combos
     */
    fn remove_duplicates(&mut self) {
        // first sort hands
        self.hands.sort_by(|a, b| a.cmp(b));
        // remove duplicates
        self.hands.dedup();
    }
}

/**
 * return 64 bit card mask from string
 * @param text, example: ah2s
 */
pub fn get_card_mask(text: &str) -> u64 {
    let char_vec: Vec<char> = text.to_lowercase().chars().collect();
    let mut cards: u64 = 0;
    let len = char_vec.len();
    // if odd length
    if len % 2 != 0 {
        return 0;
    }
    for i in (0..len).step_by(2) {
        let rank = char_to_rank(char_vec[i]);
        let suit = char_to_suit(char_vec[i+1]);
        if rank == u8::MAX || suit == u8::MAX {
            // invalid
            return 0u64;
        }
        let card = (4 * rank) + suit;
        cards |= 1u64 << card;
    }
    return cards;
}


/**
 * convert 2-a to int 0..12
 */
fn char_to_rank(c: char) -> u8 {
    let rank = match c {
        'a' => 12,
        'k' => 11,
        'q' => 10,
        'j' => 9,
        't' => 8,
        '9' => 7,
        '8' => 6,
        '7' => 5,
        '6' => 4,
        '5' => 3,
        '4' => 2,
        '3' => 1,
        '2' => 0,
        _ => u8::MAX
    };
    return rank;
}

/**
 * convert s, h, c, d to int 0..3
 * spades -> 0, hearts -> 1,
 * clubs -> 2, diamonds -> 3
 */
fn char_to_suit(c: char) -> u8 {
    let suit = match c {
        's' => 0,
        'h' => 1,
        'd' => 2,
        'c' => 3,
        _ => u8::MAX
    };
    return suit;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_char_to_suit() {
        // valid input
        assert_eq!(char_to_suit('s'), 0);
        assert_eq!(char_to_suit('h'), 1);
        // invalid input
        assert_eq!(char_to_suit('x'), u8::MAX);
        assert_eq!(char_to_suit(' '), u8::MAX);
    }

    #[test]
    fn test_char_to_rank() {
        // valid input
        assert_eq!(char_to_rank('a'), 12);
        assert_eq!(char_to_rank('2'), 0);
        // invalid input
        assert_eq!(char_to_rank('x'), u8::MAX);
        assert_eq!(char_to_rank(' '), u8::MAX);
    }

    #[test]
    fn test_hand_range_new() {
        let c = HandRange::new();
        assert_eq!(c.hands.len(), 0);
    }

    #[test]
    fn test_hand_range_remove_duplicates() {
        // add same range twice and remove
        let mut c = HandRange::new();
        c.add_combos(1, 1, true, true);
        c.add_combos(1, 1, true, true);
        assert_eq!(c.hands.len(), 12);
        c.remove_duplicates();
        assert_eq!(c.hands.len(), 6);
        // two different ranges, no change
        c = HandRange::new();
        c.add_combos(1, 0, true, false);
        c.add_combos(1, 0, false, true);
        assert_eq!(c.hands.len(), 16);
        c.remove_duplicates();
        assert_eq!(c.hands.len(), 16);
    }

    #[test]
    fn test_hand_range_add_combo() {
        // invalid: card index out of bounds
        let mut c = HandRange::new();
        c.add_combo(52, 0);
        assert_eq!(c.hands.len(), 0);
        // invalid: same card
        c = HandRange::new();
        c.add_combo(0, 0);
        assert_eq!(c.hands.len(), 0);
    }

    #[test]
    fn test_hand_range_add_combos() {
        // valid test add paired hand
        let mut c = HandRange::new();
        c.add_combos(1, 1, true, true);
        assert_eq!(c.hands.len(), 6);
        // valid: test add suited hand
        c = HandRange::new();
        c.add_combos(1, 0, true, false);
        assert_eq!(c.hands.len(), 4);
        // valid: test add offsuite hand
        c = HandRange::new();
        c.add_combos(1, 0, false, true);
        assert_eq!(c.hands.len(), 12);
        // valid: test add both
        c = HandRange::new();
        c.add_combos(1, 0, true, true);
        assert_eq!(c.hands.len(), 16);
    }

    #[test]
    fn test_hand_range_random() {
        let c = HandRange::from_str("random");
        assert_eq!(c.hands.len(), 1326);
    }

    #[test]
    fn test_hand_range_from_str() {
        // valid: paired hand
        let mut c = HandRange::from_str("33");
        assert_eq!(c.hands.len(), 6);
        // valid: offsuit hand
        c = HandRange::from_str("a2o");
        assert_eq!(c.hands.len(), 12);
        // valid: suited hand
        c = HandRange::from_str("a2s");
        assert_eq!(c.hands.len(), 4);
        // valid: hand +
        c = HandRange::from_str("a2+");
        assert_eq!(c.hands.len(), 198);
        // valid: hand o+
        c = HandRange::from_str("a2o+");
        assert_eq!(c.hands.len(), 150);
        // valid: hand s+
        c = HandRange::from_str("a2s+");
        assert_eq!(c.hands.len(), 48);
        // valid: two ranges
        c = HandRange::from_str("22,a2s+");
        assert_eq!(c.hands.len(), 54);
        // valid: overlapping ranges
        c = HandRange::from_str("a2s+,a4s+");
        assert_eq!(c.hands.len(), 48);
        // valid: specific suits
        c = HandRange::from_str("as2h,2h3d");
        assert_eq!(c.hands.len(), 2);
    }
}
