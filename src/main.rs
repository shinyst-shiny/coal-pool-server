use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    ops::ControlFlow,
    path::Path,
    str::FromStr,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use systems::{
    claim_system::claim_system, client_message_handler_system::client_message_handler_system,
    handle_ready_clients_system::handle_ready_clients_system,
    pong_tracking_system::pong_tracking_system, proof_tracking_system::proof_tracking_system,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer};

use crate::systems::{
    message_text_all_clients_system::message_text_all_clients_system,
    pool_mine_success_system::pool_mine_success_system,
    pool_submission_system::pool_submission_system,
};

use steel::AccountDeserialize;

use self::models::*;
use crate::coal_utils::{
    amount_u64_to_string, calculate_multiplier, calculate_tool_multiplier, deserialize_guild,
    deserialize_guild_config, deserialize_guild_member, deserialize_tool, get_chromium_mint,
    get_config_pubkey, get_proof_and_config_with_busses as get_proof_and_config_with_busses_coal,
    get_tool_pubkey, proof_pubkey, Resource, ToolType,
};
use crate::ore_utils::{
    get_ore_mint, get_proof_and_config_with_busses as get_proof_and_config_with_busses_ore,
};
use crate::routes::get_guild_addresses;
use crate::send_and_confirm::{send_and_confirm, ComputeBudget};
use crate::systems::chromium_reprocessing_system::chromium_reprocessing_system;
use app_database::{AppDatabase, AppDatabaseError};
use app_rr_database::AppRRDatabase;
use axum::{
    extract::{
        ws::{Message, WebSocket},
        ConnectInfo, Query, State, WebSocketUpgrade,
    },
    http::{Method, Response, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Extension, Json, Router,
};
use axum_extra::{headers::authorization::Basic, TypedHeader};
use base64::engine::general_purpose;
use base64::{prelude::BASE64_STANDARD, Engine};
use chrono::NaiveDateTime;
use clap::builder::TypedValueParser;
use clap::Parser;
use coal_api::consts::COAL_MINT_ADDRESS;
use coal_guilds_api::consts::LP_MINT_ADDRESS;
use coal_guilds_api::state::member_pda;
use coal_guilds_api::state::Member;
use coal_utils::{get_coal_mint, get_config, get_proof, get_register_ix, COAL_TOKEN_DECIMALS};
use drillx::Solution;
use futures::{stream::SplitSink, SinkExt, StreamExt};
use ore_api::prelude::Proof;
use ore_utils::get_ore_register_ix;
use routes::{get_challenges, get_latest_mine_txn};
use serde::{Deserialize, Serialize};
use solana_account_decoder::StringDecimals;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_client::rpc_config::RpcSendTransactionConfig;
use solana_sdk::instruction::{CompiledInstruction, Instruction};
use solana_sdk::program_error::ProgramError;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    compute_budget::ComputeBudgetInstruction,
    native_token::{lamports_to_sol, LAMPORTS_PER_SOL},
    pubkey::Pubkey,
    signature::{read_keypair_file, Keypair, Signature},
    signer::Signer,
    sysvar,
    transaction::Transaction,
};
use spl_associated_token_account::get_associated_token_address;
use spl_token::instruction::TokenInstruction;
use tokio::{
    sync::{mpsc::UnboundedSender, Mutex, RwLock},
    time::Instant,
};
use tower_http::{
    cors::CorsLayer,
    trace::{DefaultMakeSpan, TraceLayer},
};
use tracing::{error, info};

mod app_database;
mod app_rr_database;
mod message;
mod models;
mod proof_migration;
mod routes;
mod schema;
mod systems;

const MIN_DIFF: u32 = 12;
const MIN_HASHPOWER: u64 = 80; // difficulty 12
const MAX_CALCULATED_HASHPOWER: u64 = 327_680; // difficulty 24

#[derive(Clone)]
enum ClientVersion {
    V2,
}

#[derive(Clone)]
struct AppClientConnection {
    pubkey: Pubkey,
    miner_id: i32,
    client_version: ClientVersion,
    socket: Arc<Mutex<SplitSink<WebSocket, Message>>>,
}

#[derive(Clone)]
struct WalletExtension {
    miner_wallet: Arc<Keypair>,
    fee_wallet: Arc<Keypair>,
}

struct AppState {
    sockets: HashMap<SocketAddr, AppClientConnection>,
    paused: bool,
}

#[derive(Clone, Copy)]
struct ClaimsQueueItem {
    receiver_pubkey: Pubkey,
    amount_coal: u64,
    amount_ore: u64,
    amount_chromium: u64,
}

struct ClaimsQueue {
    queue: RwLock<HashMap<Pubkey, ClaimsQueueItem>>,
}

struct SubmissionWindow {
    closed: bool,
}

pub struct MessageInternalAllClients {
    text: String,
}

#[derive(Debug, Clone, Copy)]
pub struct InternalMessageSubmission {
    miner_id: i32,
    supplied_diff: u32,
    supplied_nonce: u64,
    hashpower: u64,
    real_hashpower: u64,
}

pub struct PoolGuildMember {
    stake_percentage: u128,
    member: coal_guilds_api::state::Member,
}

pub struct MessageInternalMineSuccess {
    difficulty: u32,
    total_balance_coal: f64,
    total_balance_ore: f64,
    rewards_coal: u64,
    commissions_coal: u64,
    rewards_ore: u64,
    commissions_ore: u64,
    challenge_id: i32,
    challenge: [u8; 32],
    best_nonce: u64,
    total_hashpower: u64,
    total_real_hashpower: u64,
    coal_config: Option<coal_api::state::Config>,
    multiplier: f64,
    submissions: HashMap<Pubkey, InternalMessageSubmission>,
    guild_total_stake: f64,
    guild_multiplier: f64,
    tool_multiplier: f64,
    guild_stake_rewards_coal: u64,
    guild_members: Vec<PoolGuildMember>,
}

pub struct LastPong {
    pongs: HashMap<SocketAddr, Instant>,
}

#[derive(Debug)]
pub enum ClientMessage {
    Ready(SocketAddr),
    Mining(SocketAddr),
    Pong(SocketAddr),
    BestSolution(SocketAddr, Solution, Pubkey),
}

pub struct EpochHashes {
    challenge: [u8; 32],
    best_hash: BestHash,
    submissions: HashMap<Pubkey, InternalMessageSubmission>,
}

pub struct BestHash {
    solution: Option<Solution>,
    difficulty: u32,
}

pub struct Config {
    password: String,
    pool_id: i32,
    stats_enabled: bool,
    signup_fee: f64,
    commissions_pubkey: String,
    commissions_miner_id: i32,
    guild_address: String,
}

mod coal_utils;
mod ore_utils;
mod send_and_confirm;

