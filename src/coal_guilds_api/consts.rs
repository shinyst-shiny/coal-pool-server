use const_crypto::ed25519;
use solana_sdk::pubkey;
use solana_sdk::pubkey::Pubkey;

/// The authority allowed to initialize the program.
pub const INITIALIZER_ADDRESS: Pubkey = pubkey!("FJka1yJHn1SWux2X1o8VqHC8uaAWGv6CbNQvPWLJQufq");

/// The seed of the config PDA.
pub const CONFIG: &[u8] = b"config";

/// The seed of the guild PDA.
pub const GUILD: &[u8] = b"guild";

/// The seed of the invite PDA.
pub const INVITE: &[u8] = b"invite";

/// The seed of the member PDA.
pub const MEMBER: &[u8] = b"member";

/// Program ID for const pda derivations
const PROGRAM_ID: [u8; 32] = unsafe { *(&super::id() as *const Pubkey as *const [u8; 32]) };

/// The address of the config account.
pub const CONFIG_ADDRESS: Pubkey =
    Pubkey::new_from_array(ed25519::derive_program_address(&[CONFIG], &PROGRAM_ID).0);

pub const LP_MINT_ADDRESS: Pubkey = pubkey!("AtDMJwa4j5w2nKTnzWrxkHHwqfsEvLniErVDcgNNeSft");
pub const LP_MINT_DECIMALS: u8 = 11;

pub const UNSTAKE_DELAY: i64 = 60 * 60; // 1 hour

pub const LEAVE_DELAY: i64 = 60 * 60; // 1 hour
