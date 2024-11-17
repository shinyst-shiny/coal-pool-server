use std::io::Read;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bytemuck::{Pod, Zeroable};
use coal_api::consts::{BUS_COUNT, COAL_MAIN_HAND_TOOL, TREASURY_ADDRESS, WOOD_BUS_ADDRESSES, WOOD_MAIN_HAND_TOOL};
use coal_api::state::{ProofV2, Tool, Treasury, WoodConfig, WoodTool};
use coal_api::{
    consts::{COAL_BUS_ADDRESSES, COAL_CONFIG_ADDRESS, COAL_MINT_ADDRESS, TOKEN_DECIMALS},
    instruction as coal_instruction,
    state::{Config, Proof}
    ,
};
use coal_guilds_api::state as guilds_state;
// use coal_miner_delegation::{instruction, state::DelegatedStake, utils::AccountDeserialize};
// use coal_utils::event;
pub use coal_utils::AccountDeserialize as _;
use drillx::Solution;
use serde::Deserialize;
use solana_client::client_error::{ClientError, ClientErrorKind};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::hash::Hash;
use solana_sdk::{account::ReadableAccount, instruction::Instruction, pubkey::Pubkey};
use steel::AccountDeserialize;
use steel::{sysvar, Clock};

pub const COAL_TOKEN_DECIMALS: u8 = TOKEN_DECIMALS;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct MineEventWithBoosts {
    pub difficulty: u64,
    pub reward: u64,
    pub timing: i64,
    pub boost_1: u64,
    pub boost_2: u64,
    pub boost_3: u64,
}

// event!(MineEventWithBoosts);


pub fn deserialize_guild_config(data: &[u8]) -> coal_guilds_api::state::Config {
    *coal_guilds_api::state::Config::try_from_bytes(&data).unwrap()
}

pub fn deserialize_guild_member(data: &[u8]) -> coal_guilds_api::state::Member {
    *coal_guilds_api::state::Member::try_from_bytes(&data).unwrap()
}

pub fn deserialize_guild(data: &[u8]) -> coal_guilds_api::state::Guild {
    *coal_guilds_api::state::Guild::try_from_bytes(&data).unwrap()
}

pub fn get_auth_ix(signer: Pubkey) -> Instruction {
    let proof = proof_pubkey(signer, Resource::Coal);

    coal_instruction::auth(proof)
}

pub fn get_mine_ix(signer: Pubkey, solution: Solution, bus: usize, tool: Option<Pubkey>, guild_member: Option<Pubkey>, guild: Option<Pubkey>) -> Instruction {
    // coal_instruction::mine_coal(signer, signer, COAL_BUS_ADDRESSES[bus], Option::None, Option::None, Option::None, solution)
    coal_instruction::mine_coal(signer, signer, COAL_BUS_ADDRESSES[bus], tool, guild_member, guild, solution)
}

pub fn get_register_ix(signer: Pubkey) -> Instruction {
    coal_instruction::open_coal(signer, signer, signer)
}

pub fn get_reset_ix(signer: Pubkey) -> Instruction {
    coal_api::instruction::reset_coal(signer)
}

pub fn get_claim_ix(signer: Pubkey, beneficiary: Pubkey, claim_amount: u64) -> Instruction {
    coal_instruction::claim_coal(signer, beneficiary, claim_amount)
}

pub fn get_stake_ix(signer: Pubkey, sender: Pubkey, stake_amount: u64) -> Instruction {
    coal_instruction::stake_coal(sender, signer, stake_amount)
}

pub fn get_guild_member(miner: Pubkey) -> (Pubkey, u8) {
    guilds_state::member_pda(miner)
}

pub fn get_guild_proof(miner: Pubkey) -> (Pubkey, u8) {
    guilds_state::guild_pda(miner)
}
pub fn get_coal_mint() -> Pubkey {
    COAL_MINT_ADDRESS
}