#[derive(Parser, Debug)]
#[command(version, author, about, long_about = None)]
struct Args {
    #[arg(
        long,
        value_name = "priority fee",
        help = "Number of microlamports to pay as priority fee per transaction",
        default_value = "10000",
        global = true
    )]
    priority_fee: u64,
    #[arg(
        long,
        value_name = "jito tip",
        help = "Number of lamports to pay as jito tip per transaction",
        default_value = "0",
        global = true
    )]
    jito_tip: u64,
    #[arg(
        long,
        value_name = "signup fee",
        help = "Amount of sol users must send to sign up for the pool",
        default_value = "0",
        global = true
    )]
    signup_fee: f64,
    #[arg(long, short, action, help = "Enable stats endpoints")]
    stats: bool,
    #[arg(
        long,
        short,
        action,
        help = "Migrate balance from original proof to delegate stake managed proof"
    )]
    migrate: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    let args = Args::parse();

    let server_logs = tracing_appender::rolling::daily("./logs", "coal-pool-server.log");
    let (server_logs, _guard) = tracing_appender::non_blocking(server_logs);
    let server_log_layer = tracing_subscriber::fmt::layer()
        .with_writer(server_logs)
        .with_filter(tracing_subscriber::filter::filter_fn(|metadata| {
            metadata.target() == "server_log"
        }));

    let submission_logs = tracing_appender::rolling::daily("./logs", "coal-pool-submissions.log");
    let (submission_logs, _guard) = tracing_appender::non_blocking(submission_logs);
    let submission_log_layer = tracing_subscriber::fmt::layer()
        .with_writer(submission_logs)
        .with_filter(tracing_subscriber::filter::filter_fn(|metadata| {
            metadata.target() == "submission_log"
        }));

    // Uncomment if you need console logging
    let console_log_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false) // disable ANSI color codes
        .with_filter(tracing_subscriber::filter::filter_fn(|metadata| {
            metadata.target() == "server_log" || metadata.target() == "submission_log"
        }));

    tracing_subscriber::registry()
        .with(server_log_layer)
        .with(submission_log_layer)
        .with(console_log_layer)
        .init();

    // load envs
    let wallet_path_str = std::env::var("WALLET_PATH").expect("WALLET_PATH must be set.");
    let fee_wallet_path_str =
        std::env::var("FEE_WALLET_PATH").expect("FEE_WALLET_PATH must be set.");
    let rpc_url = std::env::var("RPC_URL").expect("RPC_URL must be set.");
    let rpc_ws_url = std::env::var("RPC_WS_URL").expect("RPC_WS_URL must be set.");
    let rpc_url_miner = std::env::var("RPC_URL_MINER").expect("RPC_URL must be set.");
    let password = std::env::var("PASSWORD").expect("PASSWORD must be set.");
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set.");
    let database_rr_url = std::env::var("DATABASE_RR_URL").expect("DATABASE_RR_URL must be set.");
    let commission_env =
        std::env::var("COMMISSION_PUBKEY").expect("COMMISSION_PUBKEY must be set.");
    let commission_pubkey = match Pubkey::from_str(&commission_env) {
        Ok(pk) => pk,
        Err(_) => {
            println!("Invalid COMMISSION_PUBKEY");
            return Ok(());
        }
    };
    let guild_env = std::env::var("GUILD_ADDRESS").expect("GUILD_ADDRESS must be set.");

    let disable_reprocess_string =
        std::env::var("DISABLE_REPROCESS").expect("DISABLE_REPROCESS must be set.");
    let disable_reprocess = disable_reprocess_string == "true";

    let app_database = Arc::new(AppDatabase::new(database_url));
    let app_rr_database = Arc::new(AppRRDatabase::new(database_rr_url));

    let priority_fee = Arc::new(args.priority_fee);
    let jito_tip = Arc::new(args.jito_tip);

    // load wallet
    let wallet_path = Path::new(&wallet_path_str);

    if !wallet_path.exists() {
        tracing::error!(target: "server_log", "Failed to load wallet at: {}", wallet_path_str);
        return Err("Failed to find wallet path.".into());
    }

    let wallet = read_keypair_file(wallet_path)
        .expect("Failed to load keypair from file: {wallet_path_str}");
    info!(target: "server_log", "loaded wallet {}", wallet.pubkey().to_string());

    let wallet_path = Path::new(&fee_wallet_path_str);

    if !wallet_path.exists() {
        tracing::error!(target: "server_log", "Failed to load fee wallet at: {}", fee_wallet_path_str);
        return Err("Failed to find fee wallet path.".into());
    }

    let fee_wallet = read_keypair_file(wallet_path)
        .expect("Failed to load keypair from file: {wallet_path_str}");
    info!(target: "server_log", "loaded fee wallet {}", fee_wallet.pubkey().to_string());

    info!(target: "server_log", "establishing rpc connection...");
    let rpc_client = RpcClient::new_with_commitment(rpc_url, CommitmentConfig::confirmed());
    let rpc_client_miner =
        RpcClient::new_with_commitment(rpc_url_miner, CommitmentConfig::confirmed());
    let jito_url = "https://mainnet.block-engine.jito.wtf/api/v1/transactions".to_string();
    let jito_client = RpcClient::new(jito_url);

    info!(target: "server_log", "loading sol balance...");
    let balance = if let Ok(balance) = rpc_client.get_balance(&wallet.pubkey()).await {
        balance
    } else {
        return Err("Failed to load balance".into());
    };

    info!(target: "server_log", "Balance: {:.2}", balance as f64 / LAMPORTS_PER_SOL as f64);

    if balance < 1_000_000 {
        return Err("Sol balance is too low!".into());
    }

    let proof = if let Ok(loaded_proof) = get_proof(&rpc_client, wallet.pubkey()).await {
        info!(target: "server_log", "LOADED PROOF: \n{:?}", loaded_proof);
        loaded_proof
    } else {
        error!(target: "server_log", "Failed to load proof.");
        info!(target: "server_log", "Creating proof account...");

        let coal_register_ix = get_register_ix(wallet.pubkey());
        let ore_register_ix = get_ore_register_ix(wallet.pubkey());

        if let Ok((hash, _slot)) = rpc_client
            .get_latest_blockhash_with_commitment(rpc_client.commitment())
            .await
        {
            let mut tx = Transaction::new_with_payer(
                &[coal_register_ix, ore_register_ix],
                Some(&wallet.pubkey()),
            );

            tx.sign(&[&wallet], hash);

            let result = rpc_client
                .send_and_confirm_transaction_with_spinner_and_commitment(
                    &tx,
                    rpc_client.commitment(),
                )
                .await;

            if let Ok(sig) = result {
                info!(target: "server_log", "Sig: {}", sig.to_string());
            } else {
                return Err("Failed to create proof account".into());
            }
        }
        let proof = if let Ok(loaded_proof) = get_proof(&rpc_client, wallet.pubkey()).await {
            loaded_proof
        } else {
            return Err("Failed to get newly created proof".into());
        };
        proof
    };

    /*info!(target: "server_log", "Validating miners delegate stake account is created");
    match get_delegated_stake_account(&rpc_client, wallet.pubkey(), wallet.pubkey()).await {
        Ok(data) => {
            info!(target: "server_log", "Miner Delegated Stake Account: {:?}", data);
            info!(target: "server_log", "Miner delegate stake account already created.");
        }
        Err(_) => {
            info!(target: "server_log", "Creating miner delegate stake account");
            let ix = coal_miner_delegation::instruction::init_delegate_stake(
                wallet.pubkey(),
                wallet.pubkey(),
                wallet.pubkey(),
            );

            let mut tx = Transaction::new_with_payer(&[ix], Some(&wallet.pubkey()));

            let blockhash = rpc_client
                .get_latest_blockhash()
                .await
                .expect("should get latest blockhash");

            tx.sign(&[&wallet], blockhash);

            match rpc_client
                .send_and_confirm_transaction_with_spinner_and_commitment(
                    &tx,
                    CommitmentConfig {
                        commitment: CommitmentLevel::Confirmed,
                    },
                )
                .await
            {
                Ok(_) => {
                    info!(target: "server_log", "Successfully created miner delegate stake account");
                }
                Err(_) => {
                    error!(target: "server_log", "Failed to send and confirm tx.");
                    panic!("Failed to create miner delegate stake account");
                }
            }
        }
    }

    info!(target: "server_log", "Validating managed proof token account is created");
    let managed_proof = Pubkey::find_program_address(
        &[b"managed-proof-account", wallet.pubkey().as_ref()],
        &coal_miner_delegation::id(),
    );

    let managed_proof_token_account_addr = get_managed_proof_token_ata(wallet.pubkey());
    match rpc_client
        .get_token_account_balance(&managed_proof_token_account_addr)
        .await
    {
        Ok(_) => {
            info!(target: "server_log", "Managed proof token account already created.");
        }
        Err(_) => {
            info!(target: "server_log", "Creating managed proof token account");
            let ix = create_associated_token_account(
                &wallet.pubkey(),
                &managed_proof.0,
                &coal_api::consts::MINT_ADDRESS,
                &spl_token::id(),
            );

            let mut tx = Transaction::new_with_payer(&[ix], Some(&wallet.pubkey()));

            let blockhash = rpc_client
                .get_latest_blockhash()
                .await
                .expect("should get latest blockhash");

            tx.sign(&[&wallet], blockhash);

            match rpc_client
                .send_and_confirm_transaction_with_spinner_and_commitment(
                    &tx,
                    CommitmentConfig {
                        commitment: CommitmentLevel::Confirmed,
                    },
                )
                .await
            {
                Ok(_) => {
                    info!(target: "server_log", "Successfully created managed proof token account");
                }
                Err(e) => {
                    error!(target: "server_log", "Failed to send and confirm tx.\nE: {:?}", e);
                    panic!("Failed to create managed proof token account");
                }
            }
        }
    }

    let miner_coal_token_account_addr =
        get_associated_token_address(&wallet.pubkey(), &coal_api::consts::MINT_ADDRESS);
    let token_balance = if let Ok(token_balance) = rpc_client
        .get_token_account_balance(&miner_coal_token_account_addr)
        .await
    {
        let bal = token_balance.ui_amount.unwrap() * 10f64.powf(token_balance.decimals as f64);
        bal as u64
    } else {
        error!(target: "server_log", "Failed to get miner coal token account balance");
        panic!("Failed to get coal token account balance.");
    };

    if args.migrate {
        info!(target: "server_log", "Checking original proof, and token account balances for migration.");
        let original_proof =
            if let Ok(loaded_proof) = get_original_proof(&rpc_client, wallet.pubkey()).await {
                loaded_proof
            } else {
                panic!("Failed to get original proof!");
            };
        if original_proof.balance > 0 || token_balance > 0 {
            info!(target: "server_log", "Proof balance has {} tokens. Miner coal token account has {} tokens.\nMigrating...", original_proof.balance, token_balance);
            if let Err(e) = proof_migration::migrate(
                &rpc_client,
                &wallet,
                original_proof.balance,
                token_balance,
            )
            .await
            {
                info!(target: "server_log", "Failed to migrate proof balance.\nError: {}", e);
                panic!("Failed to migrate proof balance.");
            } else {
                info!(target: "server_log", "Successfully migrated proof balance");
            }
        } else {
            info!(target: "server_log", "Balances are 0, nothing to migrate.");
        }
    }*/

    info!(target: "server_log", "Validating pool exists in db");
    let db_pool = app_database
        .get_pool_by_authority_pubkey(wallet.pubkey().to_string())
        .await;

    match db_pool {
        Ok(_) => {}
        Err(AppDatabaseError::FailedToGetConnectionFromPool) => {
            panic!("Failed to get database pool connection");
        }
        Err(_) => {
            info!(target: "server_log", "Pool missing from database. Inserting...");
            let proof_pubkey = proof_pubkey(wallet.pubkey(), Resource::Coal);
            let result = app_database
                .add_new_pool(wallet.pubkey().to_string(), proof_pubkey.to_string())
                .await;

            if result.is_err() {
                panic!("Failed to create pool in database");
            }
        }
    }

    info!(target: "server_log", "Validating commissions receiver is in db");
    let commission_miner_id;
    match app_database
        .get_miner_by_pubkey_str(commission_pubkey.to_string())
        .await
    {
        Ok(miner) => {
            info!(target: "server_log", "Found commissions receiver in db.");
            commission_miner_id = miner.id;
        }
        Err(_) => {
            info!(target: "server_log", "Failed to get commissions receiver account from database.");
            info!(target: "server_log", "Inserting Commissions receiver account...");

            match app_database
                .signup_user_transaction(commission_pubkey.to_string(), wallet.pubkey().to_string())
                .await
            {
                Ok(_) => {
                    info!(target: "server_log", "Successfully inserted Commissions receiver account...");
                    if let Ok(m) = app_database
                        .get_miner_by_pubkey_str(commission_pubkey.to_string())
                        .await
                    {
                        commission_miner_id = m.id;
                    } else {
                        panic!("Failed to get commission miner id")
                    }
                }
                Err(_) => {
                    panic!("Failed to insert comissions receiver account")
                }
            }
        }
    }

    let db_pool = app_database
        .get_pool_by_authority_pubkey(wallet.pubkey().to_string())
        .await
        .unwrap();

    info!(target: "server_log", "Validating current challenge for pool exists in db");
    let result = app_database
        .get_challenge_by_challenge(proof.challenge.to_vec())
        .await;

    match result {
        Ok(_) => {}
        Err(AppDatabaseError::FailedToGetConnectionFromPool) => {
            panic!("Failed to get database pool connection");
        }
        Err(_) => {
            info!(target: "server_log", "Challenge missing from database. Inserting...");
            let new_challenge = models::InsertChallenge {
                pool_id: db_pool.id,
                challenge: proof.challenge.to_vec(),
                rewards_earned_coal: None,
                rewards_earned_ore: None,
            };
            let result = app_database.add_new_challenge(new_challenge).await;

            if result.is_err() {
                panic!("Failed to create challenge in database");
            }
        }
    }

    let config = Arc::new(Config {
        password,
        pool_id: db_pool.id,
        stats_enabled: true,
        signup_fee: args.signup_fee,
        commissions_pubkey: commission_pubkey.to_string(),
        commissions_miner_id: commission_miner_id,
        guild_address: guild_env,
    });

    let epoch_hashes = Arc::new(RwLock::new(EpochHashes {
        challenge: proof.challenge,
        best_hash: BestHash {
            solution: None,
            difficulty: 0,
        },
        submissions: HashMap::new(),
    }));

    let wallet_extension = Arc::new(WalletExtension {
        miner_wallet: Arc::new(wallet),
        fee_wallet: Arc::new(fee_wallet),
    });
    let proof_ext = Arc::new(Mutex::new(proof));
    let nonce_ext = Arc::new(Mutex::new(0u64));

    let client_nonce_ranges = Arc::new(RwLock::new(HashMap::new()));

    let shared_state = Arc::new(RwLock::new(AppState {
        sockets: HashMap::new(),
        paused: false,
    }));
    let ready_clients = Arc::new(Mutex::new(HashSet::new()));

    let pongs = Arc::new(RwLock::new(LastPong {
        pongs: HashMap::new(),
    }));

    let claims_queue = Arc::new(ClaimsQueue {
        queue: RwLock::new(HashMap::new()),
    });

    let submission_window = Arc::new(RwLock::new(SubmissionWindow { closed: false }));

    let rpc_client = Arc::new(rpc_client);
    let rpc_client_miner = Arc::new(rpc_client_miner);
    let jito_client = Arc::new(jito_client);

    let last_challenge = Arc::new(Mutex::new([0u8; 32]));

    let app_rpc_client = rpc_client.clone();
    let app_wallet = wallet_extension.clone();
    let app_claims_queue = claims_queue.clone();
    let app_app_database = app_database.clone();
    tokio::spawn(async move {
        claim_system(
            app_claims_queue,
            app_rpc_client,
            app_wallet.miner_wallet.clone(),
            app_app_database,
        )
        .await;
    });

    // Track client pong timings
    let app_pongs = pongs.clone();
    let app_state = shared_state.clone();
    tokio::spawn(async move {
        pong_tracking_system(app_pongs, app_state).await;
    });

    let app_wallet = wallet_extension.clone();
    let app_proof = proof_ext.clone();
    let app_last_challenge = last_challenge.clone();
    // Establish webocket connection for tracking pool proof changes.
    tokio::spawn(async move {
        proof_tracking_system(
            rpc_ws_url,
            app_wallet.miner_wallet.clone(),
            app_proof,
            app_last_challenge,
        )
        .await;
    });

    let (client_message_sender, client_message_receiver) =
        tokio::sync::mpsc::unbounded_channel::<ClientMessage>();

    // Handle client messages
    let app_ready_clients = ready_clients.clone();
    let app_proof = proof_ext.clone();
    let app_epoch_hashes = epoch_hashes.clone();
    let app_client_nonce_ranges = client_nonce_ranges.clone();
    let app_state = shared_state.clone();
    let app_pongs = pongs.clone();
    let app_submission_window = submission_window.clone();
    tokio::spawn(async move {
        client_message_handler_system(
            client_message_receiver,
            app_ready_clients,
            app_proof,
            app_epoch_hashes,
            app_client_nonce_ranges,
            app_state,
            app_pongs,
            app_submission_window,
        )
        .await;
    });

    // Handle ready clients
    let app_shared_state = shared_state.clone();
    let app_proof = proof_ext.clone();
    let app_epoch_hashes = epoch_hashes.clone();
    let app_nonce = nonce_ext.clone();
    let app_client_nonce_ranges = client_nonce_ranges.clone();
    let app_ready_clients = ready_clients.clone();
    let app_submission_window = submission_window.clone();
    tokio::spawn(async move {
        handle_ready_clients_system(
            app_shared_state,
            app_proof,
            app_epoch_hashes,
            app_ready_clients,
            app_nonce,
            app_client_nonce_ranges,
            app_submission_window,
        )
        .await;
    });

    let (mine_success_sender, mine_success_receiver) =
        tokio::sync::mpsc::unbounded_channel::<MessageInternalMineSuccess>();

    let (all_clients_sender, all_clients_receiver) =
        tokio::sync::mpsc::unbounded_channel::<MessageInternalAllClients>();

    let app_proof = proof_ext.clone();
    let app_epoch_hashes = epoch_hashes.clone();
    let app_wallet = wallet_extension.clone();
    let app_nonce = nonce_ext.clone();
    let app_prio_fee = priority_fee.clone();
    let app_jito_tip = jito_tip.clone();
    let app_rpc_client = rpc_client.clone();
    let app_rpc_client_miner = rpc_client_miner.clone();
    let app_jito_client = jito_client.clone();
    let app_config = config.clone();
    let app_app_database = app_database.clone();
    let app_all_clients_sender = all_clients_sender.clone();
    let app_submission_window = submission_window.clone();
    let app_client_nonce_ranges = client_nonce_ranges.clone();
    let app_last_challenge = last_challenge.clone();

    tokio::spawn(async move {
        pool_submission_system(
            app_proof,
            app_epoch_hashes,
            app_wallet,
            app_nonce,
            app_prio_fee,
            app_jito_tip,
            app_rpc_client_miner,
            app_jito_client,
            app_config,
            app_app_database,
            app_all_clients_sender,
            mine_success_sender,
            app_submission_window,
            app_client_nonce_ranges,
            app_last_challenge,
        )
        .await;
    });

    if !disable_reprocess {
        let app_config = config.clone();
        let app_wallet = wallet_extension.clone();
        let app_app_database = app_database.clone();
        let app_app_rr_database = app_rr_database.clone();
        let app_rpc_client = rpc_client.clone();
        let app_jito_client = jito_client.clone();
        tokio::spawn(async move {
            chromium_reprocessing_system(
                app_wallet,
                app_rpc_client,
                app_jito_client,
                app_app_database,
                app_config,
                app_app_rr_database,
            )
            .await;
        });
    }

    let app_shared_state = shared_state.clone();
    let app_app_database = app_database.clone();
    let app_config = config.clone();
    let app_wallet = wallet_extension.clone();
    tokio::spawn(async move {
        let app_database = app_app_database;
        pool_mine_success_system(
            app_shared_state,
            app_database,
            app_config,
            app_wallet,
            mine_success_receiver,
        )
        .await;
    });

    let app_shared_state = shared_state.clone();
    tokio::spawn(async move {
        message_text_all_clients_system(app_shared_state, all_clients_receiver).await;
    });

    let cors = CorsLayer::new()
        .allow_methods([Method::GET])
        .allow_origin(tower_http::cors::Any);

    let client_channel = client_message_sender.clone();
    let app_shared_state = shared_state.clone();
    let app = Router::new()
        .route("/v2/ws", get(ws_handler_v2))
        .route("/v2/ws-web", get(ws_handler_web))
        //.route("/pause", post(post_pause))
        .route("/latest-blockhash", get(get_latest_blockhash))
        .route("/pool/authority/pubkey", get(get_pool_authority_pubkey))
        .route("/pool/fee_payer/pubkey", get(get_pool_fee_payer_pubkey))
        .route("/v2/signup", post(post_signup_v2))
        .route("/signup-fee", get(get_signup_fee))
        .route("/sol-balance", get(get_sol_balance))
        .route("/v2/claim", post(post_claim_v2))
        .route("/v2/claim-all", post(post_claim_all_v2))
        //.route("/stake", post(post_stake))
        //.route("/unstake", post(post_unstake))
        .route("/active-miners", get(get_connected_miners))
        .route("/timestamp", get(get_timestamp))
        .route("/miner/balance", get(get_miner_balance))
        .route("/v2/miner/balance", get(get_miner_balance_v2))
        .route("/miner/guild-stake", get(get_miner_guild_stake))
        //.route("/miner/stake", get(get_miner_stake))
        .route("/stake-multiplier", get(get_stake_multiplier))
        // App RR Database routes
        .route(
            "/last-challenge-submissions",
            get(get_last_challenge_submissions),
        )
        .route("/miner/rewards", get(get_miner_rewards))
        .route("/miner/submissions", get(get_miner_submissions))
        .route("/miner/last-claim", get(get_miner_last_claim))
        .route("/challenges", get(get_challenges))
        .route("/pool", get(routes::get_pool))
        .route("/pool/staked", get(routes::get_pool_staked))
        .route(
            "/pool/chromium/reprocess-info",
            get(routes::get_chromium_reprocess_info),
        )
        .route(
            "/pool/stakes-multipliers",
            get(get_pool_stakes_and_multipliers),
        )
        .route("/txns/latest-mine", get(get_latest_mine_txn))
        .route("/guild/addresses", get(get_guild_addresses))
        .route("/guild/check-member", get(get_guild_check_member))
        .route("/guild/stake", post(post_guild_stake))
        .route("/guild/unstake", post(post_guild_un_stake))
        .route(
            "/guild/new-member-instruction",
            get(get_guild_new_member_instruction),
        )
        .route(
            "/guild/delegate-instruction",
            get(get_guild_delegate_instruction),
        )
        .route("/guild/stake-instruction", get(get_guild_stake_instruction))
        .route(
            "/guild/unstake-instruction",
            get(get_guild_unstake_instruction),
        )
        .route(
            "/guild/lp-staking-rewards",
            get(get_guild_lp_staking_rewards),
        )
        .route(
            "/guild/lp-staking-rewards-24h",
            get(get_guild_lp_staking_rewards_24h),
        )
        .route("/coal/stake", post(post_coal_stake))
        .with_state(app_shared_state)
        .layer(Extension(app_database))
        .layer(Extension(app_rr_database))
        .layer(Extension(config))
        .layer(Extension(wallet_extension))
        .layer(Extension(client_channel))
        .layer(Extension(rpc_client))
        .layer(Extension(client_nonce_ranges))
        .layer(Extension(claims_queue))
        .layer(Extension(submission_window))
        // Logging
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        )
        .layer(cors);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    tracing::info!(target: "server_log", "listening on {}", listener.local_addr().unwrap());

    let app_shared_state = shared_state.clone();
    tokio::spawn(async move {
        ping_check_system(&app_shared_state).await;
    });

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();

    Ok(())
}

