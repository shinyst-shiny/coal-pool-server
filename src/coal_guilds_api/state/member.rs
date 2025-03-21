use steel::*;

use super::GuildsAccount;

/// Member
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct Member {
    /// The bump used for signing.
    pub bump: u64,

    /// The mint address
    pub authority: Pubkey,

    /// The guild this member is in.
    pub guild: Pubkey,

    /// Whether this member is an active miner.
    pub is_active: u64,

    /// The unix timestamp of the last stake.
    pub last_stake_at: i64,

    // The total amount of stake in this boost.
    pub total_stake: u64,

    // The unix timestamp of the last join.
    pub last_join_at: i64,
}

account!(GuildsAccount, Member);