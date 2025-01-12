use chrono::NaiveDateTime;
use diesel::prelude::*;
use diesel::sql_types::{BigInt, Integer, Nullable, Text, Timestamp, TinyInt, Unsigned};
use diesel::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum ExtraResourcesGenerationType {
    None = 0,
    ChromiumReprocess = 1,
    CoalStakingRewards = 2,
    OreStakingRewards = 3,
    DiamondHandsReprocess = 4,
    NftReprocess = 5,
}

impl From<usize> for ExtraResourcesGenerationType {
    fn from(value: usize) -> Self {
        match value {
            _ if value == ExtraResourcesGenerationType::None as usize => {
                ExtraResourcesGenerationType::None
            }
            _ if value == ExtraResourcesGenerationType::ChromiumReprocess as usize => {
                ExtraResourcesGenerationType::ChromiumReprocess
            }
            _ if value == ExtraResourcesGenerationType::CoalStakingRewards as usize => {
                ExtraResourcesGenerationType::CoalStakingRewards
            }
            _ if value == ExtraResourcesGenerationType::OreStakingRewards as usize => {
                ExtraResourcesGenerationType::OreStakingRewards
            }
            _ if value == ExtraResourcesGenerationType::DiamondHandsReprocess as usize => {
                ExtraResourcesGenerationType::DiamondHandsReprocess
            }
            _ => ExtraResourcesGenerationType::None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Queryable, Selectable, QueryableByName)]
#[diesel(table_name = crate::schema::extra_resources_generation)]
#[diesel(check_for_backend(diesel::mysql::Mysql))]
pub struct ExtraResourcesGeneration {
    pub id: i32,
    pub pool_id: i32,
    #[diesel(sql_type = Nullable<Integer>)]
    pub linked_challenge_id: Option<i32>,
    #[diesel(sql_type = Unsigned<BigInt>)]
    pub amount_sol: u64,
    #[diesel(sql_type = Unsigned<BigInt>)]
    pub amount_coal: u64,
    #[diesel(sql_type = Unsigned<BigInt>)]
    pub amount_ore: u64,
    #[diesel(sql_type = Unsigned<BigInt>)]
    pub amount_chromium: u64,
    #[diesel(sql_type = Unsigned<BigInt>)]
    pub amount_wood: u64,
    #[diesel(sql_type = Unsigned<BigInt>)]
    pub amount_ingot: u64,
    #[diesel(sql_type = Timestamp)]
    pub created_at: NaiveDateTime,
    #[diesel(sql_type = Nullable<Timestamp>)]
    pub finished_at: Option<NaiveDateTime>,
    #[diesel(sql_type = Timestamp)]
    pub updated_at: NaiveDateTime,
    #[diesel(sql_type = Integer)]
    pub generation_type: i32,
}

#[derive(Debug, Copy, Clone, Deserialize, Insertable)]
#[diesel(table_name = crate::schema::earnings_extra_resources)]
#[diesel(check_for_backend(diesel::mysql::Mysql))]
pub struct InsertEarningExtraResources {
    pub miner_id: i32,
    pub pool_id: i32,
    pub extra_resources_generation_id: i32,
    pub amount_sol: u64,
    pub amount_coal: u64,
    pub amount_ore: u64,
    pub amount_chromium: u64,
    pub amount_wood: u64,
    pub amount_ingot: u64,
    pub generation_type: i32,
}

#[derive(Debug, Serialize, Deserialize, Queryable, Selectable, QueryableByName)]
#[diesel(table_name = crate::schema::earnings_extra_resources)]
#[diesel(check_for_backend(diesel::mysql::Mysql))]
pub struct EarningExtraResources {
    pub id: i32,
    pub miner_id: i32,
    pub pool_id: i32,
    #[diesel(sql_type = Integer)]
    pub extra_resources_generation_id: i32,
    #[diesel(sql_type = Unsigned<BigInt>)]
    pub amount_sol: u64,
    #[diesel(sql_type = Unsigned<BigInt>)]
    pub amount_coal: u64,
    #[diesel(sql_type = Unsigned<BigInt>)]
    pub amount_ore: u64,
    #[diesel(sql_type = Unsigned<BigInt>)]
    pub amount_chromium: u64,
    #[diesel(sql_type = Unsigned<BigInt>)]
    pub amount_wood: u64,
    #[diesel(sql_type = Unsigned<BigInt>)]
    pub amount_ingot: u64,
    #[diesel(sql_type = Timestamp)]
    pub created_at: NaiveDateTime,
    #[diesel(sql_type = Timestamp)]
    pub updated_at: NaiveDateTime,
    #[diesel(sql_type = Integer)]
    pub generation_type: i32,
}

#[derive(Debug, Serialize, Deserialize, Queryable, Selectable, QueryableByName)]
#[diesel(table_name = crate::schema::challenges)]
#[diesel(check_for_backend(diesel::mysql::Mysql))]
pub struct Challenge {
    pub id: i32,
    pub pool_id: i32,
    pub submission_id: Option<i32>,
    pub challenge: Vec<u8>,
    pub rewards_earned_coal: Option<u64>,
    pub rewards_earned_ore: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize, QueryableByName)]
