/*
 * Creates hand ranges from string
 *
 * Ranges are stored a vector of 8bit tuples
 * the value of the tuple is 4 * rank + suit
 */

use std::cmp::Ordering;
use std::fmt;
use std::iter::FromIterator;

use crate::constants::*;

/// A single player hand
/// 0: index of card 1
/// 1: index of card 2
/// 2: weight of combo
#[derive(Debug, Clone, Copy)]
pub struct Combo(pub u8, pub u8, pub u8);

impl fmt::Display for Combo {
    /// Writes hole cards to string
    ///
    /// # Example
    /// ```
    /// // prints '2s2h'
    /// use rust_poker::hand_range::Combo;
    /// let hand = Combo(0, 1, 100);
    /// println!("{}", hand.to_string());
    /// ```
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let chars: Vec<char> = vec![
            RANK_TO_CHAR[usize::from(self.0 >> 2)],
            SUIT_TO_CHAR[usize::from(self.0 & 3)],
            RANK_TO_CHAR[usize::from(self.1 >> 2)],
            SUIT_TO_CHAR[usize::from(self.1 & 3)],
        ];
        write!(f, "{}", String::from_iter(chars))
    }
}

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
        (&self.1 & 3).cmp(&(other.1 & 3))
    }
}

impl PartialOrd for Combo {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Combo {
    fn eq(&self, other: &Self) -> bool {
        let h1: u16 = (u16::from(self.0) << 8) | u16::from(self.1);
        let h2: u16 = (u16::from(other.0) << 8) | u16::from(other.1);
        h1 == h2
    }
}

impl Eq for Combo {}

/// A range of private player hands for texas holdem
#[derive(Debug, Clone)]
pub struct HandRange {
    /// A vector of possible hole card combinations
    pub hands: Vec<Combo>,
    pub char_vec: Vec<char>,
}

impl HandRange {
    /// Creates an empty range of hands
    fn new() -> Self {
        HandRange {
            hands: Vec::new(),
            char_vec: Vec::new(),
        }
    }

    /// Create a vector of Handrange from a vector of strings
    ///
    /// Supports weighting between 0-100 using the @0-100 after the combo.  If no weight is
    /// specified, weights will default to 100.
    ///
    /// # Arguments
    ///
    /// * `arr` - A vector of equilab-like range strings
    ///
    /// # Example
    ///
    /// ```
    /// use rust_poker::hand_range::HandRange;
    /// let ranges = HandRange::from_strings(["22+,QQ@50".to_string(), "AKs".to_string()].to_vec());
    /// ```
    pub fn from_strings(arr: Vec<String>) -> Vec<Self> {
        arr.iter()
            .map(|s| HandRange::from_string(s.to_owned()))
            .collect()
    }

    /// remove combos that conflict with board
    pub fn remove_conflicting_combos(&mut self, board_mask: u64) {
        self.hands
            .retain(|x| (((1u64 << x.0) | (1u64 << x.1)) & board_mask) == 0);
    }

    /// Create a Handrange from a string
    ///
    /// # Arguments
    ///
    /// * `text` - A equilab-like range string
    ///
    /// # Example
    ///
    /// ```
    /// use rust_poker::hand_range::HandRange;
    /// let range = HandRange::from_string("JJ+".to_string());
    /// ```
    pub fn from_string(text: String) -> Self {
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

        range
    }

    fn parse_hand(&mut self, i: &mut usize) -> bool {
        let backtrack = *i;

        let explicit_suits: bool;
        let mut weight: u8 = 100;
        let mut r1: u8 = u8::MAX;
        let mut r2: u8 = u8::MAX;
        let mut s1: u8 = u8::MAX;
        let mut s2: u8 = u8::MAX;

        if !self.parse_rank(i, &mut r1) {
            return false;
        }
        explicit_suits = self.parse_suit(i, &mut s1);
        if !self.parse_rank(i, &mut r2) {
            *i = backtrack;
            return false;
        }
        if explicit_suits && !self.parse_suit(i, &mut s2) {
            *i = backtrack;
            return false;
        }
        if explicit_suits {
            let c1 = 4 * r1 + s1;
            let c2 = 4 * r2 + s2;
            if c1 == c2 {
                *i = backtrack;
                return false;
            }
            if self.parse_char(i, '@') {
                self.parse_weight(i, &mut weight);
            }
            self.add_combo(c1, c2, weight);
        } else {
            let mut suited = true;
            let mut offsuited = true;
            if self.parse_char(i, 'o') {
                suited = false;
            } else if self.parse_char(i, 's') {
                offsuited = false;
            }
            if self.parse_char(i, '+') {
                if self.parse_char(i, '@') {
                    self.parse_weight(i, &mut weight);
                }
                self.add_combos_plus(r1, r2, suited, offsuited, weight);
            } else {
                if self.parse_char(i, '@') {
                    self.parse_weight(i, &mut weight);
                }
                self.add_combos(r1, r2, suited, offsuited, weight);
            }
        }

        true
    }