/*pub fn get_managed_proof_token_ata(miner: Pubkey) -> Pubkey {
    let managed_proof = Pubkey::find_program_address(
        &[b"managed-proof-account", miner.as_ref()],
        &coal_miner_delegation::id(),
    );

    get_associated_token_address(&managed_proof.0, &coal_api::consts::MINT_ADDRESS)
}

pub fn get_proof_pda(miner: Pubkey) -> Pubkey {
    let managed_proof = Pubkey::find_program_address(
        &[b"managed-proof-account", miner.as_ref()],
        &coal_miner_delegation::id(),
    );

    proof_pubkey(managed_proof.0)
}

pub async fn get_delegated_stake_account(
    client: &RpcClient,
    staker: Pubkey,
    miner: Pubkey,
) -> Result<coal_miner_delegation::state::DelegatedStake, String> {
    let data = client
        .get_account_data(&get_delegated_stake_pda(staker, miner))
        .await;
    match data {
        Ok(data) => {
            let delegated_stake = DelegatedStake::try_from_bytes(&data);
            if let Ok(delegated_stake) = delegated_stake {
                return Ok(*delegated_stake);
            } else {
                return Err("Failed to parse delegated stake account".to_string());
            }
        }
        Err(_) => return Err("Failed to get delegated stake account".to_string()),
    }
}

pub fn get_delegated_stake_pda(staker: Pubkey, miner: Pubkey) -> Pubkey {
    let managed_proof = Pubkey::find_program_address(
        &[b"managed-proof-account", miner.as_ref()],
        &coal_miner_delegation::id(),
    );

    Pubkey::find_program_address(
        &[
            b"delegated-stake",
            staker.as_ref(),
            managed_proof.0.as_ref(),
        ],
        &coal_miner_delegation::id(),
    )
    .0
}*/

pub async fn get_config(client: &RpcClient) -> Result<coal_api::state::Config, String> {
    let data = client.get_account_data(&COAL_CONFIG_ADDRESS).await;
    match data {
        Ok(data) => {
            let config = Config::try_from_bytes(&data);
            if let Ok(config) = config {
                return Ok(*config);
            } else {
                return Err("Failed to parse config account".to_string());
            }
        }
        Err(_) => return Err("Failed to get config account".to_string()),
    }
}

pub async fn get_proof_and_config_with_busses(
    client: &RpcClient,
    authority: Pubkey,
) -> (
    Result<Proof, ()>,
    Result<coal_api::state::Config, ()>,
    Result<Vec<Result<coal_api::state::Bus, ()>>, ()>,
) {
    let account_pubkeys = vec![
        proof_pubkey(authority, Resource::Coal),
        COAL_CONFIG_ADDRESS,
        COAL_BUS_ADDRESSES[0],
        COAL_BUS_ADDRESSES[1],
        COAL_BUS_ADDRESSES[2],
        COAL_BUS_ADDRESSES[3],
        COAL_BUS_ADDRESSES[4],
        COAL_BUS_ADDRESSES[5],
        COAL_BUS_ADDRESSES[6],
        COAL_BUS_ADDRESSES[7],
    ];
    let datas = client.get_multiple_accounts(&account_pubkeys).await;
    if let Ok(datas) = datas {
        let proof = if let Some(data) = &datas[0] {
            Ok(*Proof::try_from_bytes(data.data()).expect("Failed to parse treasury account"))
        } else {
            Err(())
        };

        let treasury_config = if let Some(data) = &datas[1] {
            Ok(*coal_api::state::Config::try_from_bytes(data.data())
                .expect("Failed to parse config account"))
        } else {
            Err(())
        };
        let bus_1 = if let Some(data) = &datas[2] {
            Ok(*coal_api::state::Bus::try_from_bytes(data.data())
                .expect("Failed to parse bus1 account"))
        } else {
            Err(())
        };
        let bus_2 = if let Some(data) = &datas[3] {
            Ok(*coal_api::state::Bus::try_from_bytes(data.data())
                .expect("Failed to parse bus2 account"))
        } else {
            Err(())
        };
        let bus_3 = if let Some(data) = &datas[4] {
            Ok(*coal_api::state::Bus::try_from_bytes(data.data())
                .expect("Failed to parse bus3 account"))
        } else {
            Err(())
        };
        let bus_4 = if let Some(data) = &datas[5] {
            Ok(*coal_api::state::Bus::try_from_bytes(data.data())
                .expect("Failed to parse bus4 account"))
        } else {
            Err(())
        };
        let bus_5 = if let Some(data) = &datas[6] {
            Ok(*coal_api::state::Bus::try_from_bytes(data.data())
                .expect("Failed to parse bus5 account"))
        } else {
            Err(())
        };
        let bus_6 = if let Some(data) = &datas[7] {
            Ok(*coal_api::state::Bus::try_from_bytes(data.data())
                .expect("Failed to parse bus6 account"))
        } else {
            Err(())
        };
        let bus_7 = if let Some(data) = &datas[8] {
            Ok(*coal_api::state::Bus::try_from_bytes(data.data())
                .expect("Failed to parse bus7 account"))
        } else {
            Err(())
        };
        let bus_8 = if let Some(data) = &datas[9] {
            Ok(*coal_api::state::Bus::try_from_bytes(data.data())
                .expect("Failed to parse bus1 account"))
        } else {
            Err(())
        };

        (
            proof,
            treasury_config,
            Ok(vec![bus_1, bus_2, bus_3, bus_4, bus_5, bus_6, bus_7, bus_8]),
        )
    } else {
        (Err(()), Err(()), Err(()))
    }
}

