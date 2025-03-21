use crate::coal_api::consts::*;
use crate::coal_api::state::*;
use mpl_core::types::UpdateAuthority;
use mpl_core::Asset;
use solana_program::{
    account_info::AccountInfo, msg, program_error::ProgramError, program_pack::Pack,
    pubkey::Pubkey, system_program, sysvar,
};
use spl_token::state::Mint;
use steel;
use steel::{AccountDeserialize, Discriminator};

/// Errors if:
/// - Account is not a signer.
pub fn load_signer<'a, 'info>(info: &'a AccountInfo<'info>) -> Result<(), ProgramError> {
    if !info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    Ok(())
}

/// Errors if:
/// - Owner is not Coal program.
/// - Address does not match the expected bus address.
/// - Data is empty.
/// - Data cannot deserialize into a coal bus account.
/// - Bus ID does not match the expected ID.
/// - Expected to be writable, but is not.
pub fn load_coal_bus<'a, 'info>(
    info: &'a AccountInfo<'info>,
    id: u64,
    is_writable: bool,
) -> Result<(), ProgramError> {
    if info.owner.ne(&super::id()) {
        return Err(ProgramError::InvalidAccountOwner);
    }

    if info.key.ne(&COAL_BUS_ADDRESSES[id as usize]) {
        return Err(ProgramError::InvalidSeeds);
    }

    if info.data_is_empty() {
        return Err(ProgramError::UninitializedAccount);
    }

    let bus_data = info.data.borrow();
    let bus = Bus::try_from_bytes(&bus_data)?;

    if bus.id.ne(&id) {
        return Err(ProgramError::InvalidAccountData);
    }

    if is_writable && !info.is_writable {
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(())
}

/// Errors if:
/// - Owner is not Coal program.
/// - Address does not match the expected bus address.
/// - Data is empty.
/// - Data cannot deserialize into a bus account.
/// - Bus ID does not match the expected ID.
/// - Expected to be writable, but is not.
pub fn load_wood_bus<'a, 'info>(
    info: &'a AccountInfo<'info>,
    id: u64,
    is_writable: bool,
) -> Result<(), ProgramError> {
    if info.owner.ne(&super::id()) {
        return Err(ProgramError::InvalidAccountOwner);
    }

    if info.key.ne(&WOOD_BUS_ADDRESSES[id as usize]) {
        return Err(ProgramError::InvalidSeeds);
    }

    if info.data_is_empty() {
        return Err(ProgramError::UninitializedAccount);
    }

    let bus_data = info.data.borrow();
    let bus = Bus::try_from_bytes(&bus_data)?;

    if bus.id.ne(&id) {
        return Err(ProgramError::InvalidAccountData);
    }

    if is_writable && !info.is_writable {
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(())
}

/// Errors if:
/// - Owner is not Ore program.
/// - Data is empty.
/// - Data cannot deserialize into a coal bus account.
/// - Bus ID is not in the expected range.
/// - Address is not in set of valid coal bus address.
/// - Expected to be writable, but is not.
pub fn load_any_coal_bus<'a, 'info>(
    info: &'a AccountInfo<'info>,
    is_writable: bool,
) -> Result<(), ProgramError> {
    if info.owner.ne(&super::id()) {
        return Err(ProgramError::InvalidAccountOwner);
    }

    if info.data_is_empty() {
        return Err(ProgramError::UninitializedAccount);
    }

    if info.data.borrow()[0].ne(&(Bus::discriminator() as u8)) {
        return Err(solana_program::program_error::ProgramError::InvalidAccountData);
    }

    if !COAL_BUS_ADDRESSES.contains(info.key) {
        return Err(ProgramError::InvalidSeeds);
    }

    if is_writable && !info.is_writable {
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(())
}

/// Errors if:
/// - Owner is not COAL program.
/// - Data is empty.
/// - Data cannot deserialize into a wood bus account.
/// - Bus ID is not in the expected range.
/// - Address is not in set of valid wood bus address.
/// - Expected to be writable, but is not.
pub fn load_any_wood_bus<'a, 'info>(
    info: &'a AccountInfo<'info>,
    is_writable: bool,
) -> Result<(), ProgramError> {
    if info.owner.ne(&super::id()) {
        return Err(ProgramError::InvalidAccountOwner);
    }

    if info.data_is_empty() {
        return Err(ProgramError::UninitializedAccount);
    }

    if info.data.borrow()[0].ne(&(Bus::discriminator() as u8)) {
        return Err(solana_program::program_error::ProgramError::InvalidAccountData);
    }

    if !WOOD_BUS_ADDRESSES.contains(info.key) {
        return Err(ProgramError::InvalidSeeds);
    }

    if is_writable && !info.is_writable {
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(())
}

/// Errors if:
/// - Owner is not Coal program.
/// - Address does not match the expected address.
/// - Data is empty.
/// - Data cannot deserialize into a coal config account.
/// - Expected to be writable, but is not.
pub fn load_coal_config<'a, 'info>(
    info: &'a AccountInfo<'info>,
    is_writable: bool,
) -> Result<(), ProgramError> {
    if info.owner.ne(&super::id()) {
        return Err(ProgramError::InvalidAccountOwner);
    }

    if info.key.ne(&COAL_CONFIG_ADDRESS) {
        return Err(ProgramError::InvalidSeeds);
    }

    if info.data_is_empty() {
        return Err(ProgramError::UninitializedAccount);
    }

    if info.data.borrow()[0].ne(&(Config::discriminator() as u8)) {
        return Err(solana_program::program_error::ProgramError::InvalidAccountData);
    }

    if is_writable && !info.is_writable {
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(())
}

/// Errors if:
/// - Owner is not Coal program.
/// - Address does not match the expected address.
/// - Data is empty.
/// - Data cannot deserialize into a config account.
/// - Expected to be writable, but is not.
pub fn load_wood_config<'a, 'info>(
    info: &'a AccountInfo<'info>,
    is_writable: bool,
) -> Result<(), ProgramError> {
    if info.owner.ne(&super::id()) {
        return Err(ProgramError::InvalidAccountOwner);
    }

    if info.key.ne(&WOOD_CONFIG_ADDRESS) {
        return Err(ProgramError::InvalidSeeds);
    }

    if info.data_is_empty() {
        return Err(ProgramError::UninitializedAccount);
    }

    if info.data.borrow()[0].ne(&(WoodConfig::discriminator() as u8)) {
        return Err(solana_program::program_error::ProgramError::InvalidAccountData);
    }

    if is_writable && !info.is_writable {
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(())
}

/// Errors if:
/// - Owner is not Coal program.
/// - Data is empty.
/// - Data cannot deserialize into a proof account.
/// - Proof authority does not match the expected address.
/// - Expected to be writable, but is not.
pub fn load_coal_proof<'a, 'info>(
    info: &'a AccountInfo<'info>,
    authority: &Pubkey,
    is_writable: bool,
) -> Result<(), ProgramError> {
    if info.owner.ne(&super::id()) {
        return Err(ProgramError::InvalidAccountOwner);
    }

    if info.data_is_empty() {
        return Err(ProgramError::UninitializedAccount);
    }

    let proof_data = info.data.borrow();
    let proof = Proof::try_from_bytes(&proof_data)?;

    if proof.authority.ne(&authority) {
        return Err(ProgramError::InvalidAccountData);
    }

    if is_writable && !info.is_writable {
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(())
}

/// Errors if:
/// - Owner is not Coal program.
/// - Data is empty.
/// - Data cannot deserialize into a proof account.
/// - Proof authority does not match the expected address.
/// - Expected to be writable, but is not.
pub fn load_reprocessor<'a, 'info>(
    info: &'a AccountInfo<'info>,
    authority: &Pubkey,
    is_writable: bool,
) -> Result<(), ProgramError> {
    if info.owner.ne(&super::id()) {
        return Err(ProgramError::InvalidAccountOwner);
    }

    if info.data_is_empty() {
        return Err(ProgramError::UninitializedAccount);
    }

    let data = info.data.borrow();
    let reprocessor = Reprocessor::try_from_bytes(&data)?;

    if reprocessor.authority.ne(&authority) {
        return Err(ProgramError::InvalidAccountData);
    }

    if is_writable && !info.is_writable {
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(())
}

/// Errors if:
/// - Owner is not Coal program.
/// - Data is empty.
/// - Data cannot deserialize into a proof account.
/// - Proof authority does not match the expected address.
/// - Expected to be writable, but is not.
pub fn load_proof_v2<'a, 'info>(
    info: &'a AccountInfo<'info>,
    authority: &Pubkey,
    resource: &Pubkey,
    is_writable: bool,
) -> Result<(), ProgramError> {
    if info.owner.ne(&super::id()) {
        return Err(ProgramError::InvalidAccountOwner);
    }

    if info.data_is_empty() {
        return Err(ProgramError::UninitializedAccount);
    }

    let proof_data = info.data.borrow();
    let proof = ProofV2::try_from_bytes(&proof_data)?;

    if proof.resource.ne(&resource) {
        return Err(ProgramError::InvalidAccountData);
    }

    if proof.authority.ne(&authority) {
        return Err(ProgramError::InvalidAccountData);
    }

    if is_writable && !info.is_writable {
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(())
}

/// Errors if:
/// - Owner is not Coal program.
/// - Data is empty.
/// - Data cannot deserialize into a proof account.
/// - Proof miner does not match the expected address.
/// - Expected to be writable, but is not.
pub fn load_coal_proof_with_miner<'a, 'info>(
    info: &'a AccountInfo<'info>,
    miner: &Pubkey,
    is_writable: bool,
) -> Result<(), ProgramError> {
    if info.owner.ne(&super::id()) {
        return Err(ProgramError::InvalidAccountOwner);
    }

    if info.data_is_empty() {
        return Err(ProgramError::UninitializedAccount);
    }

    let proof_data = info.data.borrow();
    let proof = Proof::try_from_bytes(&proof_data)?;

    if proof.miner.ne(&miner) {
        return Err(ProgramError::InvalidAccountData);
    }

    if is_writable && !info.is_writable {
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(())
}

/// Errors if:
/// - Owner is not Coal program.
/// - Data is empty.
/// - Data cannot deserialize into a proof account.
/// - Proof miner does not match the expected address.
/// - Expected to be writable, but is not.
pub fn load_proof_v2_with_miner<'a, 'info>(
    info: &'a AccountInfo<'info>,
    miner: &Pubkey,
    resource: &Pubkey,
    is_writable: bool,
) -> Result<(), ProgramError> {
    if info.owner.ne(&super::id()) {
        return Err(ProgramError::InvalidAccountOwner);
    }

    if info.data_is_empty() {
        return Err(ProgramError::UninitializedAccount);
    }

    let proof_data = info.data.borrow();
    let proof = ProofV2::try_from_bytes(&proof_data)?;

    if proof.resource.ne(&resource) {
        return Err(ProgramError::InvalidAccountData);
    }

    if proof.miner.ne(&miner) {
        return Err(ProgramError::InvalidAccountData);
    }

    if is_writable && !info.is_writable {
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(())
}

/// Errors if:
/// - Owner is not Coal program.
/// - Data is empty.
/// - Data cannot deserialize into a proof account.
/// - Expected to be writable, but is not.
pub fn load_any_coal_proof<'a, 'info>(
    info: &'a AccountInfo<'info>,
    is_writable: bool,
) -> Result<(), ProgramError> {
    if info.owner.ne(&super::id()) {
        return Err(ProgramError::InvalidAccountOwner);
    }

    if info.data_is_empty() {
        return Err(ProgramError::UninitializedAccount);
    }

    if info.data.borrow()[0].ne(&(Proof::discriminator() as u8)) {
        return Err(solana_program::program_error::ProgramError::InvalidAccountData);
    }

    if is_writable && !info.is_writable {
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(())
}

/// Errors if:
/// - Owner is not Coal program.
/// - Data is empty.
/// - Data cannot deserialize into a proof account.
/// - Expected to be writable, but is not.
pub fn load_any_proof_v2<'a, 'info>(
    info: &'a AccountInfo<'info>,
    is_writable: bool,
) -> Result<(), ProgramError> {
    if info.owner.ne(&super::id()) {
        return Err(ProgramError::InvalidAccountOwner);
    }

    if info.data_is_empty() {
        return Err(ProgramError::UninitializedAccount);
    }

    if info.data.borrow()[0].ne(&(ProofV2::discriminator() as u8)) {
        return Err(solana_program::program_error::ProgramError::InvalidAccountData);
    }

    if is_writable && !info.is_writable {
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(())
}

/// Errors if:
/// - Owner is not Coal program.
/// - Address does not match the expected address.
/// - Data is empty.
/// - Data cannot deserialize into a treasury account.
/// - Expected to be writable, but is not.
pub fn load_treasury<'a, 'info>(
    info: &'a AccountInfo<'info>,
    is_writable: bool,
) -> Result<(), ProgramError> {
    if info.owner.ne(&super::id()) {
        return Err(ProgramError::InvalidAccountOwner);
    }

    if info.key.ne(&TREASURY_ADDRESS) {
        return Err(ProgramError::InvalidSeeds);
    }

    if info.data_is_empty() {
        return Err(ProgramError::UninitializedAccount);
    }

    if info.data.borrow()[0].ne(&(Treasury::discriminator() as u8)) {
        return Err(solana_program::program_error::ProgramError::InvalidAccountData);
    }

    if is_writable && !info.is_writable {
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(())
}

/// Errors if:
/// - Address does not match the expected treasury tokens address.
/// - Cannot load as a token account
pub fn load_coal_treasury_tokens<'a, 'info>(
    info: &'a AccountInfo<'info>,
    is_writable: bool,
) -> Result<(), ProgramError> {
    if info.key.ne(&COAL_TREASURY_TOKENS_ADDRESS) {
        return Err(ProgramError::InvalidSeeds);
    }

    load_token_account(
        info,
        Some(&TREASURY_ADDRESS),
        &COAL_MINT_ADDRESS,
        is_writable,
    )
}

/// Errors if:
/// - Address does not match the expected treasury tokens address.
/// - Cannot load as a token account
pub fn load_wood_treasury_tokens<'a, 'info>(
    info: &'a AccountInfo<'info>,
    is_writable: bool,
) -> Result<(), ProgramError> {
    if info.key.ne(&WOOD_TREASURY_TOKENS_ADDRESS) {
        return Err(ProgramError::InvalidSeeds);
    }

    load_token_account(
        info,
        Some(&TREASURY_ADDRESS),
        &WOOD_MINT_ADDRESS,
        is_writable,
    )
}

/// Errors if:
/// - Owner is not SPL token program.
/// - Address does not match the expected mint address.
/// - Data is empty.
/// - Data cannot deserialize into a mint account.
/// - Expected to be writable, but is not.
pub fn load_mint<'a, 'info>(
    info: &'a AccountInfo<'info>,
    address: Pubkey,
    is_writable: bool,
) -> Result<(), ProgramError> {
    if info.owner.ne(&spl_token::id()) {
        return Err(ProgramError::InvalidAccountOwner);
    }

    if info.key.ne(&address) {
        return Err(ProgramError::InvalidSeeds);
    }

    if info.data_is_empty() {
        return Err(ProgramError::UninitializedAccount);
    }

    Mint::unpack(&info.data.borrow())?;

    if is_writable && !info.is_writable {
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(())
}

/// Errors if:
/// - Owner is not SPL token program.
/// - Data is empty.
/// - Data cannot deserialize into a token account.
/// - Token account owner does not match the expected owner address.
/// - Token account mint does not match the expected mint address.
/// - Expected to be writable, but is not.
pub fn load_token_account<'a, 'info>(
    info: &'a AccountInfo<'info>,
    owner: Option<&Pubkey>,
    mint: &Pubkey,
    is_writable: bool,
) -> Result<(), ProgramError> {
    if info.owner.ne(&spl_token::id()) {
        return Err(ProgramError::InvalidAccountOwner);
    }

    if info.data_is_empty() {
        return Err(ProgramError::UninitializedAccount);
    }

    let account_data = info.data.borrow();
    let account = spl_token::state::Account::unpack(&account_data)?;

    if account.mint.ne(&mint) {
        msg!("Invalid mint: {:?} == {:?}", account.mint, mint);
        return Err(ProgramError::InvalidAccountData);
    }

    if let Some(owner) = owner {
        if account.owner.ne(owner) {
            msg!("Invalid owner: {:?} == {:?}", account.owner, owner);
            return Err(ProgramError::InvalidAccountData);
        }
    }

    if is_writable && !info.is_writable {
        msg!(
            "Invalid writable: {:?} == {:?}",
            info.is_writable,
            is_writable
        );
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(())
}

/// Errors if:
/// - Address does not match PDA derived from provided seeds.
/// - Cannot load as an uninitialized account.
pub fn load_uninitialized_pda<'a, 'info>(
    info: &'a AccountInfo<'info>,
    seeds: &[&[u8]],
    bump: u8,
    program_id: &Pubkey,
) -> Result<(), ProgramError> {
    let pda = Pubkey::find_program_address(seeds, program_id);

    if info.key.ne(&pda.0) {
        return Err(ProgramError::InvalidSeeds);
    }

    if bump.ne(&pda.1) {
        return Err(ProgramError::InvalidSeeds);
    }

    load_system_account(info, true)
}

/// Errors if:
/// - Owner is not the system program.
/// - Data is not empty.
/// - Account is not writable.
pub fn load_system_account<'a, 'info>(
    info: &'a AccountInfo<'info>,
    is_writable: bool,
) -> Result<(), ProgramError> {
    if info.owner.ne(&system_program::id()) {
        return Err(ProgramError::InvalidAccountOwner);
    }

    if !info.data_is_empty() {
        return Err(ProgramError::AccountAlreadyInitialized);
    }

    if is_writable && !info.is_writable {
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(())
}

/// Errors if:
/// - Owner is not the sysvar address.
/// - Account cannot load with the expected address.
pub fn load_sysvar<'a, 'info>(
    info: &'a AccountInfo<'info>,
    key: Pubkey,
) -> Result<(), ProgramError> {
    if info.owner.ne(&sysvar::id()) {
        return Err(ProgramError::InvalidAccountOwner);
    }

    load_account(info, key, false)
}

/// Errors if:
/// - Address does not match the expected value.
/// - Expected to be writable, but is not.
pub fn load_account<'a, 'info>(
    info: &'a AccountInfo<'info>,
    key: Pubkey,
    is_writable: bool,
) -> Result<(), ProgramError> {
    if info.key.ne(&key) {
        return Err(ProgramError::InvalidAccountData);
    }

    if is_writable && !info.is_writable {
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(())
}

/// Errors if:
/// - Address does not match the expected value.
/// - Account is not executable.
pub fn load_program<'a, 'info>(
    info: &'a AccountInfo<'info>,
    key: Pubkey,
) -> Result<(), ProgramError> {
    if info.key.ne(&key) {
        return Err(ProgramError::IncorrectProgramId);
    }

    if !info.executable {
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(())
}

/// Errors if:
/// - Account is not writable.
pub fn load_any<'a, 'info>(
    info: &'a AccountInfo<'info>,
    is_writable: bool,
) -> Result<(), ProgramError> {
    if is_writable && !info.is_writable {
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(())
}

/// Errors if:
/// - Data is empty.
/// - Update authority is not the forge pickaxe collection.
/// - Attributes plugin is not present.
/// - Durability attribute is not present.
/// - Multiplier attribute is not present.
pub fn load_asset<'a, 'info>(
    info: &'a AccountInfo<'info>,
) -> Result<(f64, u64, String), ProgramError> {
    if info.owner.ne(&mpl_core::ID) {
        return Err(ProgramError::InvalidAccountOwner);
    }

    if info.data_is_empty() {
        return Err(ProgramError::UninitializedAccount);
    }

    let asset = Asset::from_bytes(&info.data.borrow()).unwrap();

    match asset.base.update_authority {
        UpdateAuthority::Collection(address) => {
            if address.ne(&FORGE_PICKAXE_COLLECTION) {
                msg!(
                    "Invalid collection: {:?} == {:?}",
                    address,
                    FORGE_PICKAXE_COLLECTION
                );
                return Err(ProgramError::InvalidAccountData);
            }
        }
        _ => return Err(ProgramError::InvalidAccountData),
    }

    if asset.plugin_list.attributes.is_none() {
        return Err(ProgramError::InvalidAccountData);
    }

    let attributes_plugin = asset.plugin_list.attributes.unwrap();
    let durability_attr = attributes_plugin
        .attributes
        .attribute_list
        .iter()
        .find(|attr| attr.key == "durability");
    let multiplier_attr = attributes_plugin
        .attributes
        .attribute_list
        .iter()
        .find(|attr| attr.key == "multiplier");
    let resource_attr = attributes_plugin
        .attributes
        .attribute_list
        .iter()
        .find(|attr| attr.key == "resource");
    let durability = durability_attr.unwrap().value.parse::<f64>().unwrap();
    let multiplier = multiplier_attr.unwrap().value.parse::<u64>().unwrap();
    let resource = resource_attr.unwrap().value.clone();

    Ok((durability, multiplier, resource))
}

pub fn load_tool<'a, 'info>(
    info: &'a AccountInfo<'info>,
    miner: &Pubkey,
    is_writable: bool,
) -> Result<(u64, u64), ProgramError> {
    if info.owner.ne(&super::id()) {
        return Err(ProgramError::InvalidAccountOwner);
    }

    if info.data_is_empty() {
        return Err(ProgramError::UninitializedAccount);
    }

    let tool_data = info.data.borrow();
    let tool = Tool::try_from_bytes(&tool_data).unwrap();

    if tool.miner.ne(&miner) {
        return Err(ProgramError::InvalidAccountData);
    }

    if is_writable && !info.is_writable {
        return Err(ProgramError::InvalidAccountData);
    }

    Ok((tool.durability, tool.multiplier))
}

pub fn is_tool<'a, 'info>(info: &'a AccountInfo<'info>) -> bool {
    info.data.borrow()[0].eq(&(Tool::discriminator() as u8))
}

pub fn load_wood_tool<'a, 'info>(
    info: &'a AccountInfo<'info>,
    miner: &Pubkey,
    is_writable: bool,
) -> Result<(u64, u64), ProgramError> {
    if info.owner.ne(&super::id()) {
        return Err(ProgramError::InvalidAccountOwner);
    }

    if info.data_is_empty() {
        return Err(ProgramError::UninitializedAccount);
    }

    let tool_data = info.data.borrow();
    let tool = WoodTool::try_from_bytes(&tool_data).unwrap();

    if tool.miner.ne(&miner) {
        return Err(ProgramError::InvalidAccountData);
    }

    if is_writable && !info.is_writable {
        return Err(ProgramError::InvalidAccountData);
    }

    Ok((tool.durability, tool.multiplier))
}

pub fn load_any_tool_with_asset<'a, 'info>(
    info: &'a AccountInfo<'info>,
    miner: &Pubkey,
    asset: &Pubkey,
    is_writable: bool,
) -> Result<u64, ProgramError> {
    if info.owner.ne(&super::id()) {
        return Err(ProgramError::InvalidAccountOwner);
    }

    if info.data_is_empty() {
        return Err(ProgramError::UninitializedAccount);
    }

    if is_writable && !info.is_writable {
        return Err(ProgramError::InvalidAccountData);
    }

    let tool_data = info.data.borrow();

    match tool_data[0] {
        d if d == Tool::discriminator() as u8 => {
            let tool = Tool::try_from_bytes(&tool_data).unwrap();

            if tool.miner.ne(&miner) {
                return Err(ProgramError::InvalidAccountData);
            }

            if tool.asset.ne(&asset) {
                return Err(ProgramError::InvalidAccountData);
            }

            return Ok(tool.durability);
        }
        d if d == WoodTool::discriminator() as u8 => {
            let tool = WoodTool::try_from_bytes(&tool_data).unwrap();

            if tool.miner.ne(&miner) {
                return Err(ProgramError::InvalidAccountData);
            }

            if tool.asset.ne(&asset) {
                return Err(ProgramError::InvalidAccountData);
            }

            return Ok(tool.durability);
        }
        _ => Err(ProgramError::InvalidAccountData),
    }
}

pub fn amount_u64_to_f64(amount: u64) -> f64 {
    (amount as f64) / 10f64.powf(TOKEN_DECIMALS as f64)
}

pub fn amount_f64_to_u64(amount: f64) -> u64 {
    (amount * 10f64.powf(TOKEN_DECIMALS as f64)) as u64
}
