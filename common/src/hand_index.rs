#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]

use std::ptr;

// test rust that we can share this between threads
unsafe impl Sync for hand_indexer_t {}
unsafe impl Send for hand_indexer_t {}

// include hand indexer bindings
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

static TOTAL_CARDS: &'static [usize; 4] = &[2, 5, 6, 7];

impl hand_indexer_t {
    /**
     * create a new hand_indexer struct
     */
    pub fn new() -> hand_indexer_t {
        hand_indexer_t {
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

    /**
     * initialize hand indexer for rounds and cards per round
     */
    pub fn init(rounds: u32, cards_per_round: Vec<u8>) -> hand_indexer_t {
        let mut hand_indexer = hand_indexer_t::new();
        unsafe {
            assert!(hand_indexer_init(
                    rounds,
                    cards_per_round.as_ptr(),
                    &mut hand_indexer));
        }
        return hand_indexer;
    }


    /**
     * Return number of indices in a round
     */
    pub fn size(&self, round: u32) -> u64 {
        return unsafe { hand_indexer_size(self, round) };
    }

    /**
     * Gets the index for a set of cards
     */
    pub fn get_index(&self, cards: &[u8]) -> hand_index_t {
        unsafe {
            return hand_index_last(self, cards.as_ptr());
        }
    }

    /**
     * get a cannonical hand from an index
     * @param index: 64bit hand index
     * @param round: 0: preflop, 1: flop, ect.
     *
     * 8bit card is 4 * rank + suit
     * get rank using card >> 2
     * get suit using card & 3
     */
    pub fn get_hand(&self, round: u32, index: hand_index_t, cards: &mut [u8]) {
        unsafe {
            hand_unindex(self, round, index, cards.as_mut_ptr());
        }
    }
}
