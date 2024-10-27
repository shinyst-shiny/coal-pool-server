use drillx::Solution;
use ore_api::{
    consts::{PROOF, BUS_ADDRESSES},
    instruction,
    ID as ORE_ID,
};
use solana_sdk::{
    pubkey::Pubkey,
    instruction::Instruction,
};


pub fn get_ore_auth_ix(signer: Pubkey) -> Instruction {
    let proof = ore_proof_pubkey(signer);
    ore_api::prelude::auth(proof)
}

pub fn get_ore_mine_ix(signer: Pubkey, solution: Solution, bus: usize) -> Instruction {
    ore_api::prelude::mine(signer, signer, BUS_ADDRESSES[bus], solution, Vec::from([signer]))
}

pub fn get_ore_register_ix(signer: Pubkey) -> Instruction {
    ore_api::prelude::open(signer, signer, signer)
}

pub fn ore_proof_pubkey(authority: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[PROOF, authority.as_ref()], &ORE_ID).0
}