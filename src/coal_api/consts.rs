use array_const_fn_init::array_const_fn_init;
use const_crypto::ed25519;
use solana_program::{native_token::LAMPORTS_PER_SOL, pubkey, pubkey::Pubkey};

/// The authority allowed to initialize the program.
pub const INITIALIZER_ADDRESS: Pubkey = pubkey!("FJka1yJHn1SWux2X1o8VqHC8uaAWGv6CbNQvPWLJQufq");

/// The base reward rate to intialize the program with.
pub const INITIAL_BASE_COAL_REWARD_RATE: u64 = BASE_COAL_REWARD_RATE_MIN_THRESHOLD;
pub const INITIAL_BASE_WOOD_REWARD_RATE: u64 = BASE_WOOD_REWARD_RATE_MIN_THRESHOLD;

/// The minimum allowed base reward rate, at which point the min difficulty should be increased
pub const BASE_COAL_REWARD_RATE_MIN_THRESHOLD: u64 =
    2u64.pow(5).saturating_mul(1000).saturating_div(128);

/// The maximum allowed base reward rate, at which point the min difficulty should be decreased.
pub const BASE_COAL_REWARD_RATE_MAX_THRESHOLD: u64 =
    2u64.pow(8).saturating_mul(1000).saturating_div(128);

/// The minimum allowed base reward rate, at which point the min difficulty should be increased
pub const BASE_WOOD_REWARD_RATE_MIN_THRESHOLD: u64 = 2u64.pow(5).saturating_mul(10);

/// The maximum allowed base reward rate, at which point the min difficulty should be decreased.
pub const BASE_WOOD_REWARD_RATE_MAX_THRESHOLD: u64 = 2u64.pow(8).saturating_mul(10);

/// The spam/liveness tolerance in seconds.
pub const TOLERANCE: i64 = 5;

pub const REPROCESS_TARGET_SLOT: u64 = 20;
pub const REPROCESS_SLOT_BUFFER: u64 = 6;
pub const REPROCESS_MAX_MULTIPLIER: u64 = 100;
pub const REPROCESS_FEE: u64 = LAMPORTS_PER_SOL / 200;

/// The liveness tolerance for WOOD in seconds.
pub const WOOD_LIVENESS_TOLERANCE: i64 = 65;

/// The minimum difficulty to initialize the program with.
pub const INITIAL_MIN_DIFFICULTY: u32 = 1;

/// The decimal precision of the COAL token.
/// There are 100 billion indivisible units per COAL (called "grains").
pub const TOKEN_DECIMALS: u8 = 11;

/// One COAL token, denominated in indivisible units.
pub const ONE_COAL: u64 = 10u64.pow(TOKEN_DECIMALS as u32);

/// One WOOD token, denominated in indivisible units.
pub const ONE_WOOD: u64 = 10u64.pow(TOKEN_DECIMALS as u32);

/// The duration of one minute, in seconds.
pub const ONE_MINUTE: i64 = 60;

/// The number of minutes in a program epoch.
pub const EPOCH_MINUTES: i64 = 5;

/// The duration of a program epoch, in seconds.
pub const COAL_EPOCH_DURATION: i64 = ONE_MINUTE * EPOCH_MINUTES;
pub const WOOD_EPOCH_DURATION: i64 = ONE_MINUTE * EPOCH_MINUTES;
/// The maximum token supply (21 million).
pub const MAX_COAL_SUPPLY: u64 = ONE_COAL * 21_000_000;

/// The target quantity of COAL to be mined per minute.
pub const TARGET_COAL_REWARDS: u64 = ONE_COAL.saturating_mul(1000).saturating_div(128);

/// The target quantity of COAL to be mined per epoch.
pub const TARGET_COAL_EPOCH_REWARDS: u64 = TARGET_COAL_REWARDS * EPOCH_MINUTES as u64;

/// The initial quantity of WOOD distributed to each bus (1000 WOOD).
pub const INITIAL_WOOD_EPOCH_REWARDS: u64 = ONE_WOOD.saturating_mul(1000);

/// The minimum rewards a bus can have for each epoch 0.1 WOOD
pub const MIN_WOOD_EPOCH_REWARDS: u64 = ONE_WOOD / 10;
/// The maximum rewards a bus can have for each epoch 4000 WOOD
pub const MAX_WOOD_EPOCH_REWARDS: u64 = ONE_WOOD * 4000;

/// WOOD propogation rate is 5% per epoch
/// New bus rewards = remaining + (remaining rewards / WOOD_PROPOGATION_RATE)
pub const WOOD_PROPOGATION_RATE: u64 = 20;

/// The maximum quantity of COAL that can be mined per epoch.
pub const MAX_COAL_EPOCH_REWARDS: u64 = TARGET_COAL_EPOCH_REWARDS * BUS_COUNT as u64;

