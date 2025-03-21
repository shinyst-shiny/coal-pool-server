use steel::*;

use super::GuildsAccount;

/// Boost ...
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct Config {
    /// The bump used for signing.
    pub bump: u64,

    /// The total amount of stake in the guilds program.
    pub total_stake: u64,

    /// The total multiplier of the guilds LP pool.
    pub total_multiplier: u64,
}

account!(GuildsAccount, Config);