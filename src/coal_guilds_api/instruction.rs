use steel::*;

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, TryFromPrimitive)]
#[rustfmt::skip]
pub enum GuildInstruction {
    // User
    Join = 100,
    Leave = 101,
    Stake = 102,
    Unstake = 103,
    Delegate = 107,
    // Guild
    NewMember = 104,
    NewGuild = 105,
    NewInvite = 106,
    // Admin
    Initialize = 200,
}

impl GuildInstruction {
    pub fn to_vec(&self) -> Vec<u8> {
        vec![*self as u8]
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Initialize {
    pub config_bump: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct NewGuild {
    pub guild_bump: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct NewMember {
    pub member_bump: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct NewInvite {
    pub invite_bump: u8,
    pub guild_bump: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Join {
    pub invite_bump: u8,
    pub member_bump: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Delegate {
    pub member_bump: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Leave {}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Stake {
    pub amount: [u8; 8],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Unstake {
    pub amount: [u8; 8],
}

instruction!(GuildInstruction, Initialize);
instruction!(GuildInstruction, Delegate);
instruction!(GuildInstruction, NewGuild);
instruction!(GuildInstruction, NewMember);
instruction!(GuildInstruction, NewInvite);
instruction!(GuildInstruction, Join);
instruction!(GuildInstruction, Leave);
instruction!(GuildInstruction, Stake);
instruction!(GuildInstruction, Unstake);