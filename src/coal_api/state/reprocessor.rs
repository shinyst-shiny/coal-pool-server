use bytemuck::{Pod, Zeroable};
use solana_program::pubkey::Pubkey;
use steel::*;

use super::AccountDiscriminator;

/// Proof accounts track a miner's current hash, claimable rewards, and lifetime stats.
/// Every miner is allowed one proof account which is required by the program to mine or claim rewards.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct Reprocessor {
    /// The reprocess authority.
    pub authority: Pubkey,
    /// The slot the reprocessor was created at.
    pub slot: u64,
    /// Sysvar hashes
    pub hash: [u8; 32],
}

/*impl_to_bytes!(Reprocessor);
impl_account_from_bytes!(Reprocessor);*/

account!(AccountDiscriminator, Reprocessor);