    fn parse_weight(&self, i: &mut usize, weight: &mut u8) -> bool {
        let backtrack = *i;
        let mut number = 0;
        loop {
            let digit = self.char_vec[*i].to_digit(10);
            match digit {
                Some(num) => {
                    number *= 10;
                    number += num;
                    *i += 1;
                }
                None => {
                    if number > 100 {
                        *i = backtrack;
                        return false;
                    } else {
                        *weight = number as u8;
                        return true;
                    }
                }
            }
        }
    }

    fn parse_char(&mut self, i: &mut usize, c: char) -> bool {
        if self.char_vec[*i] == c {
            *i += 1;
            true
        } else {
            false
        }
    }

    fn parse_rank(&mut self, i: &mut usize, rank: &mut u8) -> bool {
        *rank = char_to_rank(self.char_vec[*i]);
        if *rank == u8::MAX {
            return false;
        }
        *i += 1;
        true
    }

    fn parse_suit(&mut self, i: &mut usize, suit: &mut u8) -> bool {
        *suit = char_to_suit(self.char_vec[*i]);
        if *suit == u8::MAX {
            return false;
        }
        *i += 1;
        true
    }

    /**
     * adds a single combo
     */
    fn add_combo(&mut self, c1: u8, c2: u8, weight: u8) {
        // error: if out of bounds
        if c1 > 51 || c2 > 51 {
            return;
        }
        // error: if same two cards
        if c1 == c2 {
            return;
        }
        let h: Combo;
        // card >> 2 rips the suit bits off, so we can compare rank
        // card & 3, only views the last 2 bits so we can compare suit
        if c1 >> 2 < c2 >> 2 || (c1 >> 2 == c2 >> 2 && (c1 & 3) < (c2 & 3)) {
            h = Combo(c2, c1, weight);
        } else {
            h = Combo(c1, c2, weight);
        }
        self.hands.push(h);
    }

    /**
     * add combos rank1, rank2 -> 12
     */
    fn add_combos_plus(&mut self, rank1: u8, rank2: u8, suited: bool, offsuited: bool, weight: u8) {
        if rank1 == rank2 {
            // add paired hands 22->AA
            for r in rank1..13 {
                self.add_combos(r, r, suited, offsuited, weight);
            }
        } else {
            // add other A2+ (includes A2,A3,..AK,AA)
            for r in rank2..=rank1 {
                self.add_combos(rank1, r, suited, offsuited, weight);
            }
        }
    }