/// The quantity of COAL each bus is allowed to issue per epoch.
pub const BUS_COAL_EPOCH_REWARDS: u64 = MAX_COAL_EPOCH_REWARDS / BUS_COUNT as u64;

/// The number of bus accounts, for parallelizing mine operations.
pub const BUS_COUNT: usize = 8;

/// The smoothing factor for reward rate changes. The reward rate cannot change by mCOAL or less
/// than a factor of this constant from one epoch to the next.
pub const SMOOTHING_FACTOR: u64 = 2;
// WOOD delcines at a faster rate to prevent busses emptying too quickly
pub const WOOD_DECREMENTAL_SMOOTHING_FACTOR: u64 = 10;

// Assert MAX_EPOCH_REWARDS is evenly divisible by BUS_COUNT.
static_assertions::const_assert!(
    (MAX_COAL_EPOCH_REWARDS / BUS_COUNT as u64) * BUS_COUNT as u64 == MAX_COAL_EPOCH_REWARDS
);

/// The seed of the bus account PDA.
pub const COAL_BUS: &[u8] = b"bus";
pub const WOOD_BUS: &[u8] = b"wood_bus";

/// The seed of the config account PDA.
pub const COAL_CONFIG: &[u8] = b"config";
pub const WOOD_CONFIG: &[u8] = b"wood_config";

/// The seed of the metadata account PDA.
pub const METADATA: &[u8] = b"metadata";

/// The seed of the mint account PDA.
pub const COAL_MINT: &[u8] = b"mint";
pub const WOOD_MINT: &[u8] = b"wood_mint";
pub const CHROMIUM_MINT: &[u8] = b"chromium_mint";

/// The seed of proof account PDAs.
pub const COAL_PROOF: &[u8] = b"proof";
pub const WOOD_PROOF: &[u8] = b"wood_proof";

/// The seed of the tool account PDA.
pub const COAL_MAIN_HAND_TOOL: &[u8] = b"coal_main_hand_tool";
pub const WOOD_MAIN_HAND_TOOL: &[u8] = b"wood_main_hand_tool";

/// The seed of the treasury account PDA.
pub const TREASURY: &[u8] = b"treasury";

/// The seed of the plugin update authority PDA.
pub const PLUGIN_UPDATE_AUTHORITY: &[u8] = b"update_authority";

/// The seed of the reprocessor PDA.
pub const REPROCESSOR: &[u8] = b"reprocessor";

/// Noise for deriving the mint pda
pub const MINT_NOISE: [u8; 16] = [
    89, 157, 88, 232, 243, 249, 197, 132, 199, 49, 19, 234, 91, 94, 150, 41,
];

/// The name for token metadata.
pub const COAL_METADATA_NAME: &str = "coal";
pub const WOOD_METADATA_NAME: &str = "wood";
pub const CHROMIUM_METADATA_NAME: &str = "chromium";

/// The ticker symbol for token metadata.
pub const COAL_METADATA_SYMBOL: &str = "COAL";
pub const WOOD_METADATA_SYMBOL: &str = "WOOD";
pub const CHROMIUM_METADATA_SYMBOL: &str = "CHROMIUM";

/// The uri for token metdata.
pub const COAL_METADATA_URI: &str = "https://coal.digital/metadata.json";
pub const WOOD_METADATA_URI: &str = "https://coal.digital/metadata.wood.json";
pub const CHROMIUM_METADATA_URI: &str = "https://coal.digital/metadata.chromium.json";

/// Program id for const pda derivations
const PROGRAM_ID: [u8; 32] = unsafe { *(&super::id() as *const Pubkey as *const [u8; 32]) };

/// ORE program id
pub const ORE_PROGRAM_ID: Pubkey = pubkey!("oreV2ZymfyeXgNgBdqMkumTqqAprVqgBWQfoYkrtKWQ");
pub const ORE_PROGRAM_ID_BYTES: [u8; 32] =
    unsafe { *(&ORE_PROGRAM_ID as *const Pubkey as *const [u8; 32]) };

/// Forge collection ids
pub const FORGE_PICKAXE_COLLECTION: Pubkey =
    pubkey!("CuaLHUJA1dyQ6AYcTcMZrCoBqssSJbqkY7VfEEFdxzCk");
pub const BASE_TOOL_MULTIPLIER: u64 = 300;
pub const MAX_TOOL_MULTIPLIER: u64 = 600;

/// The addresses of the bus accounts.
pub const COAL_BUS_ADDRESSES: [Pubkey; BUS_COUNT] = array_const_fn_init![const_coal_bus_address; 8];
pub const WOOD_BUS_ADDRESSES: [Pubkey; BUS_COUNT] = array_const_fn_init![const_wood_bus_address; 8];

