use crate::constants::HAND_CATEGORY_SHIFT;
use crate::hand_evaluator::{evaluate, Hand};
use crate::hand_range::{Combo, HandRange};
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MadeHandCategories {
    QuadsOrBetter,
    FullHouse,
    Flush,
    Straight,
    ThreeOfAKind,
    TwoPair,
    Pair,
    // OverPair,
    // TopPair,
    // MiddlePair,
    // WeakPair,
    // AceHigh,
    NoMadeHand,
}

impl MadeHandCategories {
    pub fn get_table_index(&self) -> usize {
        match self {
            MadeHandCategories::QuadsOrBetter => 0,
            MadeHandCategories::FullHouse => 1,
            MadeHandCategories::Flush => 2,
            MadeHandCategories::Straight => 3,
            MadeHandCategories::ThreeOfAKind => 4,
            MadeHandCategories::TwoPair => 5,
            MadeHandCategories::Pair => 6,
            MadeHandCategories::NoMadeHand => 7,
        }
    }
    pub fn category_count() -> usize {
        8
    }
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DrawHandCategories {
    TwoCardFlushDraw,
    NutFlushDraw,
    OESD,
    // Gutshot,
    // OverCards,
    NoDraw,
}

impl DrawHandCategories {
    pub fn get_table_index(&self) -> usize {
        match self {
            DrawHandCategories::TwoCardFlushDraw => 0,
            DrawHandCategories::NutFlushDraw => 1,
            DrawHandCategories::OESD => 2,
            DrawHandCategories::NoDraw => 3,
        }
    }
    pub fn category_count() -> usize {
        4
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RangeFilter {
    pub made_hands: Vec<MadeHandCategories>,
    pub draw_hands: Vec<DrawHandCategories>,
}

impl HandRange {
    pub fn apply_filter(&mut self, board: u64, filter: &RangeFilter) {
        self.remove_conflicting_combos(board);
        self.hands.retain(|combo| {
            filter
                .made_hands
                .contains(&get_made_hand_category(&combo, board))
                || filter
                    .draw_hands
                    .contains(&get_draw_hand_category(&combo, board))
        });
    }
}

/// Contains tables representing how a hand range interacts with a board
/// Breaks hand range combo array into two tables of combos with each index representing a hand class
#[derive(Serialize, Deserialize, Debug)]
pub struct HandCategoryRange {
    board: u64,
    made_hand_table: Vec<Vec<String>>,
    draw_hand_table: Vec<Vec<String>>,
}

impl HandCategoryRange {
    pub fn from_range_and_board(hand_range: &mut HandRange, board: u64) -> Self {
        let mut made_hand_table = vec![Vec::new(); MadeHandCategories::category_count()];
        let mut draw_hand_table = vec![Vec::new(); DrawHandCategories::category_count()];
        hand_range.remove_conflicting_combos(board);
        hand_range.hands.iter().for_each(|combo| {
            made_hand_table[get_made_hand_category(&combo, board).get_table_index()]
                .push(combo.to_string());
            draw_hand_table[get_draw_hand_category(&combo, board).get_table_index()]
                .push(combo.to_string());
        });
        HandCategoryRange {
            made_hand_table,
            draw_hand_table,
            board,
        }
    }
}

pub fn get_made_hand_category(hole_cards: &Combo, board: u64) -> MadeHandCategories {
    let hand = Hand::from_bit_mask(board) + Hand::from_hole_cards(hole_cards.0, hole_cards.1);
    let score = evaluate(&hand);
    match score >> HAND_CATEGORY_SHIFT {
        9 => MadeHandCategories::QuadsOrBetter,
        8 => MadeHandCategories::QuadsOrBetter,
        7 => MadeHandCategories::FullHouse,
        6 => MadeHandCategories::Flush,
        5 => MadeHandCategories::Straight,
        4 => MadeHandCategories::ThreeOfAKind,
        3 => MadeHandCategories::TwoPair,
        2 => MadeHandCategories::Pair,
        _ => MadeHandCategories::NoMadeHand,
    }
}

pub fn get_draw_hand_category(hole_cards: &Combo, board: u64) -> DrawHandCategories {
    let eval_hand = Hand::from_bit_mask(board);
    let eval_board = Hand::empty() + Hand::from_hole_cards(hole_cards.0, hole_cards.1);
    let hand = Hand::from_bit_mask(board) + Hand::from_hole_cards(hole_cards.0, hole_cards.1);
    // detect two card flush draw
    for i in 0..4 {
        if eval_hand.suit_count(i) == 2 && eval_board.suit_count(i) == 2 {
            return DrawHandCategories::TwoCardFlushDraw;
        }
    }
    // detect ace high flush draw
    for i in 0..4 {
        // get suit mask
        if hand.suit_count(i) == 4 && hand.has_ace_of_suit(i) {
            return DrawHandCategories::NutFlushDraw;
        }
    }
    // detect OESD
    let rank_mask = hand.get_rank_mask();
    for i in 0..8 {
        let oesd_mask = 0b11110u64 << i;
        if (rank_mask & oesd_mask) == oesd_mask {
            return DrawHandCategories::OESD;
        }
    }

    DrawHandCategories::NoDraw
}

impl Hand {
    /// does hand have an ace of specific suit
    fn has_ace_of_suit(&self, suit: u8) -> bool {
        ((self.get_mask() >> 16 * (3 - suit)) & (1u64 << 12)) != 0
    }
    /// returns 16 bit rank mask, ignoring suits
    fn get_rank_mask(&self) -> u64 {
        let hand_mask = self.get_mask();
        let mut rank_mask = 0u64;
        for i in 0..4 {
            rank_mask |= (hand_mask >> 16 * (3 - i)) & 0xFFFF;
        }
        rank_mask
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hand_range::get_card_mask;

    #[test]
    fn test_get_made_hand_category() {
        let hole_cards = Combo(0u8, 1u8, 100);
        let board = 0b11100;
        assert!(get_made_hand_category(&hole_cards, board) == MadeHandCategories::QuadsOrBetter);
    }

    #[test]
    fn test_get_draw_hand_category() {
        {
            let hole_cards = Combo(0u8, 4, 100);
            let board = 0b0001000100000010;
            assert!(
                get_draw_hand_category(&hole_cards, board) == DrawHandCategories::TwoCardFlushDraw
            );
        }
        {
            let hole_cards = Combo(0u8, 1u8, 100);
            let board = get_card_mask("4s5sAs");
            assert_eq!(
                get_draw_hand_category(&hole_cards, board),
                DrawHandCategories::NutFlushDraw
            );
        }
        {
            let hole_cards = Combo(4u8, 5u8, 100); // 3, 3
            let board = get_card_mask("4s5h6c");
            assert_eq!(
                get_draw_hand_category(&hole_cards, board),
                DrawHandCategories::OESD
            );
        }
        {
            let hole_cards = Combo(8u8 * 4, 0, 100); // T, 2
            let board = get_card_mask("JcQsKd");
            assert_eq!(
                get_draw_hand_category(&hole_cards, board),
                DrawHandCategories::OESD
            );
        }
    }

    #[test]
    fn test_from_range_and_board() {
        let mut hand_range = HandRange::from_string("22+".to_string());
        let board = get_card_mask("AsTh4c");
        let tables = HandCategoryRange::from_range_and_board(&mut hand_range, board);
        assert_eq!(9, tables.made_hand_table[4].len()); // 9 trips
        assert_eq!(60, tables.made_hand_table[6].len()); // 60 pairs
    }

    #[test]
    fn test_apply_filter() {
        let mut hand_range = HandRange::from_string("22+".to_string());
        let board = get_card_mask("AsTh4c");
        let filter = RangeFilter {
            made_hands: vec![MadeHandCategories::ThreeOfAKind],
            draw_hands: vec![],
        };
        hand_range.apply_filter(board, &filter);
        assert_eq!(9, hand_range.hands.len());
    }
}