    /**
     * add suited and/or offsuit combos
     */
    fn add_combos(&mut self, rank1: u8, rank2: u8, suited: bool, offsuited: bool, weight: u8) {
        if suited && rank1 != rank2 {
            // add suited combos
            for suit in 0..4 {
                self.add_combo(4 * rank1 + suit, 4 * rank2 + suit, weight);
            }
        }
        if offsuited {
            // add off suit combos
            for suit1 in 0..4 {
                for suit2 in (suit1 + 1)..4 {
                    self.add_combo(4 * rank1 + suit1, 4 * rank2 + suit2, weight);
                    if rank1 != rank2 {
                        self.add_combo(4 * rank1 + suit2, 4 * rank2 + suit1, weight);
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
                self.add_combo(c1, c2, 100);
            }
        }
    }

    /**
     * remove duplicate combos
     */
    fn remove_duplicates(&mut self) {
        // first sort hands
        self.hands.sort();
        // remove duplicates
        self.hands.dedup();
    }
}

/// Convert lowercase rank char to u8
///
/// # Example
///
/// ```
/// use rust_poker::hand_range::char_to_rank;
/// let rank = char_to_rank('a');
/// ```
pub fn char_to_rank(c: char) -> u8 {
    match c {
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
        _ => u8::MAX,
    }
}

/// Convert lowercase suit char to u8
///
/// # Example
///
/// ```
/// use rust_poker::hand_range::char_to_suit;
/// let rank = char_to_suit('s');
/// ```
pub fn char_to_suit(c: char) -> u8 {
    match c {
        's' => 0,
        'h' => 1,
        'd' => 2,
        'c' => 3,
        _ => u8::MAX,
    }
}

/// Converts a string into a 64bit card mask
///
/// # Arguments
///
/// * `text` - A card string
///
/// # Example
///
/// ```
/// use rust_poker::hand_range::get_card_mask;
/// let card_mask = get_card_mask("As2hQd");
/// ```
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
        let suit = char_to_suit(char_vec[i + 1]);
        if rank == u8::MAX || suit == u8::MAX {
            // invalid
            return 0u64;
        }
        let card = (4 * rank) + suit;
        cards |= 1u64 << card;
    }
    cards
}

/// Converts 64 bit card mask to string representation
pub fn mask_to_string(card_mask: u64) -> String {
    let mut card_str = String::new();
    for i in 0..CARD_COUNT {
        if ((1u64 << i) & card_mask) != 0 {
            let rank = i >> 2;
            let suit = i & 3;
            card_str.push(RANK_TO_CHAR[usize::from(rank)]);
            card_str.push(SUIT_TO_CHAR[usize::from(suit)]);
        }
    }
    card_str
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
        c.add_combos(1, 1, true, true, 100);
        c.add_combos(1, 1, true, true, 100);
        assert_eq!(c.hands.len(), 12);
        c.remove_duplicates();
        assert_eq!(c.hands.len(), 6);
        // two different ranges, no change
        c = HandRange::new();
        c.add_combos(1, 0, true, false, 100);
        c.add_combos(1, 0, false, true, 100);
        assert_eq!(c.hands.len(), 16);
        c.remove_duplicates();
        assert_eq!(c.hands.len(), 16);
    }

    #[test]
    fn test_hand_range_add_combo() {
        // invalid: card index out of bounds
        let mut c = HandRange::new();
        c.add_combo(52, 0, 100);
        assert_eq!(c.hands.len(), 0);
        // invalid: same card
        c = HandRange::new();
        c.add_combo(0, 0, 100);
        assert_eq!(c.hands.len(), 0);
    }

    #[test]
    fn test_hand_range_add_combos() {
        // valid test add paired hand
        let mut c = HandRange::new();
        c.add_combos(1, 1, true, true, 100);
        assert_eq!(c.hands.len(), 6);
        // valid: test add suited hand
        c = HandRange::new();
        c.add_combos(1, 0, true, false, 100);
        assert_eq!(c.hands.len(), 4);
        // valid: test add offsuite hand
        c = HandRange::new();
        c.add_combos(1, 0, false, true, 100);
        assert_eq!(c.hands.len(), 12);
        // valid: test add both
        c = HandRange::new();
        c.add_combos(1, 0, true, true, 100);
        assert_eq!(c.hands.len(), 16);
    }

    #[test]
    fn test_hand_range_random() {
        let c = HandRange::from_string("random".to_string());
        assert_eq!(c.hands.len(), 1326);
    }

    #[test]
    fn test_hand_range_from_str() {
        // valid: paired hand
        let mut c = HandRange::from_string("33".to_string());
        assert_eq!(c.hands.len(), 6);
        // valid: offsuit hand
        c = HandRange::from_string("a2o".to_string());
        assert_eq!(c.hands.len(), 12);
        // valid: suited hand
        c = HandRange::from_string("a2s".to_string());
        assert_eq!(c.hands.len(), 4);
        // valid: hand +
        c = HandRange::from_string("a2+".to_string());
        assert_eq!(c.hands.len(), 198);
        // valid: hand o+
        c = HandRange::from_string("a2o+".to_string());
        assert_eq!(c.hands.len(), 150);
        // valid: hand s+
        c = HandRange::from_string("a2s+".to_string());
        assert_eq!(c.hands.len(), 48);
        // valid: two ranges
        c = HandRange::from_string("22,a2s+".to_string());
        assert_eq!(c.hands.len(), 54);
        // valid: overlapping ranges
        c = HandRange::from_string("a2s+,a4s+".to_string());
        assert_eq!(c.hands.len(), 48);
        // valid: specific suits
        c = HandRange::from_string("as2h@50,AA@25,KK@100".to_string());
        assert_eq!(c.hands.len(), 13);
    }
}
