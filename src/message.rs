use serde::Serialize;

pub struct ServerMessageStartMining {
    challenge: [u8; 32],
    cutoff: i64,
    nonce_range_start: u64,
    nonce_range_end: u64,
}

impl ServerMessageStartMining {
    pub fn new(
        challenge: [u8; 32],
        cutoff: i64,
        nonce_range_start: u64,
        nonce_range_end: u64,
    ) -> Self {
        ServerMessageStartMining {
            challenge,
            cutoff,
            nonce_range_start,
            nonce_range_end,
        }
    }

    pub fn to_message_binary(&self) -> Vec<u8> {
        let mut bin_data = Vec::new();
        bin_data.push(0u8);
        bin_data.extend_from_slice(&self.challenge);
        bin_data.extend_from_slice(&self.cutoff.to_le_bytes());
        bin_data.extend_from_slice(&self.nonce_range_start.to_le_bytes());
        bin_data.extend_from_slice(&self.nonce_range_end.to_le_bytes());

        bin_data
    }
}

#[derive(Serialize)]
pub struct RewardDetails {
    pub total_balance: f64,
    pub total_rewards: f64,
    pub miner_supplied_difficulty: u32,
    pub miner_earned_rewards: f64,
    pub miner_percentage: f64,
}

#[derive(Serialize)]
pub struct CoalDetails {
    pub reward_details: RewardDetails,
    pub top_stake: f64,
    pub stake_multiplier: f64,
    pub guild_total_stake: f64,
    pub guild_multiplier: f64,
    pub tool_multiplier: f64,
}

#[derive(Serialize)]
pub struct OreBoost {
    pub top_stake: f64,
    pub total_stake: f64,
    pub stake_multiplier: f64,
    pub mint_address: [u8; 32],
    pub name: String,
}

#[derive(Serialize)]
pub struct OreDetails {
    pub reward_details: RewardDetails,
    pub top_stake: f64,
    pub stake_multiplier: f64,
    pub ore_boosts: Vec<OreBoost>,
}

#[derive(Serialize)]
pub struct MinerDetails {
    pub total_chromium: f64,
    pub total_coal: f64,
    pub total_ore: f64,
    pub guild_address: [u8; 32],
    pub miner_address: [u8; 32],
}

#[derive(Serialize)]
pub struct ServerMessagePoolSubmissionResult {
    pub difficulty: u32,
    pub challenge: [u8; 32],
    pub best_nonce: u64,
    pub active_miners: u32,
    pub coal_details: CoalDetails,
    pub ore_details: OreDetails,
    pub miner_details: MinerDetails,
}

impl ServerMessagePoolSubmissionResult {
    pub fn to_binary(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap()
    }
}
