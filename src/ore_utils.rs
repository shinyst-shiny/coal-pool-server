use drillx::Solution;
use ore_api::consts::TOKEN_DECIMALS;
use ore_api::state::Proof;
use ore_api::{
    consts::{BUS_ADDRESSES, MINT_ADDRESS, PROOF},
    ID as ORE_ID,
};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    instruction::Instruction,
    pubkey::Pubkey,
};
use spl_token::solana_program;
use steel::AccountDeserialize;

pub const ORE_TOKEN_DECIMALS: u8 = coal_api::consts::TOKEN_DECIMALS;

pub fn get_ore_mint() -> Pubkey {
    MINT_ADDRESS
}

pub fn get_claim_ix(signer: Pubkey, beneficiary: Pubkey, claim_amount: u64) -> Instruction {
    ore_api::sdk::claim(signer, beneficiary, claim_amount)
}

pub fn get_ore_auth_ix(signer: Pubkey) -> Instruction {
    let proof = ore_proof_pubkey(signer);
    ore_api::prelude::auth(proof)
}

pub fn get_ore_mine_ix(signer: Pubkey, solution: Solution, bus: usize) -> Instruction {
    ore_api::prelude::mine(signer, signer, BUS_ADDRESSES[bus], solution, Vec::from([]))
}

pub fn get_ore_register_ix(signer: Pubkey) -> Instruction {
    ore_api::prelude::open(signer, signer, signer)
}

pub fn ore_proof_pubkey(authority: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[PROOF, authority.as_ref()], &ORE_ID).0
}

pub async fn get_proof(client: &RpcClient, address: Pubkey) -> Proof {
    let data = client
        .get_account_data(&address)
        .await
        .expect("Failed to get proof account");
    *bytemuck::try_from_bytes::<Proof>(&data[8..]).or(Err(
        solana_program::program_error::ProgramError::InvalidAccountData,
    )).unwrap()
    // *Proof::try_from_bytes(&data).expect("Failed to parse proof account")
}

pub async fn get_proof_with_authority(client: &RpcClient, authority: Pubkey) -> Proof {
    let proof_address = proof_pubkey(authority);
    get_proof(client, proof_address).await
}

pub async fn get_ore_balance(address: Pubkey, client: &RpcClient) -> u64 {
    let proof = get_proof_with_authority(client, address).await;
    let token_account_address = spl_associated_token_account::get_associated_token_address(
        &address,
        &ore_api::consts::MINT_ADDRESS,
    );
    let token_balance = if let Ok(Some(token_account)) = client
        .get_token_account(&token_account_address)
        .await
    {
        token_account.token_amount.ui_amount_string
    } else {
        "0".to_string()
    };
    return proof.balance;
}

pub fn proof_pubkey(authority: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[PROOF, authority.as_ref()], &ore_api::ID).0
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