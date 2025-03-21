use crate::coal_guilds_api::consts::LP_MINT_ADDRESS;
use crate::coal_guilds_api::instruction::{
    Delegate, Initialize, Join, Leave, NewGuild, NewInvite, NewMember, Stake, Unstake,
};
use crate::coal_guilds_api::state::{config_pda, guild_pda, invite_pda, member_pda};
use steel::*;

// Build initialize instruction.
pub fn initialize(signer: Pubkey) -> Instruction {
    let config_pda = config_pda();
    Instruction {
        program_id: super::ID,
        accounts: vec![
            AccountMeta::new(signer, true),
            AccountMeta::new(config_pda.0, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: Initialize {
            config_bump: config_pda.1,
        }
        .to_bytes(),
    }
}

pub fn new_guild(signer: Pubkey) -> Instruction {
    let guild = guild_pda(signer);

    Instruction {
        program_id: super::ID,
        accounts: vec![
            AccountMeta::new(signer, true),
            AccountMeta::new(guild.0, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: NewGuild {
            guild_bump: guild.1,
        }
        .to_bytes(),
    }
}

pub fn new_member(signer: Pubkey) -> Instruction {
    let member = member_pda(signer);
    let stake_tokens_info =
        spl_associated_token_account::get_associated_token_address(&member.0, &LP_MINT_ADDRESS);

    Instruction {
        program_id: super::ID,
        accounts: vec![
            AccountMeta::new(signer, true),
            AccountMeta::new(member.0, false),
            AccountMeta::new(stake_tokens_info, false),
            AccountMeta::new_readonly(LP_MINT_ADDRESS, false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(spl_associated_token_account::ID, false),
        ],
        data: NewMember {
            member_bump: member.1,
        }
        .to_bytes(),
    }
}

pub fn invite(signer: Pubkey, address: Pubkey) -> Instruction {
    let guild = guild_pda(signer);
    let member = member_pda(address);
    let invite = invite_pda(guild.0, member.0);

    Instruction {
        program_id: super::ID,
        accounts: vec![
            AccountMeta::new(signer, true),
            AccountMeta::new_readonly(guild.0, false),
            AccountMeta::new(invite.0, false),
            AccountMeta::new_readonly(member.0, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: NewInvite {
            invite_bump: invite.1,
            guild_bump: guild.1,
        }
        .to_bytes(),
    }
}

pub fn join(signer: Pubkey, guild: Pubkey, guild_authority: Pubkey) -> Instruction {
    let member = member_pda(signer);
    let invite = invite_pda(guild, member.0);

    Instruction {
        program_id: super::ID,
        accounts: vec![
            AccountMeta::new(signer, true),
            AccountMeta::new(guild, false),
            AccountMeta::new(guild_authority, false),
            AccountMeta::new(invite.0, false),
            AccountMeta::new(member.0, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: Join {
            invite_bump: invite.1,
            member_bump: member.1,
        }
        .to_bytes(),
    }
}

pub fn delegate(signer: Pubkey, guild: Pubkey) -> Instruction {
    let member = member_pda(signer);

    Instruction {
        program_id: super::ID,
        accounts: vec![
            AccountMeta::new(signer, true),
            AccountMeta::new(guild, false),
            AccountMeta::new(member.0, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: Delegate {
            member_bump: member.1,
        }
        .to_bytes(),
    }
}

pub fn leave(signer: Pubkey, guild: Pubkey) -> Instruction {
    let member = member_pda(signer);

    Instruction {
        program_id: super::ID,
        accounts: vec![
            AccountMeta::new(signer, true),
            AccountMeta::new(guild, false),
            AccountMeta::new(member.0, false),
        ],
        data: Leave {}.to_bytes(),
    }
}

pub fn stake(signer: Pubkey, guild: Pubkey, amount: u64) -> Instruction {
    let config = config_pda();
    let member = member_pda(signer);
    let member_tokens =
        spl_associated_token_account::get_associated_token_address(&signer, &LP_MINT_ADDRESS);
    let stake_tokens_info =
        spl_associated_token_account::get_associated_token_address(&member.0, &LP_MINT_ADDRESS);

    Instruction {
        program_id: super::ID,
        accounts: vec![
            AccountMeta::new(signer, true),
            AccountMeta::new(config.0, false),
            AccountMeta::new(guild, false),
            AccountMeta::new(member.0, false),
            AccountMeta::new(member_tokens, false),
            AccountMeta::new(stake_tokens_info, false),
            AccountMeta::new_readonly(spl_token::ID, false),
        ],
        data: Stake {
            amount: amount.to_le_bytes(),
        }
        .to_bytes(),
    }
}

pub fn unstake(signer: Pubkey, guild: Pubkey, amount: u64) -> Instruction {
    let config = config_pda();
    let member = member_pda(signer);
    let member_tokens =
        spl_associated_token_account::get_associated_token_address(&signer, &LP_MINT_ADDRESS);
    let stake_tokens_info =
        spl_associated_token_account::get_associated_token_address(&member.0, &LP_MINT_ADDRESS);

    Instruction {
        program_id: super::ID,
        accounts: vec![
            AccountMeta::new(signer, true),
            AccountMeta::new(config.0, false),
            AccountMeta::new(guild, false),
            AccountMeta::new(member.0, false),
            AccountMeta::new(member_tokens, false),
            AccountMeta::new(stake_tokens_info, false),
            AccountMeta::new_readonly(spl_token::ID, false),
        ],
        data: Unstake {
            amount: amount.to_le_bytes(),
        }
        .to_bytes(),
    }
}
