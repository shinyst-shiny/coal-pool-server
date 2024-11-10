use std::time::{SystemTime, UNIX_EPOCH};

use bytemuck::{Pod, Zeroable};
use drillx::Solution;
use coal_api::{
    consts::{COAL_BUS_ADDRESSES, COAL_CONFIG_ADDRESS, COAL_MINT_ADDRESS, COAL_PROOF, TOKEN_DECIMALS},
    state::{Config, Proof},
    instruction as coal_instruction,
    ID as COAL_ID,
};
use coal_guilds_api::{
    state as guilds_state,
};
// use coal_miner_delegation::{instruction, state::DelegatedStake, utils::AccountDeserialize};
// use coal_utils::event;
pub use coal_utils::AccountDeserialize as _;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{account::ReadableAccount, instruction::Instruction, pubkey::Pubkey};

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

pub fn get_auth_ix(signer: Pubkey) -> Instruction {
    let proof = proof_pubkey(signer);

    coal_instruction::auth(proof)
}

pub fn get_mine_ix(signer: Pubkey, solution: Solution, bus: usize, guild_proof: (Pubkey, u8), guild_member: (Pubkey, u8)) -> Instruction {
    coal_instruction::mine_coal(signer, signer, COAL_BUS_ADDRESSES[bus], solution)
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
        proof_pubkey(authority),
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
    let proof_address = proof_pubkey(authority);
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

pub async fn get_proof(client: &RpcClient, authority: Pubkey) -> Result<Proof, String> {
    let proof_address = proof_pubkey(authority);
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

pub fn proof_pubkey(authority: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[COAL_PROOF, authority.as_ref()], &COAL_ID).0
}

pub fn get_cutoff(proof: Proof, buffer_time: u64) -> i64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Failed to get time")
        .as_secs() as i64;
    proof
        .last_hash_at
        .saturating_add(62)
        .saturating_sub(buffer_time as i64)
        .saturating_sub(now)
}