/// Function to derive const bus addresses.
const fn const_coal_bus_address(i: usize) -> Pubkey {
    Pubkey::new_from_array(ed25519::derive_program_address(&[COAL_BUS, &[i as u8]], &PROGRAM_ID).0)
}

const fn const_wood_bus_address(i: usize) -> Pubkey {
    Pubkey::new_from_array(ed25519::derive_program_address(&[WOOD_BUS, &[i as u8]], &PROGRAM_ID).0)
}

/// The address of the config account.
pub const COAL_CONFIG_ADDRESS: Pubkey =
    Pubkey::new_from_array(ed25519::derive_program_address(&[COAL_CONFIG], &PROGRAM_ID).0);
pub const WOOD_CONFIG_ADDRESS: Pubkey =
    Pubkey::new_from_array(ed25519::derive_program_address(&[WOOD_CONFIG], &PROGRAM_ID).0);

/// The address of the mint metadata account.
pub const COAL_METADATA_ADDRESS: Pubkey = Pubkey::new_from_array(
    ed25519::derive_program_address(
        &[
            METADATA,
            unsafe { &*(&mpl_token_metadata::ID as *const Pubkey as *const [u8; 32]) },
            unsafe { &*(&COAL_MINT_ADDRESS as *const Pubkey as *const [u8; 32]) },
        ],
        unsafe { &*(&mpl_token_metadata::ID as *const Pubkey as *const [u8; 32]) },
    )
    .0,
);
pub const WOOD_METADATA_ADDRESS: Pubkey = Pubkey::new_from_array(
    ed25519::derive_program_address(
        &[
            METADATA,
            unsafe { &*(&mpl_token_metadata::ID as *const Pubkey as *const [u8; 32]) },
            unsafe { &*(&WOOD_MINT_ADDRESS as *const Pubkey as *const [u8; 32]) },
        ],
        unsafe { &*(&mpl_token_metadata::ID as *const Pubkey as *const [u8; 32]) },
    )
    .0,
);

/// The address of the COAL mint account.
pub const COAL_MINT_ADDRESS: Pubkey = Pubkey::new_from_array(
    ed25519::derive_program_address(&[COAL_MINT, &MINT_NOISE], &PROGRAM_ID).0,
);

/// The address of the WOOD mint account.
pub const WOOD_MINT_ADDRESS: Pubkey = Pubkey::new_from_array(
    ed25519::derive_program_address(&[WOOD_MINT, &MINT_NOISE], &PROGRAM_ID).0,
);

/// The address of the CHROMIUM mint account.
pub const CHROMIUM_MINT_ADDRESS: Pubkey = Pubkey::new_from_array(
    ed25519::derive_program_address(&[CHROMIUM_MINT, &MINT_NOISE], &PROGRAM_ID).0,
);

/// The address of the treasury account.
pub const TREASURY_ADDRESS: Pubkey =
    Pubkey::new_from_array(ed25519::derive_program_address(&[TREASURY], &PROGRAM_ID).0);

/// The bump of the treasury account, for cpis.
pub const TREASURY_BUMP: u8 = ed25519::derive_program_address(&[TREASURY], &PROGRAM_ID).1;

/// The address of the COAL treasury token account.
pub const COAL_TREASURY_TOKENS_ADDRESS: Pubkey = Pubkey::new_from_array(
    ed25519::derive_program_address(
        &[
            unsafe { &*(&TREASURY_ADDRESS as *const Pubkey as *const [u8; 32]) },
            unsafe { &*(&spl_token::id() as *const Pubkey as *const [u8; 32]) },
            unsafe { &*(&COAL_MINT_ADDRESS as *const Pubkey as *const [u8; 32]) },
        ],
        unsafe { &*(&spl_associated_token_account::id() as *const Pubkey as *const [u8; 32]) },
    )
    .0,
);

/// The address of the WOOD treasury token account.
pub const WOOD_TREASURY_TOKENS_ADDRESS: Pubkey = Pubkey::new_from_array(
    ed25519::derive_program_address(
        &[
            unsafe { &*(&TREASURY_ADDRESS as *const Pubkey as *const [u8; 32]) },
            unsafe { &*(&spl_token::id() as *const Pubkey as *const [u8; 32]) },
            unsafe { &*(&WOOD_MINT_ADDRESS as *const Pubkey as *const [u8; 32]) },
        ],
        unsafe { &*(&spl_associated_token_account::id() as *const Pubkey as *const [u8; 32]) },
    )
    .0,
);

pub const COAL_MAIN_HAND_TOOL_ADDRESS: Pubkey =
    Pubkey::new_from_array(ed25519::derive_program_address(&[COAL_MAIN_HAND_TOOL], &PROGRAM_ID).0);

/// The address of the CU-optimized Solana noop program.
pub const NOOP_PROGRAM_ID: Pubkey = pubkey!("noop8ytexvkpCuqbf6FB89BSuNemHtPRqaNC31GWivW");