pub async fn get_original_proof(client: &RpcClient, authority: Pubkey) -> Result<Proof, String> {
    let proof_address = proof_pubkey(authority, Resource::Coal);
    let data = client.get_account_data(&proof_address).await;
    match data {
        Ok(data) => {
            let proof = Proof::try_from_bytes(&data);
            if let Ok(proof) = proof {
                return Ok(*proof);
            } else {
                return Err("Failed to parse proof account".to_string());
            }
        }
        Err(_) => return Err("Failed to get proof account".to_string()),
    }
}

pub fn get_cutoff(proof: Proof, buffer_time: u64) -> i64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Failed to get time")
        .as_secs() as i64;
    proof
        .last_hash_at
        .saturating_add(60)
        .saturating_sub(buffer_time as i64)
        .saturating_sub(now)
}

pub const BLOCKHASH_QUERY_RETRIES: usize = 5;
pub const BLOCKHASH_QUERY_DELAY: u64 = 500;

#[derive(Clone, Hash, Eq, PartialEq)]
pub enum Resource {
    Coal,
    Ore,
    Ingots,
    Wood,
    Chromium,
}

pub enum ConfigType {
    General(Config),
    Wood(WoodConfig),
}

impl ConfigType {
    pub fn last_reset_at(&self) -> i64 {
        match self {
            ConfigType::General(config) => config.last_reset_at,
            ConfigType::Wood(config) => config.last_reset_at,
        }
    }

    pub fn min_difficulty(&self) -> u64 {
        match self {
            ConfigType::General(config) => config.min_difficulty,
            ConfigType::Wood(config) => config.min_difficulty,
        }
    }

    pub fn base_reward_rate(&self) -> u64 {
        match self {
            ConfigType::General(config) => config.base_reward_rate,
            ConfigType::Wood(config) => config.base_reward_rate,
        }
    }

    pub fn top_balance(&self) -> u64 {
        match self {
            ConfigType::General(config) => config.top_balance,
            ConfigType::Wood(config) => config.top_balance,
        }
    }
}

pub enum ProofType {
    Proof(Proof),
    ProofV2(ProofV2),
}

impl ProofType {
    pub fn authority(&self) -> Pubkey {
        match self {
            ProofType::Proof(proof) => proof.authority,
            ProofType::ProofV2(proof) => proof.authority,
        }
    }

    pub fn balance(&self) -> u64 {
        match self {
            ProofType::Proof(proof) => proof.balance,
            ProofType::ProofV2(proof) => proof.balance,
        }
    }

    pub fn challenge(&self) -> [u8; 32] {
        match self {
            ProofType::Proof(proof) => proof.challenge,
            ProofType::ProofV2(proof) => proof.challenge,
        }
    }

    pub fn last_hash(&self) -> [u8; 32] {
        match self {
            ProofType::Proof(proof) => proof.last_hash,
            ProofType::ProofV2(proof) => proof.last_hash,
        }
    }

    pub fn last_hash_at(&self) -> i64 {
        match self {
            ProofType::Proof(proof) => proof.last_hash_at,
            ProofType::ProofV2(proof) => proof.last_hash_at,
        }
    }

    pub fn last_stake_at(&self) -> i64 {
        match self {
            ProofType::Proof(proof) => proof.last_stake_at,
            ProofType::ProofV2(proof) => proof.last_stake_at,
        }
    }


    pub fn miner(&self) -> Pubkey {
        match self {
            ProofType::Proof(proof) => proof.miner,
            ProofType::ProofV2(proof) => proof.miner,
        }
    }

