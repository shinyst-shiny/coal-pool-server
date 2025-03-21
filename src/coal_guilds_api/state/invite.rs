use steel::*;

use super::GuildsAccount;

/// Boost ...
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct Invite {
    /// The bump used for signing.
    pub bump: u64,

    /// The guild this invite is for.
    pub guild: Pubkey,

    /// The member this invite is for.
    pub member: Pubkey,

    /// The unix timestamp of when this invite was created.
    pub created_at: i64,
}

account!(GuildsAccount, Invite);