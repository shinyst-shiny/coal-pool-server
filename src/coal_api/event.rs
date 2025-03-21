use bytemuck::{Pod, Zeroable};
use steel::impl_to_bytes;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct MineEvent {
    pub difficulty: u64,
    pub reward: u64,
    pub timing: i64,
    pub tool_reward: u64,
    pub stake_reward: u64,
}

impl_to_bytes!(MineEvent);
