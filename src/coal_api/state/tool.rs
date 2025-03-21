use super::AccountDiscriminator;
use bytemuck::{Pod, Zeroable};
use solana_program::pubkey::Pubkey;
use steel::*;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct Tool {
    /// The tool authority.
    pub authority: Pubkey,

    /// Miner authorized to use the tool.
    pub miner: Pubkey,

    /// The equipped tool.
    pub asset: Pubkey,

    /// The remaining durability of the tool.
    pub durability: u64,

    /// The multiplier of the tool.
    pub multiplier: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct WoodTool {
    /// The tool authority.
    pub authority: Pubkey,

    /// Miner authorized to use the tool.
    pub miner: Pubkey,

    /// The equipped tool.
    pub asset: Pubkey,

    /// The remaining durability of the tool.
    pub durability: u64,

    /// The multiplier of the tool.
    pub multiplier: u64,
}

/*impl_to_bytes!(Tool);
impl_account_from_bytes!(Tool);
impl_to_bytes!(WoodTool);
impl_account_from_bytes!(WoodTool);*/

account!(AccountDiscriminator, Tool);
account!(AccountDiscriminator, WoodTool);