    pub fn total_hashes(&self) -> u64 {
        match self {
            ProofType::Proof(proof) => proof.total_hashes,
            ProofType::ProofV2(proof) => proof.total_hashes,
        }
    }

    pub fn total_rewards(&self) -> u64 {
        match self {
            ProofType::Proof(proof) => proof.total_rewards,
            ProofType::ProofV2(proof) => proof.total_rewards,
        }
    }
}

pub enum ToolType {
    Tool(Tool),
    WoodTool(WoodTool),
}

impl ToolType {
    pub fn authority(&self) -> Pubkey {
        match self {
            ToolType::Tool(tool) => tool.authority,
            ToolType::WoodTool(tool) => tool.authority,
        }
    }

    pub fn asset(&self) -> Pubkey {
        match self {
            ToolType::Tool(tool) => tool.asset,
            ToolType::WoodTool(tool) => tool.asset,
        }
    }

    pub fn durability(&self) -> u64 {
        match self {
            ToolType::Tool(tool) => tool.durability,
            ToolType::WoodTool(tool) => tool.durability,
        }
    }

    pub fn multiplier(&self) -> u64 {
        match self {
            ToolType::Tool(tool) => tool.multiplier,
            ToolType::WoodTool(tool) => tool.multiplier,
        }
    }
}

pub async fn _get_treasury(client: &RpcClient) -> Treasury {
    let data = client
        .get_account_data(&TREASURY_ADDRESS)
        .await
        .expect("Failed to get treasury account");
    *Treasury::try_from_bytes(&data).expect("Failed to parse treasury account")
}

pub fn get_config_pubkey(resource: &Resource) -> Pubkey {
    match resource {
        Resource::Coal => coal_api::consts::COAL_CONFIG_ADDRESS,
        Resource::Wood => coal_api::consts::WOOD_CONFIG_ADDRESS,
        Resource::Ore => ore_api::consts::CONFIG_ADDRESS,
        _ => panic!("No config for resource"),
    }
}

pub fn deserialize_config(data: &[u8], resource: &Resource) -> ConfigType {
    match resource {
        Resource::Wood => ConfigType::Wood(
            *WoodConfig::try_from_bytes(&data).expect("Failed to parse wood config account")
        ),
        _ => ConfigType::General(
            *Config::try_from_bytes(&data).expect("Failed to parse config account")
        ),
    }
}

pub fn deserialize_tool(data: &[u8], resource: &Resource) -> ToolType {
    match resource {
        Resource::Wood => ToolType::WoodTool(*WoodTool::try_from_bytes(&data).expect("Failed to parse tool account")),
        _ => ToolType::Tool(*Tool::try_from_bytes(&data).expect("Failed to parse tool account")),
    }
}

pub async fn get_proof(client: &RpcClient, authority: Pubkey) -> Result<Proof, String> {
    let proof_address = proof_pubkey(authority, Resource::Coal);
    let data = client.get_account_data(&proof_address).await;
    match data {
        Ok(data) => {
            let proof = Proof::try_from_bytes(&data);
            if let Ok(proof) = proof {
                return Ok(*proof);
            } else {
                return Err("Failed to parse proof account".to_string());
            }
        }
        Err(_) => return Err("Failed to get proof account".to_string()),
    }
}

pub async fn get_clock(client: &RpcClient) -> Clock {
    let data = client
        .get_account_data(&sysvar::clock::ID)
        .await
        .expect("Failed to get clock account");
    bincode::deserialize::<Clock>(&data).expect("Failed to deserialize clock")
}

pub fn amount_u64_to_string(amount: u64) -> String {
    amount_u64_to_f64(amount).to_string()
}

pub fn amount_u64_to_f64(amount: u64) -> f64 {
    (amount as f64) / 10f64.powf(TOKEN_DECIMALS as f64)
}

pub fn amount_f64_to_u64(amount: f64) -> u64 {
    (amount * 10f64.powf(TOKEN_DECIMALS as f64)) as u64
}

pub fn ask_confirm(question: &str) -> bool {
    println!("{}", question);
    loop {
        let mut input = [0];
        let _ = std::io::stdin().read(&mut input);
        match input[0] as char {
            'y' | 'Y' => return true,
            'n' | 'N' => return false,
            _ => println!("y/n only please."),
        }
    }
}