pub struct ChallengeWithDifficulty {
    #[diesel(sql_type = Integer)]
    pub id: i32,
    #[diesel(sql_type = Nullable<Unsigned<BigInt>>)]
    pub rewards_earned_coal: Option<u64>,
    #[diesel(sql_type = Nullable<Unsigned<BigInt>>)]
    pub rewards_earned_ore: Option<u64>,
    #[diesel(sql_type = TinyInt)]
    pub difficulty: i8,
    #[diesel(sql_type = Timestamp)]
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Selectable, QueryableByName)]
#[diesel(table_name = crate::schema::challenges)]
#[diesel(check_for_backend(diesel::mysql::Mysql))]
pub struct InsertChallenge {
    pub pool_id: i32,
    pub challenge: Vec<u8>,
    pub rewards_earned_coal: Option<u64>,
    pub rewards_earned_ore: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Queryable, Selectable, QueryableByName)]
#[diesel(table_name = crate::schema::challenges)]
#[diesel(check_for_backend(diesel::mysql::Mysql))]
pub struct UpdateChallengeRewards {
    pub rewards_earned_coal: Option<u64>,
    pub rewards_earned_ore: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Queryable, Selectable, QueryableByName)]
#[diesel(table_name = crate::schema::claims)]
#[diesel(check_for_backend(diesel::mysql::Mysql))]
pub struct Claim {
    pub miner_id: i32,
    pub pool_id: i32,
    pub txn_id: i32,
    pub amount_sol: u64,
    pub amount_coal: u64,
    pub amount_ore: u64,
    pub amount_chromium: u64,
    pub amount_wood: u64,
    pub amount_ingot: u64,
}

#[derive(Debug, Serialize, Deserialize, Queryable, Selectable, QueryableByName)]
#[diesel(table_name = crate::schema::claims)]
#[diesel(check_for_backend(diesel::mysql::Mysql))]
pub struct LastClaim {
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, Queryable, Selectable, QueryableByName)]
#[diesel(table_name = crate::schema::claims)]
#[diesel(check_for_backend(diesel::mysql::Mysql))]
pub struct InsertClaim {
    pub miner_id: i32,
    pub pool_id: i32,
    pub txn_id: i32,
    pub amount_sol: u64,
    pub amount_coal: u64,
    pub amount_ore: u64,
    pub amount_chromium: u64,
    pub amount_wood: u64,
    pub amount_ingot: u64,
}

#[derive(Debug, Serialize, Deserialize, Queryable, Selectable, QueryableByName)]
#[diesel(table_name = crate::schema::miners)]
#[diesel(check_for_backend(diesel::mysql::Mysql))]
pub struct Miner {
    pub id: i32,
    pub pubkey: String,
    pub enabled: bool,
}

#[derive(Debug, Serialize, Deserialize, Queryable, Selectable, QueryableByName)]
#[diesel(table_name = crate::schema::pools)]
#[diesel(check_for_backend(diesel::mysql::Mysql))]
pub struct Pool {
    pub id: i32,
    pub proof_pubkey: String,
    pub authority_pubkey: String,
    pub total_rewards_sol: u64,
    pub total_rewards_coal: u64,
    pub total_rewards_ore: u64,
    pub total_rewards_chromium: u64,
    pub total_rewards_wood: u64,
    pub total_rewards_ingot: u64,
    pub claimed_rewards_sol: u64,
    pub claimed_rewards_coal: u64,
    pub claimed_rewards_ore: u64,
    pub claimed_rewards_chromium: u64,
    pub claimed_rewards_wood: u64,
    pub claimed_rewards_ingot: u64,
}

#[derive(Debug, Serialize, Deserialize, Queryable, Selectable, QueryableByName)]
#[diesel(table_name = crate::schema::submissions)]
#[diesel(check_for_backend(diesel::mysql::Mysql))]
pub struct Submission {
    pub id: i64,
    pub miner_id: i32,
    pub challenge_id: i32,
    pub nonce: u64,
    pub difficulty: i8,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Deserialize, Serialize, QueryableByName)]
