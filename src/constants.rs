/// Number of cards in standard deck
pub const CARD_COUNT: u8 = 52;

/// Number of ranks in a sandard deck
/// (2 -> A)
pub const RANK_COUNT: u8 = 13;

/// Tables of unique primes for hashing hands
pub const RANKS: &'static [u64; 13] = &[
    8192, 32769, 69632, 237568, 593920,
    1531909, 3563520, 4300819, 4685870,
    4690024, 4767972, 4780561, 4801683
];

/// Table of power of 2 flush ranks
pub const FLUSH_RANKS: &'static [u64; 13] = &[
    1, 2, 4, 8, 16,
    32, 64, 128, 256,
    512, 1024, 2048, 4096
];