async fn get_pool_authority_pubkey(
    Extension(wallet): Extension<Arc<WalletExtension>>,
) -> impl IntoResponse {
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/text")
        .body(wallet.miner_wallet.pubkey().to_string())
        .unwrap()
}

async fn get_pool_fee_payer_pubkey(
    Extension(wallet): Extension<Arc<WalletExtension>>,
) -> impl IntoResponse {
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/text")
        .body(wallet.fee_wallet.pubkey().to_string())
        .unwrap()
}

async fn get_latest_blockhash(
    Extension(rpc_client): Extension<Arc<RpcClient>>,
) -> impl IntoResponse {
    let latest_blockhash = rpc_client.get_latest_blockhash().await.unwrap();

    let serialized_blockhash = bincode::serialize(&latest_blockhash).unwrap();

    let encoded_blockhash = BASE64_STANDARD.encode(serialized_blockhash);
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/text")
        .body(encoded_blockhash)
        .unwrap()
}

#[derive(Deserialize)]
struct PauseParams {
    p: String,
}

async fn post_pause(
    query_params: Query<PauseParams>,
    Extension(app_config): Extension<Arc<Config>>,
    State(app_state): State<Arc<RwLock<AppState>>>,
) -> impl IntoResponse {
    if query_params.p.eq(app_config.password.as_str()) {
        let mut writer = app_state.write().await;
        writer.paused = true;
        drop(writer);
        return Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/text")
            .body("SUCCESS".to_string())
            .unwrap();
    }

    return Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .header("Content-Type", "text/text")
        .body("Unauthorized".to_string())
        .unwrap();
}

#[derive(Deserialize)]
struct SignupParams {
    pubkey: String,
}

#[derive(Deserialize)]
struct SignupParamsV2 {
    miner: String,
}

async fn post_signup_v2(
    query_params: Query<SignupParamsV2>,
    Extension(app_database): Extension<Arc<AppDatabase>>,
    Extension(wallet): Extension<Arc<WalletExtension>>,
    _body: String,
) -> impl IntoResponse {
    if let Ok(miner_pubkey) = Pubkey::from_str(&query_params.miner) {
        let db_miner = app_database
            .get_miner_by_pubkey_str(miner_pubkey.to_string())
            .await;

        match db_miner {
            Ok(miner) => {
                if miner.enabled {
                    info!(target: "server_log", "Miner account already enabled!");
                    return Response::builder()
                        .status(StatusCode::OK)
                        .header("Content-Type", "text/text")
                        .body("EXISTS".to_string())
                        .unwrap();
                }
            }
            Err(AppDatabaseError::FailedToGetConnectionFromPool) => {
                error!(target: "server_log", "Failed to get database pool connection");
                return Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body("Failed to get db pool connection".to_string())
                    .unwrap();
            }
            Err(_) => {
                info!(target: "server_log", "No miner account exists. Signing up new user.");
            }
        }

        let res = app_database
            .signup_user_transaction(
                miner_pubkey.to_string(),
                wallet.miner_wallet.pubkey().to_string(),
            )
            .await;

        match res {
            Ok(_) => {
                return Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", "text/text")
                    .body("SUCCESS".to_string())
                    .unwrap();
            }
            Err(_) => {
                error!(target: "server_log", "Failed to add miner to database");
                return Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body("Failed to add user to database".to_string())
                    .unwrap();
            }
        }
    } else {
        error!(target: "server_log", "Signup with invalid miner_pubkey");
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body("Invalid miner pubkey".to_string())
            .unwrap();
    }
}

async fn get_signup_fee(Extension(app_config): Extension<Arc<Config>>) -> impl IntoResponse {
    return Response::builder()
        .status(StatusCode::OK)
        .body(app_config.signup_fee.to_string())
        .unwrap();
}