pub async fn get_latest_blockhash_with_retries(
    client: &RpcClient,
) -> Result<(Hash, u64), ClientError> {
    let mut attempts = 0;

    loop {
        if let Ok((hash, slot)) = client
            .get_latest_blockhash_with_commitment(client.commitment())
            .await
        {
            return Ok((hash, slot));
        }

        // Retry
        tokio::time::sleep(Duration::from_millis(BLOCKHASH_QUERY_DELAY)).await;
        attempts += 1;
        if attempts >= BLOCKHASH_QUERY_RETRIES {
            return Err(ClientError {
                request: None,
                kind: ClientErrorKind::Custom(
                    "Max retries reached for latest blockhash query".into(),
                ),
            });
        }
    }
}

pub fn get_resource_from_str(resource: &Option<String>) -> Resource {
    match resource {
        Some(resource) => match resource.as_str() {
            "ore" => Resource::Ore,
            "ingot" => Resource::Ingots,
            "coal" => Resource::Coal,
            "wood" => Resource::Wood,
            "chromium" => Resource::Chromium,
            _ => {
                println!("Error: Invalid resource type specified.");
                std::process::exit(1);
            }
        }
        None => Resource::Coal,
    }
}

pub fn get_resource_name(resource: &Resource) -> String {
    match resource {
        Resource::Coal => "COAL".to_string(),
        Resource::Wood => "WOOD".to_string(),
        Resource::Ingots => "INGOTS".to_string(),
        Resource::Ore => "ORE".to_string(),
        Resource::Chromium => "CHROMIUM".to_string(),
    }
}

pub fn get_resource_mint(resource: &Resource) -> Pubkey {
    match resource {
        Resource::Coal => coal_api::consts::COAL_MINT_ADDRESS,
        Resource::Wood => coal_api::consts::WOOD_MINT_ADDRESS,
        Resource::Ore => ore_api::consts::MINT_ADDRESS,
        Resource::Chromium => coal_api::consts::CHROMIUM_MINT_ADDRESS,
        _ => panic!("No mint for resource"),
    }
}

pub fn get_resource_bus_addresses(resource: &Resource) -> [Pubkey; BUS_COUNT] {
    match resource {
        Resource::Coal => COAL_BUS_ADDRESSES,
        Resource::Wood => WOOD_BUS_ADDRESSES,
        _ => panic!("No bus addresses for resource"),
    }
}

pub fn get_tool_pubkey(authority: Pubkey, resource: &Resource) -> Pubkey {
    match resource {
        Resource::Wood => Pubkey::find_program_address(&[WOOD_MAIN_HAND_TOOL, authority.as_ref()], &coal_api::id()).0,
        _ => Pubkey::find_program_address(&[COAL_MAIN_HAND_TOOL, authority.as_ref()], &coal_api::id()).0,
    }
}

pub fn proof_pubkey(authority: Pubkey, resource: Resource) -> Pubkey {
    let program_id = match resource {
        Resource::Coal => &coal_api::ID,
        Resource::Wood => &coal_api::ID,
        Resource::Ore => &ore_api::ID,
        _ => panic!("No program id for resource"),
    };

    let seed = match resource {
        Resource::Coal => coal_api::consts::COAL_PROOF,
        Resource::Wood => coal_api::consts::WOOD_PROOF,
        Resource::Ore => ore_api::consts::PROOF,
        _ => panic!("No seed for resource"),
    };
    Pubkey::find_program_address(&[seed, authority.as_ref()], program_id).0
}

pub fn calculate_guild_multiplier(total_stake: u64, total_multiplier: u64, member_stake: u64) -> f64 {
    total_multiplier as f64 * member_stake as f64 / total_stake as f64
}


#[derive(Debug, Deserialize)]
pub struct Tip {
    pub time: String,
    pub landed_tips_25th_percentile: f64,
    pub landed_tips_50th_percentile: f64,
    pub landed_tips_75th_percentile: f64,
    pub landed_tips_95th_percentile: f64,
    pub landed_tips_99th_percentile: f64,
    pub ema_landed_tips_50th_percentile: f64,
}

pub fn calculate_tool_multiplier(tool: &Option<ToolType>) -> f64 {
    match tool {
        Some(tool) => {
            if tool.durability().gt(&0) {
                return 1.0 + (tool.multiplier() as f64 / 100.0);
            }
            0.0
        }
        None => 0.0,
    }
}