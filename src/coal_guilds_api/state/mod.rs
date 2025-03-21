mod config;
mod guild;
mod invite;
mod member;

pub use config::*;
pub use guild::*;
pub use member::*;

use crate::coal_guilds_api::consts::*;
use steel::*;

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, IntoPrimitive, TryFromPrimitive)]
pub enum GuildsAccount {
    Config = 100,
    Guild = 101,
    Member = 102,
    Invite = 103,
}

/// Fetch the PDA of the config account.
pub fn config_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[CONFIG], &super::id())
}

/// Fetch the PDA of the boost account.
pub fn guild_pda(authority: Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[GUILD, authority.as_ref()], &super::id())
}

/// Fetch the PDA of the stake account.
pub fn member_pda(member: Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[MEMBER, member.as_ref()], &super::id())
}

/// Fetch the PDA of the stake account.
pub fn invite_pda(guild: Pubkey, member: Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[INVITE, guild.as_ref(), member.as_ref()], &super::id())
}