async fn get_sol_balance(
    Extension(rpc_client): Extension<Arc<RpcClient>>,
    query_params: Query<PubkeyParam>,
) -> impl IntoResponse {
    if let Ok(user_pubkey) = Pubkey::from_str(&query_params.pubkey) {
        let res = rpc_client.get_balance(&user_pubkey).await;

        match res {
            Ok(balance) => {
                let response = format!("{}", lamports_to_sol(balance));
                return Response::builder()
                    .status(StatusCode::OK)
                    .body(response)
                    .unwrap();
            }
            Err(_) => {
                error!(target: "server_log", "get_sol_balance: failed to get sol balance for {}", user_pubkey.to_string());
                return Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body("Failed to get sol balance".to_string())
                    .unwrap();
            }
        }
    } else {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body("Invalid public key".to_string())
            .unwrap();
    }
}

#[derive(Deserialize)]
struct PubkeyParam {
    pubkey: String,
}

#[derive(Deserialize, Serialize)]
struct MinerRewards {
    coal: f64,
    ore: f64,
    chromium: f64,
}

async fn get_miner_rewards(
    query_params: Query<PubkeyParam>,
    Extension(app_rr_database): Extension<Arc<AppRRDatabase>>,
) -> impl IntoResponse {
    if let Ok(user_pubkey) = Pubkey::from_str(&query_params.pubkey) {
        let res = app_rr_database
            .get_miner_rewards(user_pubkey.to_string())
            .await;

        match res {
            Ok(rewards) => {
                let decimal_bal_coal = rewards.balance_coal as f64
                    / 10f64.powf(coal_api::consts::TOKEN_DECIMALS as f64);
                let decimal_bal_ore =
                    rewards.balance_ore as f64 / 10f64.powf(ore_api::consts::TOKEN_DECIMALS as f64);
                let decimal_bal_chromium = rewards.balance_chromium as f64
                    / 10f64.powf(coal_api::consts::TOKEN_DECIMALS as f64);
                let response = MinerRewards {
                    ore: decimal_bal_ore,
                    coal: decimal_bal_coal,
                    chromium: decimal_bal_chromium,
                };
                return Ok(Json(response));
            }
            Err(_) => {
                error!(target: "server_log", "get_miner_rewards: failed to get rewards balance from db for {}", user_pubkey.to_string());
                return Err("Failed to get balance".to_string());
            }
        }
    } else {
        return Err("Invalid public key".to_string());
    }
}

async fn get_last_challenge_submissions(
    Extension(app_rr_database): Extension<Arc<AppRRDatabase>>,
    Extension(app_config): Extension<Arc<Config>>,
) -> Result<Json<Vec<SubmissionWithPubkey>>, String> {
    if app_config.stats_enabled {
        let res = app_rr_database.get_last_challenge_submissions().await;

        match res {
            Ok(submissions) => Ok(Json(submissions)),
            Err(_) => Err("Failed to get submissions for miner".to_string()),
        }
    } else {
        return Err("Stats not enabled for this server.".to_string());
    }
}

#[derive(Deserialize)]
struct GetSubmissionsParams {
    pubkey: String,
}

async fn get_miner_submissions(
    query_params: Query<GetSubmissionsParams>,
    Extension(app_rr_database): Extension<Arc<AppRRDatabase>>,
    Extension(app_config): Extension<Arc<Config>>,
) -> Result<Json<Vec<Submission>>, String> {
    if app_config.stats_enabled {
        if let Ok(user_pubkey) = Pubkey::from_str(&query_params.pubkey) {
            let res = app_rr_database
                .get_miner_submissions(user_pubkey.to_string())
                .await;

            match res {
                Ok(submissions) => Ok(Json(submissions)),
                Err(_) => Err("Failed to get submissions for miner".to_string()),
            }
        } else {
            Err("Invalid public key".to_string())
        }
    } else {
        return Err("Stats not enabled for this server.".to_string());
    }
}

#[derive(Deserialize)]
struct GetLastClaimParams {
    pubkey: String,
}

async fn get_miner_last_claim(
    query_params: Query<GetLastClaimParams>,
    Extension(app_rr_database): Extension<Arc<AppRRDatabase>>,
    Extension(app_config): Extension<Arc<Config>>,
) -> Result<Json<LastClaim>, String> {
    if app_config.stats_enabled {
        if let Ok(user_pubkey) = Pubkey::from_str(&query_params.pubkey) {
            let res = app_rr_database
                .get_last_claim_by_pubkey(user_pubkey.to_string())
                .await;

            match res {
                Ok(last_claim) => Ok(Json(last_claim)),
                Err(_) => Err("Failed to get last claim for miner".to_string()),
            }
        } else {
            Err("Invalid public key".to_string())
        }
    } else {
        return Err("Stats not enabled for this server.".to_string());
    }
}

#[derive(Deserialize, Serialize)]
struct MinerBalance {
    coal: f64,
    ore: f64,
    chromium: f64,
}

async fn get_miner_balance(
    query_params: Query<PubkeyParam>,
    Extension(rpc_client): Extension<Arc<RpcClient>>,
) -> impl IntoResponse {
    if let Ok(user_pubkey) = Pubkey::from_str(&query_params.pubkey) {
        let miner_token_account_coal = get_associated_token_address(&user_pubkey, &get_coal_mint());
        let miner_token_account_ore = get_associated_token_address(&user_pubkey, &get_ore_mint());
        let miner_token_account_chromium =
            get_associated_token_address(&user_pubkey, &get_chromium_mint());

        let mut resp = MinerBalance {
            ore: 0.0,
            coal: 0.0,
            chromium: 0.0,
        };

        if let Ok(response_coal) = rpc_client
            .get_token_account_balance(&miner_token_account_coal)
            .await
        {
            resp.coal = response_coal.ui_amount.unwrap();
        }

        if let Ok(response_ore) = rpc_client
            .get_token_account_balance(&miner_token_account_ore)
            .await
        {
            resp.ore = response_ore.ui_amount.unwrap();
        }

        if let Ok(response_chromium) = rpc_client
            .get_token_account_balance(&miner_token_account_chromium)
            .await
        {
            resp.chromium = response_chromium.ui_amount.unwrap();
        }

        return Ok(Json(resp));
    } else {
        return Err("Invalid public key".to_string());
    }
}

/*async fn get_miner_stake(
    query_params: Query<PubkeyParam>,
    Extension(rpc_client): Extension<Arc<RpcClient>>,
    Extension(wallet): Extension<Arc<WalletExtension>>,
) -> impl IntoResponse {
    if let Ok(user_pubkey) = Pubkey::from_str(&query_params.pubkey) {
        if let Ok(account) =
            get_delegated_stake_account(&rpc_client, user_pubkey, wallet.miner_wallet.pubkey())
                .await
        {
            let decimals = 10f64.powf(COAL_TOKEN_DECIMALS as f64);
            let dec_amount = (account.amount as f64).div(decimals);
            return Ok(dec_amount.to_string());
        } else {
            return Err("Failed to get token account balance".to_string());
        }
    } else {
        return Err("Invalid pubkey".to_string());
    }
}*/

async fn get_stake_multiplier(
    Extension(rpc_client): Extension<Arc<RpcClient>>,
    Extension(app_config): Extension<Arc<Config>>,
) -> impl IntoResponse {
    if app_config.stats_enabled {
        let pubkey = Pubkey::from_str("6zbGwDbfwVS3hF8r7Yei8HuwSWm2yb541jUtmAZKhFDM").unwrap();
        let proof = if let Ok(loaded_proof) = get_proof(&rpc_client, pubkey).await {
            loaded_proof
        } else {
            error!(target: "server_log", "get_pool_staked: Failed to load proof.");
            return Err("Stats not enabled for this server.".to_string());
        };

        if let Ok(config) = get_config(&rpc_client).await {
            let multiplier = 1.0 + (proof.balance as f64 / config.top_balance as f64).min(1.0f64);
            return Ok(Json(multiplier));
        } else {
            return Err("Failed to get coal config account".to_string());
        }
    } else {
        return Err("Stats not enabled for this server.".to_string());
    }
}

#[derive(Deserialize)]
struct ConnectedMinersParams {
    pubkey: Option<String>,
}

async fn get_connected_miners(
    query_params: Query<ConnectedMinersParams>,
    State(app_state): State<Arc<RwLock<AppState>>>,
) -> impl IntoResponse {
    let reader = app_state.read().await;
    let socks = reader.sockets.clone();
    drop(reader);

    if let Some(pubkey_str) = &query_params.pubkey {
        if let Ok(user_pubkey) = Pubkey::from_str(&pubkey_str) {
            let mut connection_count = 0;

            for (_addr, client_connection) in socks.iter() {
                if user_pubkey.eq(&client_connection.pubkey) {
                    connection_count += 1;
                }
            }

            return Response::builder()
                .status(StatusCode::OK)
                .body(connection_count.to_string())
                .unwrap();
        } else {
            error!(target: "server_log", "Get connected miners with invalid pubkey");
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body("Invalid Pubkey".to_string())
                .unwrap();
        }
    } else {
        return Response::builder()
            .status(StatusCode::OK)
            .body(socks.len().to_string())
            .unwrap();
    }
}

async fn get_timestamp() -> impl IntoResponse {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs();
    return Response::builder()
        .status(StatusCode::OK)
        .body(now.to_string())
        .unwrap();
}

#[derive(Deserialize)]
struct ClaimParams {
    pubkey: String,
    amount: u64,
}

#[derive(Deserialize)]
struct ClaimParamsV2 {
    timestamp: u64,
    receiver_pubkey: String,
    amount_coal: u64,
    amount_ore: u64,
    amount_chromium: u64,
}

async fn post_claim_v2(
    TypedHeader(auth_header): TypedHeader<axum_extra::headers::Authorization<Basic>>,
    Extension(app_database): Extension<Arc<AppDatabase>>,
    Extension(claims_queue): Extension<Arc<ClaimsQueue>>,
    query_params: Query<ClaimParamsV2>,
) -> impl IntoResponse {
    let msg_timestamp = query_params.timestamp;

    let miner_pubkey_str = auth_header.username();
    let signed_msg = auth_header.password();

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs();

    // Signed authentication message is only valid for 30 seconds
    if (now - msg_timestamp) >= 30 {
        return Err((StatusCode::UNAUTHORIZED, "Timestamp too old.".to_string()));
    }
    let receiver_pubkey = match Pubkey::from_str(&query_params.receiver_pubkey) {
        Ok(pubkey) => pubkey,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                "Invalid receiver_pubkey provided.".to_string(),
            ))
        }
    };

    if let Ok(miner_pubkey) = Pubkey::from_str(miner_pubkey_str) {
        if let Ok(signature) = Signature::from_str(signed_msg) {
            let amount_coal = query_params.amount_coal;
            let amount_ore = query_params.amount_ore;
            let amount_chromium = query_params.amount_chromium;
            let mut signed_msg = vec![];
            signed_msg.extend(msg_timestamp.to_le_bytes());
            signed_msg.extend(receiver_pubkey.to_bytes());
            signed_msg.extend(amount_coal.to_le_bytes());
            signed_msg.extend(amount_ore.to_le_bytes());
            signed_msg.extend(amount_chromium.to_le_bytes());

            if signature.verify(&miner_pubkey.to_bytes(), &signed_msg) {
                let reader = claims_queue.queue.read().await;
                let queue = reader.clone();
                drop(reader);

                if queue.contains_key(&miner_pubkey) {
                    return Err((StatusCode::TOO_MANY_REQUESTS, "QUEUED".to_string()));
                }

                let amount_coal = query_params.amount_coal;

                // 5 COAL 0.05 ORE
                if amount_coal < 500_000_000_000 && amount_ore < 5_000_000_000 {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        "claim minimum is 5 COAL or 0.05 ORE".to_string(),
                    ));
                }

                if let Ok(miner_rewards) = app_database
                    .get_miner_rewards(miner_pubkey.to_string())
                    .await
                {
                    if amount_coal > miner_rewards.balance_coal {
                        return Err((
                            StatusCode::BAD_REQUEST,
                            "claim amount for COAL exceeds miner rewards balance.".to_string(),
                        ));
                    }
                    if amount_ore > miner_rewards.balance_ore {
                        return Err((
                            StatusCode::BAD_REQUEST,
                            "claim amount for ORE exceeds miner rewards balance.".to_string(),
                        ));
                    }

                    if let Ok(last_claim) =
                        app_database.get_last_claim(miner_rewards.miner_id).await
                    {
                        let last_claim_ts = last_claim.created_at.and_utc().timestamp();
                        let now = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .expect("Time went backwards")
                            .as_secs() as i64;
                        let time_difference = now - last_claim_ts;
                        if time_difference <= 1800 {
                            return Err((
                                StatusCode::TOO_MANY_REQUESTS,
                                time_difference.to_string(),
                            ));
                        }
                    }

                    let mut writer = claims_queue.queue.write().await;
                    writer.insert(
                        miner_pubkey,
                        ClaimsQueueItem {
                            receiver_pubkey,
                            amount_coal,
                            amount_ore,
                            amount_chromium,
                        },
                    );
                    drop(writer);
                    return Ok((StatusCode::OK, "SUCCESS"));
                } else {
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "failed to get miner account from database".to_string(),
                    ));
                }
            } else {
                return Err((
                    StatusCode::UNAUTHORIZED,
                    "Sig verification failed".to_string(),
                ));
            }
        } else {
            return Err((StatusCode::UNAUTHORIZED, "Invalid signature".to_string()));
        }
    } else {
        error!(target: "server_log", "Claim with invalid pubkey");
        return Err((StatusCode::BAD_REQUEST, "Invalid Pubkey".to_string()));
    }
}

