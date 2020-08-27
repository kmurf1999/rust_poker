/// Number of cards in standard deck
pub const CARD_COUNT: u8 = 52;

/// Number of ranks in a sandard deck
/// (2 -> A)
pub const RANK_COUNT: u8 = 13;

/// char to u8 rank table
pub const RANK_TO_CHAR: &[char; 13] = &[
    '2', '3', '4', '5', '6', '7', '8', '9', 'T', 'J', 'Q', 'K', 'A',
];

/// char to u8 suit table
pub static SUIT_TO_CHAR: &[char; 4] = &['s', 'h', 'd', 'c'];

/// Tables of unique primes for hashing hands
pub const RANKS: &[u64; 13] = &[
    8192, 32769, 69632, 237568, 593920, 1531909, 3563520, 4300819, 4685870, 4690024, 4767972,
    4780561, 4801683,
];

/// Table of power of 2 flush ranks
pub const FLUSH_RANKS: &[u64; 13] = &[1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096];
