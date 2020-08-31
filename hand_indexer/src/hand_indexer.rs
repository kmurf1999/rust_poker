#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]

use std::ptr;

// tell rust that we can share this between threads
unsafe impl Sync for hand_indexer_s {}
unsafe impl Send for hand_indexer_s {}

// include hand indexer bindings
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

static TOTAL_CARDS: &'static [usize; 4] = &[2, 5, 6, 7];

pub type HandIndexer = hand_indexer_s;
pub type HandIndex = hand_index_t;

/// Wrapper functions to interface with bindgen binding for hand_indexer_s C library
impl HandIndexer {
    /// Creates a new hand_indexer_s object
    pub fn new() -> Self {
        Self {
            cards_per_round: [0; 8usize],
            round_start: [0; 8usize],
            rounds: 0,
            configurations: [0; 8usize],
            permutations: [0; 8usize],
            round_size: [0; 8usize],
            permutation_to_configuration: [ptr::null_mut(); 8usize],
            permutation_to_pi: [ptr::null_mut(); 8usize],
            configuration_to_equal: [ptr::null_mut(); 8usize],
            configuration: [ptr::null_mut(); 8usize],
            configuration_to_suit_size: [ptr::null_mut(); 8usize],
            configuration_to_offset: [ptr::null_mut(); 8usize],
        }
    }

    /// Initializes a new hand_indexer
    ///
    /// # Example
    ///
    /// ```
    /// use hand_indexer::HandIndexer;
    /// let flop_indexer = HandIndexer::init(2, [2, 3].to_vec());
    /// ```
    pub fn init(rounds: u32, cards_per_round: Vec<u8>) -> Self {
        let mut hand_indexer = HandIndexer::new();
        unsafe {
            assert!(hand_indexer_init(
                rounds.into(),
                cards_per_round.as_ptr(),
                &mut hand_indexer
            ));
        }
        return hand_indexer;
    }

    /// Return number of isomorphic hands in a round
    ///
    /// # Arguments
    ///
    /// * `round` - round to get hand for (0 -> preflop, 1 -> flop)
    pub fn size(&self, round: u32) -> u64 {
        return unsafe { hand_indexer_size(self, round.into()) };
    }

    /// Gets the index for a set of cards
    ///
    /// # Example
    /// ```
    /// use hand_indexer::HandIndexer;
    /// let flop_indexer = HandIndexer::init(2, [2, 3].to_vec());
    /// // first two cards are hole cards
    /// let cards = [0u8, 1, 5, 6, 7];
    /// let index = flop_indexer.get_index(&cards);
    /// ```
    pub fn get_index(&self, cards: &[u8]) -> HandIndex {
        unsafe {
            return hand_index_last(self, cards.as_ptr());
        }
    }

    /// Gets hand for a certain index
    ///
    /// # Arguments
    ///
    /// * `round` - round to get hand for
    /// * `cards` - cuffer to push cards into
    ///
    /// # Example
    ///
    /// ```
    /// use hand_indexer::HandIndexer;
    /// let flop_indexer = HandIndexer::init(2, [2, 3].to_vec());
    /// let mut cards = [0u8; 5];
    /// let hand_index = 400;
    /// let round = 1;
    /// flop_indexer.get_hand(round, hand_index, &mut cards);
    /// ```
    pub fn get_hand(&self, round: u32, index: HandIndex, cards: &mut [u8]) {
        unsafe {
            hand_unindex(self, round.into(), index, cards.as_mut_ptr());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_round_size() {
        let preflop_indexer = HandIndexer::init(1, [2].to_vec());
        assert_eq!(preflop_indexer.size(0), 169);
        let flop_indexer = HandIndexer::init(2, [2, 3].to_vec());
        assert_eq!(flop_indexer.size(0), 169);
    }
}