#[derive(Deserialize)]
struct ClaimAllParamsV2 {
    timestamp: u64,
    receiver_pubkey: String,
    amount_coal: u64,
    amount_ore: u64,
    amount_chromium: u64,
    username: String,
    password: String,
}

async fn post_claim_all_v2(
    // TypedHeader(auth_header): TypedHeader<axum_extra::headers::Authorization<Basic>>,
    Extension(app_database): Extension<Arc<AppDatabase>>,
    Extension(claims_queue): Extension<Arc<ClaimsQueue>>,
    query_params: Query<ClaimAllParamsV2>,
) -> impl IntoResponse {
    let msg_timestamp = query_params.timestamp;

    let miner_pubkey_str = query_params.username.to_string();
    let signed_msg = query_params.password.to_string();

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs();

    // Signed authentication message is only valid for 30 seconds
    if (now - msg_timestamp) >= 30 {
        return Err((StatusCode::UNAUTHORIZED, "Timestamp too old.".to_string()));
    }

    if let Ok(miner_pubkey) = Pubkey::from_str(&miner_pubkey_str) {
        // if let Ok(_) = Signature::from_str(&signed_msg) {
        let reader = claims_queue.queue.read().await;
        let queue = reader.clone();
        drop(reader);

        if queue.contains_key(&miner_pubkey) {
            return Err((StatusCode::TOO_MANY_REQUESTS, "QUEUED".to_string()));
        }

        let amount_coal = query_params.amount_coal;
        let amount_ore = query_params.amount_ore;
        let amount_chromium = query_params.amount_chromium;

        // 5 COAL 0.05 ORE
        if amount_coal < 500_000_000_000 && amount_ore < 5_000_000_000 {
            return Err((
                StatusCode::BAD_REQUEST,
                "claim minimum is 5 COAL or 0.05 ORE".to_string(),
            ));
        }

        if let Ok(miner_rewards) = app_database
            .get_miner_rewards(miner_pubkey.to_string())
            .await
        {
            if amount_coal > miner_rewards.balance_coal {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "claim amount for COAL exceeds miner rewards balance.".to_string(),
                ));
            }
            if amount_ore > miner_rewards.balance_ore {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "claim amount for ORE exceeds miner rewards balance.".to_string(),
                ));
            }

            if let Ok(last_claim) = app_database.get_last_claim(miner_rewards.miner_id).await {
                let last_claim_ts = last_claim.created_at.and_utc().timestamp();
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("Time went backwards")
                    .as_secs() as i64;
                let time_difference = now - last_claim_ts;
                if time_difference <= 1800 {
                    return Err((StatusCode::TOO_MANY_REQUESTS, time_difference.to_string()));
                }
            }

            let mut writer = claims_queue.queue.write().await;
            writer.insert(
                miner_pubkey,
                ClaimsQueueItem {
                    receiver_pubkey: miner_pubkey,
                    amount_coal,
                    amount_ore,
                    amount_chromium,
                },
            );
            drop(writer);
            return Ok((StatusCode::OK, "SUCCESS"));
        } else {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to get miner account from database".to_string(),
            ));
        }
        /*} else {
            error!(target: "server_log", "Invalid signature");
            return Err((
                StatusCode::UNAUTHORIZED,
                "Sig verification failed".to_string(),
            ));
        }*/
    } else {
        error!(target: "server_log", "Claim with invalid pubkey");
        return Err((StatusCode::BAD_REQUEST, "Invalid Pubkey".to_string()));
    }
}

#[derive(Deserialize)]
struct GuildStakeParams {
    pubkey: String,
    amount: u64,
    mint: String,
}

async fn post_guild_stake(
    query_params: Query<GuildStakeParams>,
    Extension(rpc_client): Extension<Arc<RpcClient>>,
    Extension(wallet): Extension<Arc<WalletExtension>>,
    Extension(app_config): Extension<Arc<Config>>,
    body: String,
) -> impl IntoResponse {
    const MAX_RETRIES: u32 = 5; // Maximum number of retry attempts
    const BASE_DELAY: u64 = 500; // Base delay in milliseconds

    if let (Ok(user_pubkey), Ok(guild_pubkey)) = (
        Pubkey::from_str(&query_params.pubkey),
        Pubkey::from_str(&app_config.guild_address),
    ) {
        let serialized_tx = match BASE64_STANDARD.decode(&body) {
            Ok(tx) => tx,
            Err(e) => {
                error!(target: "server_log", "Failed to decode transaction: {}", e);
                return Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body("Invalid Tx encoding".to_string())
                    .unwrap();
            }
        };

        let mut tx: Transaction = match bincode::deserialize(&serialized_tx) {
            Ok(tx) => tx,
            Err(_) => {
                error!(target: "server_log", "Failed to deserialize tx");
                return Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body("Invalid Tx format".to_string())
                    .unwrap();
            }
        };

        // Verify fee payer and ensure transaction structure
        if !tx.message.account_keys[0].eq(&wallet.fee_wallet.pubkey()) {
            error!(target: "server_log", "Guild stake: Unexpected fee payer detected in transaction. {:?}",tx.message);
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body("Invalid fee payer".to_string())
                .unwrap();
        }

        for instruction in tx.message.instructions.clone() {
            if tx.message.account_keys[instruction.program_id_index as usize] != coal_guilds_api::ID
                && validate_compute_unit_instruction(&instruction, &tx.message).is_err()
            {
                error!(target: "server_log", "Guild stake: Wrong program detected in transaction. {:?}",tx.message);
                return Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body("Instructions error".to_string())
                    .unwrap();
            }
        }

        // Sign the transaction
        tx.sign(&[&*wallet.fee_wallet], tx.message.recent_blockhash);

        // Retry mechanism for sending the transaction
        //for attempt in 0..=MAX_RETRIES {
        match rpc_client
            .send_and_confirm_transaction_with_spinner_and_commitment(
                &tx,
                CommitmentConfig::confirmed(),
            )
            .await
        {
            Ok(signature) => {
                // Transaction successful
                let amount_dec =
                    query_params.amount as f64 / 10f64.powf(COAL_TOKEN_DECIMALS as f64);
                info!(target: "server_log", "Miner {} successfully staked to the guild {}.\nSig: {:?}", user_pubkey, amount_dec, signature);
                return Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", "text/text")
                    .body("SUCCESS".to_string())
                    .unwrap();
            }
            Err(e) => {
                error!(target: "server_log", "Failed to send transaction. Error: {:?}", e);
                return Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(format!("Transaction failed with error: {}", e))
                    .unwrap();
            }
        }
        //}
    } else {
        error!(target: "server_log", "Invalid pubkey in stake request");
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body("Invalid pubkey".to_string())
            .unwrap();
    }
}

fn validate_compute_unit_instruction(
    ix: &CompiledInstruction,
    message: &solana_sdk::message::Message,
) -> Result<(), ProgramError> {
    if message.account_keys[ix.program_id_index as usize]
        != Pubkey::from_str("ComputeBudget111111111111111111111111111111").unwrap()
        && message.account_keys[ix.program_id_index as usize]
            != Pubkey::from_str("L2TExMFKdjpN9kozasaurPirfHy9P8sbXoAN1qA3S95").unwrap()
    {
        return Err(ProgramError::InvalidInstructionData);
    }

    // Additional validation can be added if necessary
    Ok(())
}

fn validate_token_transfer_instruction(
    ix: &CompiledInstruction,
    message: &solana_sdk::message::Message,
    user_token_account: &Pubkey,
    program_token_account: &Pubkey,
) -> Result<(), ProgramError> {
    let program_id_from_ix = get_program_id(ix, message)?;
    if program_id_from_ix != spl_token::id() {
        return Err(ProgramError::InvalidInstructionData);
    }

    // Parse the instruction data to confirm it's a Transfer
    let token_instruction =
        TokenInstruction::unpack(&ix.data).map_err(|_| ProgramError::InvalidInstructionData)?;

    match token_instruction {
        TokenInstruction::Transfer { amount: _ } => {
            let source_account_index = ix
                .accounts
                .get(0)
                .ok_or(ProgramError::InvalidInstructionData)?;
            let destination_account_index = ix
                .accounts
                .get(1)
                .ok_or(ProgramError::InvalidInstructionData)?;

            let source_account = &message.account_keys[*source_account_index as usize];
            let destination_account = &message.account_keys[*destination_account_index as usize];

            // Ensure the source is the user's token account and the destination is the program's token account
            if source_account != user_token_account || destination_account != program_token_account
            {
                return Err(ProgramError::InvalidInstructionData);
            }
        }
        _ => return Err(ProgramError::InvalidInstructionData),
    }

    Ok(())
}

fn get_program_id(
    ix: &CompiledInstruction,
    message: &solana_sdk::message::Message,
) -> Result<Pubkey, ProgramError> {
    message
        .account_keys
        .get(ix.program_id_index as usize)
        .cloned()
        .ok_or(ProgramError::InvalidInstructionData)
}

