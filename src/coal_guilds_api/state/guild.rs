use steel::*;

use super::GuildsAccount;

/// Boost ...
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct Guild {
    /// The bump used for signing.
    pub bump: u64,

    /// The guild authority.
    pub authority: Pubkey,

    /// Whether this guild is invite only.
    pub exclusive: u64,

    /// The minimum stake required to join this guild.
    pub min_stake: u64,

    // The total amount of stake in this boost.
    pub total_stake: u64,

    /// The unix timestamp of the last stake.
    pub last_stake_at: i64,
}

account!(GuildsAccount, Guild);