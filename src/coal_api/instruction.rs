use crate::coal_api::consts::*;
use bytemuck::{Pod, Zeroable};
use drillx::Solution;
use num_enum::TryFromPrimitive;
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    system_program, sysvar,
};
use steel::{impl_instruction_from_bytes, impl_to_bytes};

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, TryFromPrimitive)]
#[rustfmt::skip]
pub enum CoalInstruction {
    // User
    Claim = 0,
    Close = 1,
    Mine = 2,
    OpenCoal = 3,
    Reset = 4,
    Stake = 5,
    Update = 6,
    OpenWood = 7,
    Equip = 8,
    Unequip = 9,
    InitReprocess = 10,
    FinalizeReprocess = 11,
    // Admin
    // InitCoal = 100,
    // InitWood = 101,
    InitChromium = 102,
}

impl CoalInstruction {
    pub fn to_vec(&self) -> Vec<u8> {
        vec![*self as u8]
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct InitializeArgs {
    pub bus_0_bump: u8,
    pub bus_1_bump: u8,
    pub bus_2_bump: u8,
    pub bus_3_bump: u8,
    pub bus_4_bump: u8,
    pub bus_5_bump: u8,
    pub bus_6_bump: u8,
    pub bus_7_bump: u8,
    pub config_bump: u8,
    pub metadata_bump: u8,
    pub mint_bump: u8,
    pub treasury_bump: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct InitChromiumArgs {
    pub metadata_bump: u8,
    pub mint_bump: u8,
    pub treasury_bump: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct OpenArgs {
    pub bump: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct EquipArgs {
    pub bump: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct UnequipArgs {
    pub bump: u8,
    pub plugin_authority_bump: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct MineArgs {
    pub digest: [u8; 16],
    pub nonce: [u8; 8],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct MineArgsV2 {
    pub digest: [u8; 16],
    pub nonce: [u8; 8],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ClaimArgs {
    pub amount: [u8; 8],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ReprocessArgs {
    pub reprocessor_bump: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct StakeArgs {
    pub amount: [u8; 8],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct UpgradeArgs {
    pub amount: [u8; 8],
}

impl_to_bytes!(InitializeArgs);
impl_to_bytes!(InitChromiumArgs);
impl_to_bytes!(OpenArgs);
impl_to_bytes!(MineArgs);
impl_to_bytes!(ClaimArgs);
impl_to_bytes!(StakeArgs);
impl_to_bytes!(UpgradeArgs);
impl_to_bytes!(EquipArgs);
impl_to_bytes!(UnequipArgs);
impl_to_bytes!(ReprocessArgs);

impl_instruction_from_bytes!(InitializeArgs);
impl_instruction_from_bytes!(InitChromiumArgs);
impl_instruction_from_bytes!(OpenArgs);
impl_instruction_from_bytes!(MineArgs);
impl_instruction_from_bytes!(ClaimArgs);
impl_instruction_from_bytes!(StakeArgs);
impl_instruction_from_bytes!(UpgradeArgs);
impl_instruction_from_bytes!(EquipArgs);
impl_instruction_from_bytes!(UnequipArgs);
impl_instruction_from_bytes!(ReprocessArgs);

/// Builds an auth instruction.
pub fn auth(proof: Pubkey) -> Instruction {
    Instruction {
        program_id: NOOP_PROGRAM_ID,
        accounts: vec![],
        data: proof.to_bytes().to_vec(),
    }
}

/// Builds a claim instruction.
pub fn claim_coal(signer: Pubkey, beneficiary: Pubkey, amount: u64) -> Instruction {
    let proof = Pubkey::find_program_address(&[COAL_PROOF, signer.as_ref()], &super::id()).0;
    let treasury_tokens = spl_associated_token_account::get_associated_token_address(
        &TREASURY_ADDRESS,
        &COAL_MINT_ADDRESS,
    );
    Instruction {
        program_id: super::id(),
        accounts: vec![
            AccountMeta::new(signer, true),
            AccountMeta::new(beneficiary, false),
            AccountMeta::new(proof, false),
            AccountMeta::new_readonly(TREASURY_ADDRESS, false),
            AccountMeta::new(treasury_tokens, false),
            AccountMeta::new_readonly(spl_token::id(), false),
        ],
        data: [
            CoalInstruction::Claim.to_vec(),
            ClaimArgs {
                amount: amount.to_le_bytes(),
            }
            .to_bytes()
            .to_vec(),
        ]
        .concat(),
    }
}

pub fn claim_wood(signer: Pubkey, beneficiary: Pubkey, amount: u64) -> Instruction {
    let proof = Pubkey::find_program_address(&[WOOD_PROOF, signer.as_ref()], &super::id()).0;
    let treasury_tokens = spl_associated_token_account::get_associated_token_address(
        &TREASURY_ADDRESS,
        &WOOD_MINT_ADDRESS,
    );
    Instruction {
        program_id: super::id(),
        accounts: vec![
            AccountMeta::new(signer, true),
            AccountMeta::new(beneficiary, false),
            AccountMeta::new(proof, false),
            AccountMeta::new_readonly(TREASURY_ADDRESS, false),
            AccountMeta::new(treasury_tokens, false),
            AccountMeta::new_readonly(spl_token::id(), false),
        ],
        data: [
            CoalInstruction::Claim.to_vec(),
            ClaimArgs {
                amount: amount.to_le_bytes(),
            }
            .to_bytes()
            .to_vec(),
        ]
        .concat(),
    }
}

/// Builds a close instruction.
pub fn close_coal(signer: Pubkey) -> Instruction {
    let proof_pda = Pubkey::find_program_address(&[COAL_PROOF, signer.as_ref()], &super::id());
    Instruction {
        program_id: super::id(),
        accounts: vec![
            AccountMeta::new(signer, true),
            AccountMeta::new(proof_pda.0, false),
            AccountMeta::new_readonly(solana_program::system_program::id(), false),
        ],
        data: CoalInstruction::Close.to_vec(),
    }
}

pub fn close_wood(signer: Pubkey) -> Instruction {
    let proof_pda = Pubkey::find_program_address(&[WOOD_PROOF, signer.as_ref()], &super::id());
    Instruction {
        program_id: super::id(),
        accounts: vec![
            AccountMeta::new(signer, true),
            AccountMeta::new(proof_pda.0, false),
            AccountMeta::new_readonly(solana_program::system_program::id(), false),
        ],
        data: CoalInstruction::Close.to_vec(),
    }
}

/// Builds a mine instruction.
pub fn mine_coal(
    signer: Pubkey,
    proof_authority: Pubkey,
    bus: Pubkey,
    tool: Option<Pubkey>,
    member: Option<Pubkey>,
    guild: Option<Pubkey>,
    solution: Solution,
) -> Instruction {
    let proof =
        Pubkey::find_program_address(&[COAL_PROOF, proof_authority.as_ref()], &super::id()).0;

    let mut accounts = vec![
        AccountMeta::new(signer, true),
        AccountMeta::new(bus, false),
        AccountMeta::new_readonly(COAL_CONFIG_ADDRESS, false),
        AccountMeta::new(proof, false),
        AccountMeta::new_readonly(sysvar::instructions::id(), false),
        AccountMeta::new_readonly(sysvar::slot_hashes::id(), false),
    ];

    if let Some(tool) = tool {
        accounts.push(AccountMeta::new(tool, false));
    }

    if let Some(member) = member {
        let guild_config = crate::coal_guilds_api::state::config_pda().0;
        accounts.push(AccountMeta::new_readonly(guild_config, false));
        accounts.push(AccountMeta::new_readonly(member, false));
    }

    if let Some(guild) = guild {
        accounts.push(AccountMeta::new_readonly(guild, false));
    }

    Instruction {
        program_id: super::id(),
        accounts,
        data: [
            CoalInstruction::Mine.to_vec(),
            MineArgs {
                digest: solution.d,
                nonce: solution.n,
            }
            .to_bytes()
            .to_vec(),
        ]
        .concat(),
    }
}

pub fn chop_wood(
    signer: Pubkey,
    proof_authority: Pubkey,
    bus: Pubkey,
    solution: Solution,
) -> Instruction {
    let proof =
        Pubkey::find_program_address(&[WOOD_PROOF, proof_authority.as_ref()], &super::id()).0;
    let tool = Pubkey::find_program_address(
        &[WOOD_MAIN_HAND_TOOL, proof_authority.as_ref()],
        &super::id(),
    )
    .0;

    Instruction {
        program_id: super::id(),
        accounts: vec![
            AccountMeta::new(signer, true),
            AccountMeta::new(bus, false),
            AccountMeta::new_readonly(WOOD_CONFIG_ADDRESS, false),
            AccountMeta::new(proof, false),
            AccountMeta::new_readonly(sysvar::instructions::id(), false),
            AccountMeta::new_readonly(sysvar::slot_hashes::id(), false),
            AccountMeta::new(tool, false),
        ],
        data: [
            CoalInstruction::Mine.to_vec(),
            MineArgs {
                digest: solution.d,
                nonce: solution.n,
            }
            .to_bytes()
            .to_vec(),
        ]
        .concat(),
    }
}

/// Builds an open instruction.
pub fn open_coal(signer: Pubkey, miner: Pubkey, payer: Pubkey) -> Instruction {
    let proof_pda = Pubkey::find_program_address(&[COAL_PROOF, signer.as_ref()], &super::id());
    Instruction {
        program_id: super::id(),
        accounts: vec![
            AccountMeta::new(signer, true),
            AccountMeta::new_readonly(miner, false),
            AccountMeta::new(payer, true),
            AccountMeta::new(proof_pda.0, false),
            AccountMeta::new_readonly(solana_program::system_program::id(), false),
            AccountMeta::new_readonly(sysvar::slot_hashes::id(), false),
        ],
        data: [
            CoalInstruction::OpenCoal.to_vec(),
            OpenArgs { bump: proof_pda.1 }.to_bytes().to_vec(),
        ]
        .concat(),
    }
}

pub fn open_wood(signer: Pubkey, miner: Pubkey, payer: Pubkey) -> Instruction {
    let proof_pda = Pubkey::find_program_address(&[WOOD_PROOF, signer.as_ref()], &super::id());
    Instruction {
        program_id: super::id(),
        accounts: vec![
            AccountMeta::new(signer, true),
            AccountMeta::new_readonly(miner, false),
            AccountMeta::new(payer, true),
            AccountMeta::new(proof_pda.0, false),
            AccountMeta::new_readonly(solana_program::system_program::id(), false),
            AccountMeta::new_readonly(sysvar::slot_hashes::id(), false),
        ],
        data: [
            CoalInstruction::OpenWood.to_vec(),
            OpenArgs { bump: proof_pda.1 }.to_bytes().to_vec(),
        ]
        .concat(),
    }
}

/// Builds an equip instruction
pub fn equip(
    signer: Pubkey,
    miner: Pubkey,
    payer: Pubkey,
    asset: Pubkey,
    collection: Pubkey,
    seed: &[u8],
) -> Instruction {
    let tool_pda = Pubkey::find_program_address(&[seed, signer.as_ref()], &super::id());

    Instruction {
        program_id: super::id(),
        accounts: vec![
            AccountMeta::new(signer, true),
            AccountMeta::new_readonly(miner, false),
            AccountMeta::new(payer, true),
            AccountMeta::new(asset, false),
            AccountMeta::new_readonly(collection, false),
            AccountMeta::new(tool_pda.0, false),
            AccountMeta::new_readonly(mpl_core::ID, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: [
            CoalInstruction::Equip.to_vec(),
            EquipArgs { bump: tool_pda.1 }.to_bytes().to_vec(),
        ]
        .concat(),
    }
}

/// Builds an unequip instruction
pub fn unequip(
    signer: Pubkey,
    miner: Pubkey,
    payer: Pubkey,
    asset: Pubkey,
    collection: Pubkey,
    seed: &[u8],
) -> Instruction {
    let tool_pda = Pubkey::find_program_address(&[seed, signer.as_ref()], &super::id());
    let plugin_authority = Pubkey::find_program_address(&[PLUGIN_UPDATE_AUTHORITY], &super::id());

    Instruction {
        program_id: super::id(),
        accounts: vec![
            AccountMeta::new(signer, true),
            AccountMeta::new_readonly(miner, false),
            AccountMeta::new(payer, true),
            AccountMeta::new(asset, false),
            AccountMeta::new(collection, false),
            AccountMeta::new(tool_pda.0, false),
            AccountMeta::new(plugin_authority.0, false),
            AccountMeta::new_readonly(mpl_core::ID, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: [
            CoalInstruction::Unequip.to_vec(),
            UnequipArgs {
                bump: tool_pda.1,
                plugin_authority_bump: plugin_authority.1,
            }
            .to_bytes()
            .to_vec(),
        ]
        .concat(),
    }
}

/// Builds a reset instruction.
pub fn reset_coal(signer: Pubkey) -> Instruction {
    let treasury_tokens = spl_associated_token_account::get_associated_token_address(
        &TREASURY_ADDRESS,
        &COAL_MINT_ADDRESS,
    );
    Instruction {
        program_id: super::id(),
        accounts: vec![
            AccountMeta::new(signer, true),
            AccountMeta::new(COAL_BUS_ADDRESSES[0], false),
            AccountMeta::new(COAL_BUS_ADDRESSES[1], false),
            AccountMeta::new(COAL_BUS_ADDRESSES[2], false),
            AccountMeta::new(COAL_BUS_ADDRESSES[3], false),
            AccountMeta::new(COAL_BUS_ADDRESSES[4], false),
            AccountMeta::new(COAL_BUS_ADDRESSES[5], false),
            AccountMeta::new(COAL_BUS_ADDRESSES[6], false),
            AccountMeta::new(COAL_BUS_ADDRESSES[7], false),
            AccountMeta::new(COAL_CONFIG_ADDRESS, false),
            AccountMeta::new(COAL_MINT_ADDRESS, false),
            AccountMeta::new(TREASURY_ADDRESS, false),
            AccountMeta::new(treasury_tokens, false),
            AccountMeta::new_readonly(spl_token::id(), false),
        ],
        data: CoalInstruction::Reset.to_vec(),
    }
}

pub fn reset_wood(signer: Pubkey) -> Instruction {
    let treasury_tokens = spl_associated_token_account::get_associated_token_address(
        &TREASURY_ADDRESS,
        &WOOD_MINT_ADDRESS,
    );
    Instruction {
        program_id: super::id(),
        accounts: vec![
            AccountMeta::new(signer, true),
            AccountMeta::new(WOOD_BUS_ADDRESSES[0], false),
            AccountMeta::new(WOOD_BUS_ADDRESSES[1], false),
            AccountMeta::new(WOOD_BUS_ADDRESSES[2], false),
            AccountMeta::new(WOOD_BUS_ADDRESSES[3], false),
            AccountMeta::new(WOOD_BUS_ADDRESSES[4], false),
            AccountMeta::new(WOOD_BUS_ADDRESSES[5], false),
            AccountMeta::new(WOOD_BUS_ADDRESSES[6], false),
            AccountMeta::new(WOOD_BUS_ADDRESSES[7], false),
            AccountMeta::new(WOOD_CONFIG_ADDRESS, false),
            AccountMeta::new(WOOD_MINT_ADDRESS, false),
            AccountMeta::new(TREASURY_ADDRESS, false),
            AccountMeta::new(treasury_tokens, false),
            AccountMeta::new_readonly(spl_token::id(), false),
        ],
        data: CoalInstruction::Reset.to_vec(),
    }
}

/// Build a stake instruction.
pub fn stake_coal(signer: Pubkey, sender: Pubkey, amount: u64) -> Instruction {
    let proof = Pubkey::find_program_address(&[COAL_PROOF, signer.as_ref()], &super::id()).0;
    let treasury_tokens = spl_associated_token_account::get_associated_token_address(
        &TREASURY_ADDRESS,
        &COAL_MINT_ADDRESS,
    );
    Instruction {
        program_id: super::id(),
        accounts: vec![
            AccountMeta::new(signer, true),
            AccountMeta::new(proof, false),
            AccountMeta::new(sender, false),
            AccountMeta::new(treasury_tokens, false),
            AccountMeta::new_readonly(spl_token::id(), false),
        ],
        data: [
            CoalInstruction::Stake.to_vec(),
            StakeArgs {
                amount: amount.to_le_bytes(),
            }
            .to_bytes()
            .to_vec(),
        ]
        .concat(),
    }
}

pub fn stake_wood(signer: Pubkey, sender: Pubkey, amount: u64) -> Instruction {
    let proof = Pubkey::find_program_address(&[WOOD_PROOF, signer.as_ref()], &super::id()).0;
    let treasury_tokens = spl_associated_token_account::get_associated_token_address(
        &TREASURY_ADDRESS,
        &WOOD_MINT_ADDRESS,
    );
    Instruction {
        program_id: super::id(),
        accounts: vec![
            AccountMeta::new(signer, true),
            AccountMeta::new(proof, false),
            AccountMeta::new(sender, false),
            AccountMeta::new(treasury_tokens, false),
            AccountMeta::new_readonly(spl_token::id(), false),
        ],
        data: [
            CoalInstruction::Stake.to_vec(),
            StakeArgs {
                amount: amount.to_le_bytes(),
            }
            .to_bytes()
            .to_vec(),
        ]
        .concat(),
    }
}

// Build an update instruction.
pub fn update_coal(signer: Pubkey, miner: Pubkey) -> Instruction {
    let proof = Pubkey::find_program_address(&[COAL_PROOF, signer.as_ref()], &super::id()).0;
    Instruction {
        program_id: super::id(),
        accounts: vec![
            AccountMeta::new(signer, true),
            AccountMeta::new_readonly(miner, false),
            AccountMeta::new(proof, false),
        ],
        data: CoalInstruction::Update.to_vec(),
    }
}

pub fn update_wood(signer: Pubkey, miner: Pubkey) -> Instruction {
    let proof = Pubkey::find_program_address(&[WOOD_PROOF, signer.as_ref()], &super::id()).0;
    Instruction {
        program_id: super::id(),
        accounts: vec![
            AccountMeta::new(signer, true),
            AccountMeta::new_readonly(miner, false),
            AccountMeta::new(proof, false),
        ],
        data: CoalInstruction::Update.to_vec(),
    }
}

pub fn init_chromium(signer: Pubkey) -> Instruction {
    let mint_pda =
        Pubkey::find_program_address(&[CHROMIUM_MINT, MINT_NOISE.as_slice()], &super::id());
    let metadata_pda = Pubkey::find_program_address(
        &[
            METADATA,
            mpl_token_metadata::ID.as_ref(),
            mint_pda.0.as_ref(),
        ],
        &mpl_token_metadata::ID,
    );
    let treasury_pda = Pubkey::find_program_address(&[TREASURY], &super::id());
    let treasury_tokens =
        spl_associated_token_account::get_associated_token_address(&treasury_pda.0, &mint_pda.0);

    Instruction {
        program_id: super::id(),
        accounts: vec![
            AccountMeta::new(signer, true),
            AccountMeta::new(mint_pda.0, false),
            AccountMeta::new(metadata_pda.0, false),
            AccountMeta::new_readonly(treasury_pda.0, false),
            AccountMeta::new(treasury_tokens, false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(spl_token::id(), false),
            AccountMeta::new_readonly(spl_associated_token_account::id(), false),
            AccountMeta::new_readonly(mpl_token_metadata::ID, false),
            AccountMeta::new_readonly(sysvar::rent::id(), false),
        ],
        data: [
            CoalInstruction::InitChromium.to_vec(),
            InitChromiumArgs {
                metadata_bump: metadata_pda.1,
                mint_bump: mint_pda.1,
                treasury_bump: treasury_pda.1,
            }
            .to_bytes()
            .to_vec(),
        ]
        .concat(),
    }
}

pub fn init_reprocess(signer: Pubkey) -> Instruction {
    let (reprocessor, reprocessor_bump) =
        Pubkey::find_program_address(&[REPROCESSOR, signer.as_ref()], &super::id());

    Instruction {
        program_id: super::id(),
        accounts: vec![
            AccountMeta::new(signer, true),
            AccountMeta::new(TREASURY_ADDRESS, false),
            AccountMeta::new(reprocessor, false),
            AccountMeta::new_readonly(sysvar::slot_hashes::id(), false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: [
            CoalInstruction::InitReprocess.to_vec(),
            ReprocessArgs { reprocessor_bump }.to_bytes().to_vec(),
        ]
        .concat(),
    }
}

pub fn reprocess(signer: Pubkey) -> Instruction {
    let (proof, _proof_bump) =
        Pubkey::find_program_address(&[COAL_PROOF, signer.as_ref()], &super::id());
    let (reprocessor, reprocessor_bump) =
        Pubkey::find_program_address(&[REPROCESSOR, signer.as_ref()], &super::id());
    let (bus, _bus_bump) = Pubkey::find_program_address(&[COAL_BUS, &[0]], &super::id());
    let tokens =
        spl_associated_token_account::get_associated_token_address(&signer, &CHROMIUM_MINT_ADDRESS);

    Instruction {
        program_id: super::id(),
        accounts: vec![
            AccountMeta::new(signer, true),
            AccountMeta::new(reprocessor, false),
            AccountMeta::new(proof, false),
            AccountMeta::new(bus, false),
            AccountMeta::new(CHROMIUM_MINT_ADDRESS, false),
            AccountMeta::new(tokens, false),
            AccountMeta::new_readonly(TREASURY_ADDRESS, false),
            AccountMeta::new_readonly(spl_token::id(), false),
            AccountMeta::new_readonly(sysvar::slot_hashes::id(), false),
        ],
        data: [
            CoalInstruction::FinalizeReprocess.to_vec(),
            ReprocessArgs { reprocessor_bump }.to_bytes().to_vec(),
        ]
        .concat(),
    }
}