#[derive(Deserialize)]
struct CoalStakeParams {
    pubkey: String,
    amount: u64,
}
async fn post_coal_stake(
    query_params: Query<CoalStakeParams>,
    Extension(rpc_client): Extension<Arc<RpcClient>>,
    Extension(wallet): Extension<Arc<WalletExtension>>,
    Extension(app_config): Extension<Arc<Config>>,
    Extension(app_database): Extension<Arc<AppDatabase>>,
    body: String,
) -> impl IntoResponse {
    const MAX_RETRIES: u32 = 5; // Maximum number of retry attempts
    const BASE_DELAY: u64 = 500; // Base delay in milliseconds

    if let (Ok(user_pubkey), Ok(guild_pubkey)) = (
        Pubkey::from_str(&query_params.pubkey),
        Pubkey::from_str(&app_config.guild_address),
    ) {
        let serialized_tx = match BASE64_STANDARD.decode(&body) {
            Ok(tx) => tx,
            Err(e) => {
                error!(target: "server_log", "Failed to decode transaction: {}", e);
                return Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body("Invalid Tx encoding".to_string())
                    .unwrap();
            }
        };

        let mut tx: Transaction = match bincode::deserialize(&serialized_tx) {
            Ok(tx) => tx,
            Err(_) => {
                error!(target: "server_log", "Failed to deserialize tx");
                return Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body("Invalid Tx format".to_string())
                    .unwrap();
            }
        };

        // Verify fee payer and ensure transaction structure
        if !tx.message.account_keys[0].eq(&wallet.fee_wallet.pubkey()) {
            error!(target: "server_log", "Coal stake: Unexpected fee payer detected in transaction.");
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body("Invalid fee payer".to_string())
                .unwrap();
        }

        // check if the only transactions are new_member, stake, and delegate. None is mandatory
        if tx.message.instructions.len() == 0 {
            error!(target: "server_log", "Coal stake: No instructions detected in transaction.");
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body("Instructions error".to_string())
                .unwrap();
        }

        let pool_token_account_coal =
            get_associated_token_address(&wallet.miner_wallet.pubkey(), &get_coal_mint());

        let user_token_account_coal = get_associated_token_address(&user_pubkey, &get_coal_mint());

        info!(target: "server_log", "tx.message {:?}.", tx.message);
        info!(target: "server_log", "tx.message.instructions {:?}.", tx.message.instructions);

        for instruction in tx.message.instructions.clone() {
            if tx.message.account_keys[instruction.program_id_index as usize] != coal_guilds_api::ID
                && validate_compute_unit_instruction(&instruction, &tx.message).is_err()
                && validate_token_transfer_instruction(
                    &instruction,
                    &tx.message,
                    &user_token_account_coal,
                    &pool_token_account_coal,
                )
                .is_err()
            {
                error!(target: "server_log", "Pool stake: Wrong program detected in transaction. {:?}",tx.message);
                return Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body("Instructions error".to_string())
                    .unwrap();
            }
        }

        // Sign the transaction
        tx.sign(&[&*wallet.fee_wallet], tx.message.recent_blockhash);

        // Retry mechanism for sending the transaction
        //for attempt in 0..=MAX_RETRIES {
        match rpc_client
            .send_and_confirm_transaction_with_spinner_and_commitment(
                &tx,
                CommitmentConfig::confirmed(),
            )
            .await
        {
            Ok(signature) => {
                tokio::time::sleep(Duration::from_millis(5000)).await;
                let pool_sender =
                    get_associated_token_address(&wallet.miner_wallet.pubkey(), &COAL_MINT_ADDRESS);

                let stake_ix = coal_utils::get_stake_ix(
                    wallet.miner_wallet.pubkey(),
                    pool_sender,
                    query_params.amount,
                );

                match send_and_confirm(
                    &[stake_ix],
                    ComputeBudget::Fixed(32_000),
                    &rpc_client.clone(),
                    &rpc_client.clone(),
                    &wallet.miner_wallet.clone(),
                    &wallet.fee_wallet.clone(),
                    None,
                    None,
                )
                .await
                {
                    Ok(signature) => {
                        tracing::info!(target: "server_log", "Coal stake: Transaction successful {}",signature);
                    }
                    Err(e) => {
                        tracing::error!(target: "server_log", "Coal stake: Transaction failed: {}", e);
                        return Response::builder()
                            .status(StatusCode::BAD_REQUEST)
                            .body("Transaction failed with error".to_string())
                            .unwrap();
                    }
                }

                // Transaction successful
                let amount_dec =
                    query_params.amount as f64 / 10f64.powf(COAL_TOKEN_DECIMALS as f64);

                let mut miner_id = 1;

                match app_database
                    .get_miner_by_pubkey_str(query_params.pubkey.clone())
                    .await
                {
                    Ok(miner) => miner_id = miner.id,
                    Err(_) => {
                        tracing::error!(target: "server_log", "Coal stake: Failed to get miner... retrying...");
                        tokio::time::sleep(Duration::from_millis(500)).await;
                    }
                }

                // Insert commissions earning
                let user_earning = vec![InsertEarning {
                    miner_id: miner_id,
                    pool_id: app_config.pool_id,
                    challenge_id: -1,
                    amount_coal: query_params.amount,
                    amount_ore: 0,
                }];
                tracing::info!(target: "server_log", "Coal stake: Inserting earning");
                while let Err(_) = app_database
                    .add_new_earnings_batch(user_earning.clone())
                    .await
                {
                    tracing::error!(target: "server_log", "Coal stake: Failed to add earning... retrying...");
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                tracing::info!(target: "server_log", "Coal stake: Inserted earning");

                let new_rewards = vec![UpdateReward {
                    miner_id: miner_id,
                    balance_coal: query_params.amount,
                    balance_ore: 0,
                    balance_chromium: 0,
                    balance_ingot: 0,
                    balance_sol: 0,
                    balance_wood: 0,
                }];

                tracing::info!(target: "server_log", "Coal stake: Updating rewards...");
                while let Err(_) = app_database.update_rewards(new_rewards.clone()).await {
                    tracing::error!(target: "server_log", "Coal stake: Failed to update rewards in db. Retrying...");
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }

                info!(target: "server_log", "Miner {} successfully staked {}.\nSig: {:?}", user_pubkey, amount_dec, signature);
                return Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", "text/text")
                    .body("SUCCESS".to_string())
                    .unwrap();
            }
            Err(e) => {
                error!(target: "server_log", "Failed to send transaction. Error: {:?}", e);
                return Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(format!("Transaction failed with error: {}", e))
                    .unwrap();
            }
        }
        //}
    } else {
        error!(target: "server_log", "Invalid pubkey in stake request");
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body("Invalid pubkey".to_string())
            .unwrap();
    }
}

#[derive(Deserialize)]
struct GuildUnStakeParams {
    pubkey: String,
    amount: u64,
    mint: String,
}

async fn post_guild_un_stake(
    query_params: Query<GuildUnStakeParams>,
    Extension(rpc_client): Extension<Arc<RpcClient>>,
    Extension(wallet): Extension<Arc<WalletExtension>>,
    Extension(app_config): Extension<Arc<Config>>,
    body: String,
) -> impl IntoResponse {
    const MAX_RETRIES: u32 = 5; // Maximum number of retry attempts
    const BASE_DELAY: u64 = 500; // Base delay in milliseconds

    if let (Ok(user_pubkey), Ok(guild_pubkey)) = (
        Pubkey::from_str(&query_params.pubkey),
        Pubkey::from_str(&app_config.guild_address),
    ) {
        let serialized_tx = match BASE64_STANDARD.decode(&body) {
            Ok(tx) => tx,
            Err(e) => {
                error!(target: "server_log", "Failed to decode transaction: {}", e);
                return Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body("Invalid Tx encoding".to_string())
                    .unwrap();
            }
        };

        let mut tx: Transaction = match bincode::deserialize(&serialized_tx) {
            Ok(tx) => tx,
            Err(_) => {
                error!(target: "server_log", "Failed to deserialize tx");
                return Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body("Invalid Tx format".to_string())
                    .unwrap();
            }
        };

        // Verify fee payer and ensure transaction structure
        if !tx.message.account_keys[0].eq(&wallet.fee_wallet.pubkey()) {
            error!(target: "server_log", "Guild unstake: Unexpected fee payer detected in transaction.");
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body("Invalid fee payer".to_string())
                .unwrap();
        }

        // check if the only transactions are new_member, unstake, and delegate. None is mandatory
        if tx.message.instructions.len() == 0 {
            error!(target: "server_log", "Guild unstake: No instructions detected in transaction.");
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body("Instructions error".to_string())
                .unwrap();
        }

        for instruction in tx.message.instructions.clone() {
            if tx.message.account_keys[instruction.program_id_index as usize] != coal_guilds_api::ID
                && validate_compute_unit_instruction(&instruction, &tx.message).is_err()
            {
                error!(target: "server_log", "Guild unstake: Wrong program detected in transaction. {:?}",tx.message);
                return Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body("Instructions error".to_string())
                    .unwrap();
            }
        }

        // Sign the transaction
        tx.sign(&[&*wallet.fee_wallet], tx.message.recent_blockhash);

        // Retry mechanism for sending the transaction
        //for attempt in 0..=MAX_RETRIES {
        match rpc_client
            .send_and_confirm_transaction_with_spinner_and_commitment(
                &tx,
                CommitmentConfig::confirmed(),
            )
            .await
        {
            Ok(signature) => {
                // Transaction successful
                let amount_dec =
                    query_params.amount as f64 / 10f64.powf(COAL_TOKEN_DECIMALS as f64);
                info!(target: "server_log", "Miner {} successfully unstaked from the guild {}.\nSig: {:?}", user_pubkey, amount_dec, signature);
                return Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", "text/text")
                    .body("SUCCESS".to_string())
                    .unwrap();
            }
            Err(e) => {
                error!(target: "server_log", "Failed to send transaction. Error: {:?}", e);
                return Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(format!("Transaction failed with error: {}", e))
                    .unwrap();
            }
        }
        //}
    } else {
        error!(target: "server_log", "Invalid pubkey in unstake request");
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body("Invalid pubkey".to_string())
            .unwrap();
    }
}

#[derive(Deserialize)]
struct WsQueryParams {
    timestamp: u64,
}

async fn ws_handler_v2(
    ws: WebSocketUpgrade,
    TypedHeader(auth_header): TypedHeader<axum_extra::headers::Authorization<Basic>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(app_state): State<Arc<RwLock<AppState>>>,
    //Extension(app_config): Extension<Arc<Config>>,
    Extension(client_channel): Extension<UnboundedSender<ClientMessage>>,
    Extension(app_database): Extension<Arc<AppDatabase>>,
    query_params: Query<WsQueryParams>,
) -> impl IntoResponse {
    let msg_timestamp = query_params.timestamp;
    info!(target:"server_log", "New WebSocket connection from: {:?}", addr);

    let pubkey = auth_header.username();
    let signed_msg = auth_header.password();

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs();

    // Signed authentication message is only valid for 30 seconds
    if (now - query_params.timestamp) >= 30 {
        return Err((StatusCode::UNAUTHORIZED, "Timestamp too old."));
    }

    // verify client
    if let Ok(user_pubkey) = Pubkey::from_str(pubkey) {
        let db_miner = app_database
            .get_miner_by_pubkey_str(pubkey.to_string())
            .await;

        let miner;
        match db_miner {
            Ok(db_miner) => {
                miner = db_miner;
            }
            Err(AppDatabaseError::QueryFailed) => {
                return Err((
                    StatusCode::UNAUTHORIZED,
                    "pubkey is not authorized to mine. please sign up.",
                ));
            }
            Err(AppDatabaseError::InteractionFailed) => {
                return Err((
                    StatusCode::UNAUTHORIZED,
                    "pubkey is not authorized to mine. please sign up.",
                ));
            }
            Err(AppDatabaseError::FailedToGetConnectionFromPool) => {
                error!(target: "server_log", "Failed to get database pool connection.");
                return Err((StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error"));
            }
            Err(_) => {
                error!(target: "server_log", "DB Error: Catch all.");
                return Err((StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error"));
            }
        }

        if !miner.enabled {
            return Err((StatusCode::UNAUTHORIZED, "pubkey is not authorized to mine"));
        }

        if let Ok(signature) = Signature::from_str(signed_msg) {
            let ts_msg = msg_timestamp.to_le_bytes();

            if signature.verify(&user_pubkey.to_bytes(), &ts_msg) {
                info!(target: "server_log", "Client: {addr} connected with pubkey {pubkey} on V2.");
                return Ok(ws.on_upgrade(move |socket| {
                    handle_socket(
                        socket,
                        addr,
                        user_pubkey,
                        miner.id,
                        ClientVersion::V2,
                        app_state,
                        client_channel,
                    )
                }));
            } else {
                return Err((StatusCode::UNAUTHORIZED, "Sig verification failed"));
            }
        } else {
            return Err((StatusCode::UNAUTHORIZED, "Invalid signature"));
        }
    } else {
        return Err((StatusCode::UNAUTHORIZED, "Invalid pubkey"));
    }
}

#[derive(Deserialize)]
struct WsWebQueryParams {
    timestamp: u64,
    pubkey: String,
}
async fn ws_handler_web(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(app_state): State<Arc<RwLock<AppState>>>,
    Extension(client_channel): Extension<UnboundedSender<ClientMessage>>,
    Extension(app_database): Extension<Arc<AppDatabase>>,
    query_params: Query<WsWebQueryParams>,
) -> impl IntoResponse {
    let msg_timestamp = query_params.timestamp;
    info!(target:"server_log", "New WebSocket connection from: {:?}", addr);

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs();

    // Signed authentication message is only valid for 30 seconds
    if (now - query_params.timestamp) >= 30 {
        return Err((StatusCode::UNAUTHORIZED, "Timestamp too old."));
    }

    // verify client
    if let Ok(user_pubkey) = Pubkey::from_str(query_params.pubkey.as_str()) {
        let db_miner = app_database
            .get_miner_by_pubkey_str(user_pubkey.to_string())
            .await;

        let miner;
        match db_miner {
            Ok(db_miner) => {
                miner = db_miner;
            }
            Err(AppDatabaseError::QueryFailed) => {
                return Err((
                    StatusCode::UNAUTHORIZED,
                    "pubkey is not authorized to mine. please sign up.",
                ));
            }
            Err(AppDatabaseError::InteractionFailed) => {
                return Err((
                    StatusCode::UNAUTHORIZED,
                    "pubkey is not authorized to mine. please sign up.",
                ));
            }
            Err(AppDatabaseError::FailedToGetConnectionFromPool) => {
                error!(target: "server_log", "Failed to get database pool connection.");
                return Err((StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error"));
            }
            Err(_) => {
                error!(target: "server_log", "DB Error: Catch all.");
                return Err((StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error"));
            }
        }

        if !miner.enabled {
            return Err((StatusCode::UNAUTHORIZED, "pubkey is not authorized to mine"));
        }

        info!(target: "server_log", "Client: {addr} connected with pubkey {user_pubkey} on V2.");
        return Ok(ws.on_upgrade(move |socket| {
            handle_socket(
                socket,
                addr,
                user_pubkey,
                miner.id,
                ClientVersion::V2,
                app_state,
                client_channel,
            )
        }));
    } else {
        return Err((StatusCode::UNAUTHORIZED, "Invalid pubkey"));
    }
}

async fn handle_socket(
    mut socket: WebSocket,
    who: SocketAddr,
    who_pubkey: Pubkey,
    who_miner_id: i32,
    client_version: ClientVersion,
    rw_app_state: Arc<RwLock<AppState>>,
    client_channel: UnboundedSender<ClientMessage>,
) {
    if socket
        .send(axum::extract::ws::Message::Ping(vec![1, 2, 3]))
        .await
        .is_ok()
    {
        tracing::debug!("Pinged {who}...");
    } else {
        error!(target: "server_log", "could not ping {who}");

        // if we can't ping we can't do anything, return to close the connection
        return;
    }

    let (sender, mut receiver) = socket.split();
    let mut app_state = rw_app_state.write().await;
    if app_state.sockets.contains_key(&who) {
        info!(target: "server_log", "Socket addr: {who} already has an active connection");
        return;
    } else {
        info!(target: "server_log", "Client: {} connected!", who_pubkey.to_string());
        let new_app_client_connection = AppClientConnection {
            pubkey: who_pubkey,
            miner_id: who_miner_id,
            client_version,
            socket: Arc::new(Mutex::new(sender)),
        };
        app_state.sockets.insert(who, new_app_client_connection);
    }
    drop(app_state);

    let _ = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if process_message(msg, who, client_channel.clone()).is_break() {
                break;
            }
        }
    })
    .await;

    let mut app_state = rw_app_state.write().await;
    app_state.sockets.remove(&who);
    drop(app_state);

    info!(target: "server_log", "Client: {} disconnected!", who_pubkey.to_string());
}

fn process_message(
    msg: Message,
    who: SocketAddr,
    client_channel: UnboundedSender<ClientMessage>,
) -> ControlFlow<(), ()> {
    // info!(target: "server_log", "Received message from {who}: {msg:?}");
    match msg {
        Message::Text(_t) => {
            //println!(">>> {who} sent str: {t:?}");
        }
        Message::Binary(d) => {
            // first 8 bytes are message type
            let message_type = d[0];
            match message_type {
                0 => {
                    let msg = ClientMessage::Ready(who);
                    let _ = client_channel.send(msg);
                }
                1 => {
                    let msg = ClientMessage::Mining(who);
                    let _ = client_channel.send(msg);
                }
                2 => {
                    // parse solution from message data
                    let mut solution_bytes = [0u8; 16];
                    // extract (16 u8's) from data for hash digest
                    let mut b_index = 1;
                    for i in 0..16 {
                        solution_bytes[i] = d[i + b_index];
                    }
                    b_index += 16;

                    // extract 64 bytes (8 u8's)
                    let mut nonce = [0u8; 8];
                    for i in 0..8 {
                        nonce[i] = d[i + b_index];
                    }
                    b_index += 8;

                    let mut pubkey = [0u8; 32];
                    for i in 0..32 {
                        pubkey[i] = d[i + b_index];
                    }

                    // REMOVED MINING SIGNATURE
                    // b_index += 32;

                    //let signature_bytes = d[b_index..].to_vec();
                    //if let Ok(sig_str) = String::from_utf8(signature_bytes.clone()) {
                    //if let Ok(sig) = Signature::from_str(&sig_str) {
                    let pubkey = Pubkey::new_from_array(pubkey);

                    //let mut hash_nonce_message = [0; 24];
                    //hash_nonce_message[0..16].copy_from_slice(&solution_bytes);
                    //hash_nonce_message[16..24].copy_from_slice(&nonce);

                    //if sig.verify(&pubkey.to_bytes(), &hash_nonce_message) {
                    let solution = Solution::new(solution_bytes, nonce);

                    let msg = ClientMessage::BestSolution(who, solution, pubkey);
                    let _ = client_channel.send(msg);
                    //} else {
                    //    error!(target: "server_log", "Client submission sig verification failed.");
                    //}
                    //} else {
                    //    error!(target: "server_log", "Failed to parse into Signature.");
                    //}
                    //}
                    //else {
                    //    error!(target: "server_log", "Failed to parse signed message from client.");
                    //}
                }
                _ => {
                    error!(target: "server_log", ">>> {} sent an invalid message", who);
                }
            }
        }
        Message::Close(c) => {
            if let Some(cf) = c {
                info!(
                    target: "server_log",
                    ">>> {} sent close with code {} and reason `{}`",
                    who, cf.code, cf.reason
                );
            } else {
                info!(target: "server_log", ">>> {who} somehow sent close message without CloseFrame");
            }
            return ControlFlow::Break(());
        }
        Message::Pong(_v) => {
            let msg = ClientMessage::Pong(who);
            let _ = client_channel.send(msg);
        }
        Message::Ping(_v) => {
            //println!(">>> {who} sent ping with {v:?}");
        }
    }

    ControlFlow::Continue(())
}

async fn ping_check_system(shared_state: &Arc<RwLock<AppState>>) {
    loop {
        // send ping to all sockets
        let app_state = shared_state.read().await;
        let socks = app_state.sockets.clone();
        drop(app_state);

        let mut handles = Vec::new();
        for (who, socket) in socks.iter() {
            let who = who.clone();
            let socket = socket.clone();
            handles.push(tokio::spawn(async move {
                if socket
                    .socket
                    .lock()
                    .await
                    .send(Message::Ping(vec![1, 2, 3]))
                    .await
                    .is_ok()
                {
                    return None;
                } else {
                    return Some((who.clone(), socket.pubkey.clone()));
                }
            }));
        }

        // remove any sockets where ping failed
        for handle in handles {
            match handle.await {
                Ok(Some((who, pubkey))) => {
                    error!(target: "server_log", "Got error sending ping to client: {} on pk: {}.", who, pubkey);
                    let mut app_state = shared_state.write().await;
                    app_state.sockets.remove(&who);
                }
                Ok(None) => {}
                Err(_) => {
                    error!(target: "server_log", "Got error sending ping to client.");
                }
            }
        }

        tokio::time::sleep(Duration::from_secs(30)).await;
    }
}

#[derive(Deserialize)]
struct PubkeyMintParam {
    pubkey: String,
    mint: String,
}

async fn get_miner_balance_v2(
    query_params: Query<PubkeyMintParam>,
    Extension(rpc_client): Extension<Arc<RpcClient>>,
) -> impl IntoResponse {
    let mint = match Pubkey::from_str(&query_params.mint) {
        Ok(pk) => pk,
        Err(_) => {
            error!(target: "server_log", "get_miner_balance_v2 - Failed to parse mint");
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body("Invalid Mint".to_string())
                .unwrap();
        }
    };
    if let Ok(user_pubkey) = Pubkey::from_str(&query_params.pubkey) {
        let miner_token_account = get_associated_token_address(&user_pubkey, &mint);
        if let Ok(response) = rpc_client
            .get_token_account_balance(&miner_token_account)
            .await
        {
            Response::builder()
                .status(StatusCode::OK)
                .body(response.ui_amount_string)
                .unwrap()
        } else {
            Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body("Failed to get token account balance".to_string())
                .unwrap()
        }
    } else {
        Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body("Invalid public key".to_string())
            .unwrap()
    }
}

pub async fn get_guild_check_member(
    query_params: Query<PubkeyParam>,
    Extension(app_config): Extension<Arc<Config>>,
    Extension(rpc_client): Extension<Arc<RpcClient>>,
) -> impl IntoResponse {
    if let Ok(user_pubkey) = Pubkey::from_str(&query_params.pubkey) {
        let member = member_pda(user_pubkey);
        let member_data = rpc_client.get_account_data(&member.0).await;
        // let's check guild that the user is in, if any
        match member_data {
            Err(_) => {
                info!(target: "server_log", "Pubkey: {} has no member_data. Answering with generation response", user_pubkey.to_string());
                return Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .header("Content-Type", "text/text")
                    .body("User info for guild not found".to_string())
                    .unwrap();
            }
            Ok(data) => {
                if let Ok(member) = Member::try_from_bytes(&data) {
                    if member.guild.to_string().is_empty()
                        || member
                            .guild
                            .to_string()
                            .eq("11111111111111111111111111111111")
                    {
                        info!(target: "server_log", "Pubkey: {} without any guild. We can continue with the flow without any extra steps", user_pubkey.to_string());
                        return Response::builder()
                            .status(StatusCode::OK)
                            .header("Content-Type", "text/text")
                            .body("SUCCESS".to_string())
                            .unwrap();
                    } else if member
                        .guild
                        .to_string()
                        .eq(&app_config.guild_address.to_string())
                    {
                        info!(target: "server_log", "Pubkey: {} is already in the guild. No extra steps needed", user_pubkey.to_string());
                        return Response::builder()
                            .status(StatusCode::FOUND)
                            .header("Content-Type", "text/text")
                            .body("SUCCESS".to_string())
                            .unwrap();
                    } else {
                        error!(target: "server_log", "Pubkey: {} already in another guild {}. Leave it first before joining", user_pubkey.to_string(), member.guild.to_string());
                        return Response::builder()
                            .status(StatusCode::BAD_REQUEST)
                            .header("Content-Type", "text/text")
                            .body("Public key already in another guild. Leave it first before joining".to_string())
                            .unwrap();
                    };
                } else {
                    error!(target: "server_log", "Pubkey: {} Invalid public key", user_pubkey.to_string());
                    return Response::builder()
                        .status(StatusCode::BAD_REQUEST)
                        .header("Content-Type", "text/text")
                        .body("Invalid public key".to_string())
                        .unwrap();
                }
            }
        }
    } else {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header("Content-Type", "text/text")
            .body("Invalid public key".to_string())
            .unwrap();
    }
}

pub async fn get_guild_lp_staking_rewards(
    query_params: Query<PubkeyParam>,
    Extension(app_rr_database): Extension<Arc<AppRRDatabase>>,
) -> impl IntoResponse {
    if let Ok(user_pubkey) = Pubkey::from_str(&query_params.pubkey) {
        let db_rewards = app_rr_database
            .get_extra_resources_rewards_by_pubkey(
                user_pubkey.to_string(),
                ExtraResourcesGenerationType::CoalStakingRewards,
            )
            .await;

        match db_rewards {
            Ok(rewards) => {
                // convert the total COAL rewards to UI amount
                let ui_amount = amount_u64_to_string(rewards.amount_coal);

                Response::builder()
                    .status(StatusCode::OK)
                    .body(ui_amount)
                    .unwrap()
            }
            Err(e) => {
                error!(target: "server_log", "Error fetching extra resources rewards: {:?}", e);
                Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .header("Content-Type", "text/text")
                    .body("Error fetching extra resources rewards".to_string())
                    .unwrap()
            }
        }
    } else {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header("Content-Type", "text/text")
            .body("Invalid public key".to_string())
            .unwrap();
    }
}

pub async fn get_guild_lp_staking_rewards_24h(
    query_params: Query<PubkeyParam>,
    Extension(app_rr_database): Extension<Arc<AppRRDatabase>>,
) -> impl IntoResponse {
    if let Ok(user_pubkey) = Pubkey::from_str(&query_params.pubkey) {
        let db_rewards = app_rr_database
            .get_extra_resources_rewards_24h_by_pubkey(
                user_pubkey.to_string(),
                ExtraResourcesGenerationType::CoalStakingRewards,
            )
            .await;

        match db_rewards {
            Ok(rewards) => {
                // convert the total COAL rewards to UI amount
                let ui_amount = amount_u64_to_string(rewards.amount_coal);

                Response::builder()
                    .status(StatusCode::OK)
                    .body(ui_amount)
                    .unwrap()
            }
            Err(e) => {
                error!(target: "server_log", "Error fetching extra resources rewards: {:?}", e);
                Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .header("Content-Type", "text/text")
                    .body("Error fetching extra resources rewards".to_string())
                    .unwrap()
            }
        }
    } else {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header("Content-Type", "text/text")
            .body("Invalid public key".to_string())
            .unwrap();
    }
}

pub async fn get_guild_new_member_instruction(
    query_params: Query<PubkeyParam>,
) -> impl IntoResponse {
    if let Ok(user_pubkey) = Pubkey::from_str(&query_params.pubkey) {
        let new_member_instruction = coal_guilds_api::sdk::new_member(user_pubkey);

        Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/text")
            .body(
                serde_json::to_string(&new_member_instruction)
                    .unwrap()
                    .to_string(),
            )
            .unwrap()
    } else {
        Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header("Content-Type", "text/text")
            .body("Invalid public key".to_string())
            .unwrap()
    }
}

pub async fn get_guild_delegate_instruction(
    query_params: Query<PubkeyParam>,
    Extension(app_config): Extension<Arc<Config>>,
) -> impl IntoResponse {
    if let (Ok(user_pubkey), Ok(guild_pubkey)) = (
        Pubkey::from_str(&query_params.pubkey),
        Pubkey::from_str(&app_config.guild_address),
    ) {
        info!(target: "server_log", "Pubkey: {} is trying to delegate to Guild: {}", user_pubkey.to_string(), guild_pubkey.to_string());

        let delegate_instruction = coal_guilds_api::sdk::delegate(user_pubkey, guild_pubkey);

        Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/text")
            .body(
                serde_json::to_string(&delegate_instruction)
                    .unwrap()
                    .to_string(),
            )
            .unwrap()
    } else {
        Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header("Content-Type", "text/text")
            .body("Invalid public key".to_string())
            .unwrap()
    }
}

pub async fn get_guild_stake_instruction(
    query_params: Query<GuildStakeParams>,
    Extension(app_config): Extension<Arc<Config>>,
) -> impl IntoResponse {
    if let (Ok(user_pubkey), Ok(guild_pubkey)) = (
        Pubkey::from_str(&query_params.pubkey),
        Pubkey::from_str(&app_config.guild_address),
    ) {
        info!(target: "server_log", "Pubkey: {} is trying to stake to Guild: {} {} LP", user_pubkey.to_string(), guild_pubkey.to_string(), query_params.amount);
        let stake_instruction =
            coal_guilds_api::sdk::stake(user_pubkey, guild_pubkey, query_params.amount);

        Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/text")
            .body(
                serde_json::to_string(&stake_instruction)
                    .unwrap()
                    .to_string(),
            )
            .unwrap()
    } else {
        Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header("Content-Type", "text/text")
            .body("Invalid public key".to_string())
            .unwrap()
    }
}

pub async fn get_guild_unstake_instruction(
    query_params: Query<GuildStakeParams>,
    Extension(app_config): Extension<Arc<Config>>,
) -> impl IntoResponse {
    if let (Ok(user_pubkey), Ok(guild_pubkey)) = (
        Pubkey::from_str(&query_params.pubkey),
        Pubkey::from_str(&app_config.guild_address),
    ) {
        info!(target: "server_log", "Pubkey: {} is trying to stake to Guild: {} {} LP", user_pubkey.to_string(), guild_pubkey.to_string(), query_params.amount);
        let stake_instruction =
            coal_guilds_api::sdk::unstake(user_pubkey, guild_pubkey, query_params.amount);

        Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/text")
            .body(
                serde_json::to_string(&stake_instruction)
                    .unwrap()
                    .to_string(),
            )
            .unwrap()
    } else {
        Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header("Content-Type", "text/text")
            .body("Invalid public key".to_string())
            .unwrap()
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StakeAndMultipliers {
    coal_multiplier: f64,
    coal_stake: f64,
    guild_multiplier: f64,
    guild_stake: f64,
    tool_multiplier: f64,
    ore_stake: f64,
}

pub async fn get_pool_stakes_and_multipliers(
    Extension(app_wallet): Extension<Arc<WalletExtension>>,
    Extension(rpc_client): Extension<Arc<RpcClient>>,
) -> Result<Json<StakeAndMultipliers>, String> {
    // Fetch coal_proof
    let config_address = get_config_pubkey(&Resource::Coal);
    let tool_address = get_tool_pubkey(
        app_wallet.clone().miner_wallet.clone().pubkey(),
        &Resource::Coal,
    );
    let guild_config_address = coal_guilds_api::state::config_pda().0;
    let guild_member_address =
        coal_guilds_api::state::member_pda(app_wallet.clone().miner_wallet.clone().pubkey()).0;

    let mut accounts_multipliers = vec![
        config_address,
        tool_address,
        guild_config_address,
        guild_member_address,
    ];
    let accounts_multipliers = rpc_client
        .get_multiple_accounts(&accounts_multipliers)
        .await
        .unwrap();

    let mut tool: Option<ToolType> = None;
    let mut member: Option<coal_guilds_api::state::Member> = None;
    let mut guild_config: Option<coal_guilds_api::state::Config> = None;
    let mut guild: Option<coal_guilds_api::state::Guild> = None;
    let mut guild_address: Option<Pubkey> = None;

    info!(target: "server_log", "setting up accounts");

    if accounts_multipliers.len() > 1 {
        if accounts_multipliers[1].as_ref().is_some() {
            tool = Some(deserialize_tool(
                &accounts_multipliers[1].as_ref().unwrap().data,
                &Resource::Coal,
            ));
        }

        if accounts_multipliers.len() > 2 && accounts_multipliers[2].as_ref().is_some() {
            guild_config = Some(deserialize_guild_config(
                &accounts_multipliers[2].as_ref().unwrap().data,
            ));
        }

        if accounts_multipliers.len() > 3 && accounts_multipliers[3].as_ref().is_some() {
            member = Some(deserialize_guild_member(
                &accounts_multipliers[3].as_ref().unwrap().data,
            ));
        }

        if accounts_multipliers.len() > 4 && accounts_multipliers[4].as_ref().is_some() {
            guild = Some(deserialize_guild(
                &accounts_multipliers[4].as_ref().unwrap().data,
            ));
        }
    }

    info!(target: "server_log", "getting guild info");

    if member.is_some() && member.unwrap().guild.ne(&coal_guilds_api::ID) && guild_address.is_none()
    {
        let guild_data = rpc_client
            .get_account_data(&member.unwrap().guild)
            .await
            .unwrap();
        guild = Some(deserialize_guild(&guild_data));
        guild_address = Some(member.unwrap().guild);
    }

    let tool_multiplier = calculate_tool_multiplier(&tool);

    let guild_stake = guild.unwrap().total_stake as f64;
    let guild_multiplier = calculate_multiplier(
        guild_config.unwrap().total_stake,
        guild_config.unwrap().total_multiplier,
        guild.unwrap().total_stake,
    );

    let mut loaded_config_coal = None;
    let mut loaded_config_proof_coal = None;
    info!(target: "server_log", "Getting latest config and busses data.");
    if let (Ok(p), Ok(config), Ok(_busses)) =
        get_proof_and_config_with_busses_coal(&rpc_client, app_wallet.miner_wallet.pubkey()).await
    {
        loaded_config_coal = Some(config);
        loaded_config_proof_coal = Some(p);
    }

    let coal_stake = loaded_config_proof_coal.unwrap().balance as f64;

    let coal_multiplier = calculate_multiplier(
        loaded_config_coal.unwrap().top_balance,
        2,
        loaded_config_proof_coal.unwrap().balance,
    );

    let mut loaded_config_ore = None;
    let mut loaded_config_proof_ore = None;
    info!(target: "server_log", "Getting latest config and busses data.");
    if let (Ok(p), Ok(config), Ok(_busses)) =
        get_proof_and_config_with_busses_ore(&rpc_client, app_wallet.miner_wallet.pubkey()).await
    {
        loaded_config_ore = Some(config);
        loaded_config_proof_ore = Some(p);
    }

    let ore_stake = loaded_config_proof_ore.unwrap().balance as f64;

    return Ok(Json(StakeAndMultipliers {
        coal_multiplier,
        coal_stake,
        guild_multiplier,
        guild_stake,
        tool_multiplier,
        ore_stake,
    }));
}

async fn get_miner_guild_stake(
    query_params: Query<PubkeyParam>,
    Extension(rpc_client): Extension<Arc<RpcClient>>,
) -> impl IntoResponse {
    if let Ok(user_pubkey) = (Pubkey::from_str(&query_params.pubkey)) {
        let member_address = coal_guilds_api::state::member_pda(user_pubkey).0;
        let miner_token_account = get_associated_token_address(&member_address, &LP_MINT_ADDRESS);
        if let Ok(response) = rpc_client
            .get_token_account_balance(&miner_token_account)
            .await
        {
            Response::builder()
                .status(StatusCode::OK)
                .body(response.ui_amount_string)
                .unwrap()
        } else {
            Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body("Failed to get token account balance".to_string())
                .unwrap()
        }
    } else {
        Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header("Content-Type", "text/text")
            .body("Invalid public key".to_string())
            .unwrap()
    }
}