pub struct SubmissionWithPubkey {
    #[diesel(sql_type = BigInt)]
    pub id: i64,
    #[diesel(sql_type = Integer)]
    pub miner_id: i32,
    #[diesel(sql_type = Integer)]
    pub challenge_id: i32,
    #[diesel(sql_type = Unsigned<BigInt>)]
    pub nonce: u64,
    #[diesel(sql_type = TinyInt)]
    pub difficulty: i8,
    #[diesel(sql_type = Timestamp)]
    pub created_at: NaiveDateTime,
    #[diesel(sql_type = Text)]
    pub pubkey: String,
}

#[derive(
    Debug, Clone, Serialize, Deserialize, Insertable, Queryable, Selectable, QueryableByName,
)]
#[diesel(table_name = crate::schema::submissions)]
#[diesel(check_for_backend(diesel::mysql::Mysql))]
pub struct InsertSubmission {
    pub miner_id: i32,
    pub challenge_id: i32,
    pub nonce: u64,
    pub difficulty: i8,
}

#[derive(Debug, Serialize, Deserialize, Queryable, Selectable, QueryableByName)]
#[diesel(table_name = crate::schema::submissions)]
#[diesel(check_for_backend(diesel::mysql::Mysql))]
pub struct SubmissionWithId {
    pub id: i64,
}

#[derive(Debug, Serialize, Deserialize, Queryable, Selectable, QueryableByName)]
#[diesel(table_name = crate::schema::txns)]
#[diesel(check_for_backend(diesel::mysql::Mysql))]
pub struct Txn {
    pub id: i32,
    pub txn_type: String,
    pub signature: String,
    pub priority_fee: u32,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize, Queryable, Selectable, QueryableByName)]
#[diesel(table_name = crate::schema::txns)]
#[diesel(check_for_backend(diesel::mysql::Mysql))]
pub struct TxnId {
    pub id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Selectable, QueryableByName)]
#[diesel(table_name = crate::schema::txns)]
#[diesel(check_for_backend(diesel::mysql::Mysql))]
pub struct InsertTxn {
    pub txn_type: String,
    pub signature: String,
    pub priority_fee: u32,
}

#[derive(Debug, Serialize, Deserialize, Queryable, Selectable, QueryableByName)]
#[diesel(table_name = crate::schema::rewards)]
#[diesel(check_for_backend(diesel::mysql::Mysql))]
pub struct InsertReward {
    pub miner_id: i32,
    pub pool_id: i32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Queryable, Selectable, QueryableByName)]
#[diesel(table_name = crate::schema::rewards)]
#[diesel(check_for_backend(diesel::mysql::Mysql))]
pub struct UpdateReward {
    pub miner_id: i32,
    pub balance_sol: u64,
    pub balance_coal: u64,
    pub balance_ore: u64,
    pub balance_chromium: u64,
    pub balance_wood: u64,
    pub balance_ingot: u64,
}

#[derive(Debug, Serialize, Deserialize, Queryable, Selectable, QueryableByName)]
#[diesel(table_name = crate::schema::rewards)]
#[diesel(check_for_backend(diesel::mysql::Mysql))]
pub struct Reward {
    pub miner_id: i32,
    pub balance_sol: u64,
    pub balance_coal: u64,
    pub balance_ore: u64,
    pub balance_chromium: u64,
    pub balance_wood: u64,
    pub balance_ingot: u64,
}

#[derive(Debug, Copy, Clone, Deserialize, Insertable)]
#[diesel(table_name = crate::schema::earnings)]
#[diesel(check_for_backend(diesel::mysql::Mysql))]
pub struct InsertEarning {
    pub miner_id: i32,
    pub pool_id: i32,
    pub challenge_id: i32,
    pub amount_coal: u64,
    pub amount_ore: u64,
}

#[derive(Debug, Serialize, Deserialize, Queryable, Selectable, QueryableByName)]
#[diesel(table_name = crate::schema::earnings)]
#[diesel(check_for_backend(diesel::mysql::Mysql))]
pub struct Earning {
    pub id: i32,
    pub miner_id: i32,
    pub pool_id: i32,
    pub challenge_id: i32,
    pub amount_coal: u64,
    pub amount_ore: u64,
    #[diesel(sql_type = Timestamp)]
    pub created_at: NaiveDateTime,
    #[diesel(sql_type = Timestamp)]
    pub updated_at: NaiveDateTime,
}
