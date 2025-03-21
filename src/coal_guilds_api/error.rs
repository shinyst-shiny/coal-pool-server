use steel::*;

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq, IntoPrimitive)]
#[repr(u32)]
pub enum GuildError {
    #[error("Too early to unstake")]
    TooEarly = 100,
    #[error("Invalid guild")]
    InvalidGuild = 101,
    #[error("Insufficient balance")]
    InsufficientBalance = 102,
}

error!(GuildError);