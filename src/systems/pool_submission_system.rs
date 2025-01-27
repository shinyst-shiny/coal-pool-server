use b64::FromBase64;
use bs58;
use rand::seq::SliceRandom;
use serde_json::json;
use std::{
    collections::HashMap,
    ops::{Mul, Range},
    str::FromStr,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use crate::coal_utils::{
    amount_u64_to_string, calculate_multiplier, calculate_tool_multiplier, deserialize_config,
    deserialize_guild, deserialize_guild_config, deserialize_guild_member, deserialize_tool,
    get_coal_balance, get_config_pubkey, get_tool_pubkey, Resource, ToolType,
};
use crate::ore_utils::{
    get_ore_auth_ix, get_ore_balance, get_ore_mine_ix,
    get_proof_and_config_with_busses as get_proof_and_config_with_busses_ore, get_reservation,
    get_reset_ix as get_reset_ix_ore, ore_proof_pubkey, ORE_TOKEN_DECIMALS,
};
use crate::{
    app_database::AppDatabase,
    coal_utils::{
        get_auth_ix, get_cutoff, get_guild_member, get_guild_proof, get_mine_ix, get_proof,
        get_proof_and_config_with_busses as get_proof_and_config_with_busses_coal,
        get_reset_ix as get_reset_ix_coal, COAL_TOKEN_DECIMALS,
    },
    Config, EpochHashes, InsertChallenge, InsertEarning, InsertTxn, MessageInternalAllClients,
    MessageInternalMineSuccess, PoolGuildMember, SubmissionWindow, UpdateReward, WalletExtension,
};
use base64::{prelude::BASE64_STANDARD, Engine};
use coal_api::{consts::BUS_COUNT, state::Proof};
use coal_guilds_api::state::config_pda;
use futures::future::try_join;
use jito_sdk_rust::JitoJsonRpcSDK;
use ore_api::state::proof_pda;
use ore_boost_api::state::reservation_pda;
use rand::Rng;
use solana_client::{
    nonblocking::rpc_client::RpcClient,
    rpc_config::{RpcSendTransactionConfig, RpcSimulateTransactionConfig, RpcTransactionConfig},
};
use solana_sdk::commitment_config::CommitmentLevel;
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction,
    instruction::InstructionError,
    native_token::lamports_to_sol,
    pubkey::Pubkey,
    signature::Signature,
    signer::Signer,
    system_instruction::transfer,
    transaction::{Transaction, TransactionError},
};
use solana_transaction_status::option_serializer::OptionSerializer;
use solana_transaction_status::{TransactionConfirmationStatus, UiTransactionEncoding};
use tokio::{
    sync::{mpsc::UnboundedSender, Mutex, RwLock},
    time::Instant,
};
use tracing::{error, info};
use uuid::Uuid;

#[derive(Debug)]
struct BundleStatus {
    confirmation_status: Option<String>,
    err: Option<serde_json::Value>,
    transactions: Option<Vec<String>>,
}

pub async fn pool_submission_system(
    app_proof: Arc<Mutex<Proof>>,
    app_epoch_hashes: Arc<RwLock<EpochHashes>>,
    app_wallet: Arc<WalletExtension>,
    app_nonce: Arc<Mutex<u64>>,
    app_prio_fee: Arc<u64>,
    app_jito_tip: Arc<u64>,
    rpc_client: Arc<RpcClient>,
    jito_client: Arc<JitoJsonRpcSDK>,
    config: Arc<Config>,
    app_database: Arc<AppDatabase>,
    app_all_clients_sender: UnboundedSender<MessageInternalAllClients>,
    mine_success_sender: UnboundedSender<MessageInternalMineSuccess>,
    app_submission_window: Arc<RwLock<SubmissionWindow>>,
    app_client_nonce_ranges: Arc<RwLock<HashMap<Uuid, Vec<Range<u64>>>>>,
    app_last_challenge: Arc<Mutex<[u8; 32]>>,
) {
    loop {
        let lock = app_proof.lock().await;
        let old_proof = lock.clone();
        drop(lock);

        let cutoff = get_cutoff(old_proof, 0);
        if cutoff <= 0 {
            // process solutions
            let reader = app_epoch_hashes.read().await;
            let solution = reader.best_hash.solution.clone();
            drop(reader);
            if solution.is_some() {
                // Close submission window
                info!(target: "server_log", "Submission window closed.");
                let mut writer = app_submission_window.write().await;
                writer.closed = true;
                drop(writer);

                let signer = app_wallet.clone().miner_wallet.clone();

                let bus = rand::thread_rng().gen_range(0..BUS_COUNT);

                let mut success = false;
                let reader = app_epoch_hashes.read().await;
                let best_solution = reader.best_hash.solution.clone();
                let submissions = reader.submissions.clone();
                drop(reader);

                let app_app_wallet = app_wallet.clone();

                for i in 0..10 {
                    let app_wallet = app_app_wallet.clone();
                    if let Some(best_solution) = best_solution {
                        let difficulty = best_solution.to_hash().difficulty();

                        info!(target: "server_log",
                            "Starting mine submission attempt {} with difficulty {}.",
                            i, difficulty
                        );
                        info!(target: "server_log", "Submission Challenge: {}", BASE64_STANDARD.encode(old_proof.challenge));

                        // Fetch coal_proof
                        let config_address = get_config_pubkey(&Resource::Coal);
                        let tool_address = get_tool_pubkey(
                            app_wallet.clone().miner_wallet.clone().pubkey(),
                            &Resource::Coal,
                        );
                        let guild_config_address = coal_guilds_api::state::config_pda().0;
                        let guild_member_address = coal_guilds_api::state::member_pda(
                            app_wallet.clone().miner_wallet.clone().pubkey(),
                        )
                        .0;

                        let mut accounts_multipliers = vec![
                            config_address,
                            tool_address,
                            guild_config_address,
                            guild_member_address,
                        ];
                        let accounts_multipliers = match rpc_client
                            .get_multiple_accounts(&accounts_multipliers)
                            .await
                        {
                            Ok(accounts) => accounts,
                            Err(e) => {
                                error!(target: "server_log", "Failed to get program accounts: {:?}", e);
                                Vec::new() // Return an empty vector in case of error
                            }
                        };

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

                            if accounts_multipliers.len() > 2
                                && accounts_multipliers[2].as_ref().is_some()
                            {
                                guild_config = Some(deserialize_guild_config(
                                    &accounts_multipliers[2].as_ref().unwrap().data,
                                ));
                            }

                            if accounts_multipliers.len() > 3
                                && accounts_multipliers[3].as_ref().is_some()
                            {
                                member = Some(deserialize_guild_member(
                                    &accounts_multipliers[3].as_ref().unwrap().data,
                                ));
                            }

                            if accounts_multipliers.len() > 4
                                && accounts_multipliers[4].as_ref().is_some()
                            {
                                guild = Some(deserialize_guild(
                                    &accounts_multipliers[4].as_ref().unwrap().data,
                                ));
                            }
                        }

                        info!(target: "server_log", "getting guild info");

                        tokio::time::sleep(Duration::from_millis(1000)).await;

                        if member.is_some()
                            && member.unwrap().guild.ne(&coal_guilds_api::ID)
                            && guild_address.is_none()
                        {
                            let guild_data = rpc_client
                                .get_account_data(&member.unwrap().guild)
                                .await
                                .unwrap();
                            guild = Some(deserialize_guild(&guild_data));
                            guild_address = Some(member.unwrap().guild);
                        }

                        let mut tool_multiplier = calculate_tool_multiplier(&tool);

                        if (tool_multiplier <= 0.0) {
                            tool_multiplier = 1.0;
                        }

                        let coal_guild_accounts = match rpc_client
                            .get_program_accounts(&coal_guilds_api::id())
                            .await
                        {
                            Ok(accounts) => accounts,
                            Err(e) => {
                                error!(target: "server_log", "Failed to get program accounts: {:?}", e);
                                Vec::new() // Return an empty vector in case of error
                            }
                        };

                        let mut guilds = Vec::new();
                        let mut guild_members = Vec::new();
                        let mut solo_stakers = Vec::new();
                        let mut pool_guild_members = Vec::new();

                        for (pubkey, account) in coal_guild_accounts {
                            if account.data[0]
                                .eq(&(coal_guilds_api::state::GuildsAccount::Guild as u8))
                            {
                                let guild = deserialize_guild(&account.data);
                                if guild.total_stake.gt(&0) {
                                    guilds.push((pubkey, guild));
                                }
                            } else if account.data[0]
                                .eq(&(coal_guilds_api::state::GuildsAccount::Member as u8))
                            {
                                let member = deserialize_guild_member(&account.data);
                                if member.guild.eq(&solana_sdk::system_program::id())
                                    && member.total_stake.gt(&0)
                                {
                                    solo_stakers.push((pubkey, member));
                                } else if member.total_stake.gt(&0) {
                                    guild_members.push((pubkey, member));
                                }
                            }
                        }
                        println!("Guilds found: {}", guilds.len());

                        for (pubkey, guild) in guilds {
                            println!("{}: {}", pubkey.to_string(), guild.total_stake);

                            if (pubkey.to_string().eq(&config.guild_address.to_string())) {
                                let guild_members_in_guild: Vec<_> = guild_members
                                    .iter()
                                    .filter(|(_, member)| member.guild.eq(&pubkey))
                                    .collect();

                                println!("  Members: {}", guild_members_in_guild.len());
                                for (_, member) in guild_members_in_guild {
                                    let percentage_of_guild_stake = (member.total_stake as u128)
                                        .saturating_mul(1_000_000)
                                        .saturating_div(guild.total_stake as u128);
                                    pool_guild_members.push(PoolGuildMember {
                                        stake_percentage: percentage_of_guild_stake,
                                        member: member.clone(),
                                    });
                                    println!(
                                        "    {}: {} ({}%)",
                                        member.authority.to_string(),
                                        member.total_stake,
                                        percentage_of_guild_stake
                                    );
                                }
                                break;
                            }
                        }

                        let mut loaded_config_coal = None;
                        info!(target: "server_log", "Getting latest config and busses data.");
                        tokio::time::sleep(Duration::from_millis(1000)).await;
                        if let (Ok(p), Ok(config), Ok(_busses)) =
                            get_proof_and_config_with_busses_coal(&rpc_client, signer.pubkey())
                                .await
                        {
                            loaded_config_coal = Some(config);

                            info!(target: "server_log", "Proof COAL: {:?}",p);

                            if !best_solution.is_valid(&p.challenge) {
                                error!(target: "server_log", "SOLUTION IS NOT VALID ANYMORE!");
                                info!(target: "server_log", "Updating to latest proof.");
                                let mut lock = app_proof.lock().await;
                                *lock = p;
                                drop(lock);
                                break;
                            }
                        }
                        let mut loaded_config_ore = None;
                        if let (Ok(p), Ok(config), Ok(_busses)) =
                            get_proof_and_config_with_busses_ore(&rpc_client, signer.pubkey()).await
                        {
                            loaded_config_ore = Some(config);

                            info!(target: "server_log", "Proof ORE: {:?}",p);
                        }

                        tokio::time::sleep(Duration::from_millis(1000)).await;

                        let ore_proof_address = proof_pda(signer.pubkey()).0;

                        let reservation_address = reservation_pda(ore_proof_address).0;
                        let reservation = get_reservation(&rpc_client, reservation_address).await;

                        info!(target: "server_log", "reservation: {:?}", reservation);

                        let boost_address = reservation
                            .map(|r| {
                                if r.boost == Pubkey::default() {
                                    None
                                } else {
                                    Some(r.boost)
                                }
                            })
                            .unwrap_or(None);
                        let boost_keys = if let Some(boost_address) = boost_address {
                            Some((boost_address, reservation_address))
                        } else {
                            None
                        };

                        let now = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .expect("Time went backwards")
                            .as_secs();

                        let mut ixs_coal = vec![];
                        let mut ixs_ore = vec![];
                        let mut prio_fee = *app_prio_fee;

                        let _ = app_all_clients_sender.send(MessageInternalAllClients {
                            text: String::from("Server is sending mine transaction..."),
                        });

                        let mut cu_limit_coal = 500_000;
                        let should_add_reset_ix_coal = if let Some(config) = loaded_config_coal {
                            let time_until_reset = (config.last_reset_at + 300) - now as i64;
                            if time_until_reset <= 5 {
                                cu_limit_coal += 50_000;
                                info!(target: "server_log", "Including reset tx COAL.");
                                true
                            } else {
                                false
                            }
                        } else {
                            false
                        };

                        let mut cu_limit_ore = 500_000;
                        let should_add_reset_ix_ore = if let Some(config) = loaded_config_ore {
                            let time_until_reset = (config.last_reset_at + 300) - now as i64;
                            if time_until_reset <= 5 {
                                cu_limit_ore += 50_000;
                                info!(target: "server_log", "Including reset tx ORE.");
                                true
                            } else {
                                false
                            }
                        } else {
                            false
                        };

                        info!(target: "server_log", "using priority fee of {}", prio_fee);

                        ixs_coal.push(ComputeBudgetInstruction::set_compute_unit_limit(
                            cu_limit_coal,
                        ));
                        ixs_ore.push(ComputeBudgetInstruction::set_compute_unit_limit(
                            cu_limit_ore,
                        ));

                        if (prio_fee > 0) {
                            ixs_coal
                                .push(ComputeBudgetInstruction::set_compute_unit_price(prio_fee));
                            ixs_ore
                                .push(ComputeBudgetInstruction::set_compute_unit_price(prio_fee));
                        }

                        let jito_tip = *app_jito_tip;
                        if jito_tip > 0 {
                            info!(target: "server_log", "adding tip {}", jito_tip);
                            let tip_accounts = [
                                "96gYZGLnJYVFmbjzopPSU6QiEV5fGqZNyN9nmNhvrZU5",
                                "HFqU5x63VTqvQss8hp11i4wVV8bD44PvwucfZ2bU7gRe",
                                "Cw8CFyM9FkoMi7K7Crf6HNQqf4uEMzpKw6QNghXLvLkY",
                                "ADaUMid9yfUytqMBgopwjb2DTLSokTSzL1zt6iGPaS49",
                                "DfXygSm4jCyNCybVYYK6DwvWqjKee8pbDmJGcLWNDXjh",
                                "ADuUkR4vqLUMWXxW9gh6D6L8pMSawimctcNZ5pGwDcEt",
                                "DttWaMuVvTiduZRnguLF7jNxTgiMBZ1hyAumKUiL2KRL",
                                "3AVi9Tg9Uo68tJfuvoKvqKNWKkC5wPdSSdeBnizKZ6jT",
                            ];
                            ixs_ore.push(transfer(
                                &signer.pubkey(),
                                &Pubkey::from_str(
                                    &tip_accounts
                                        .choose(&mut rand::thread_rng())
                                        .unwrap()
                                        .to_string(),
                                )
                                .unwrap(),
                                jito_tip,
                            ));

                            info!(target: "server_log", "Jito tip: {} SOL", lamports_to_sol(jito_tip));
                        }

                        info!(target: "server_log", "Adding noop ix ORE");

                        let coal_noop_ix = get_auth_ix(signer.pubkey());
                        ixs_coal.push(coal_noop_ix.clone());
                        ixs_coal.push(coal_noop_ix.clone());

                        let ore_noop_ix = get_ore_auth_ix(signer.pubkey());
                        ixs_ore.push(ore_noop_ix.clone());

                        info!(target: "server_log", "Adding reset ix COAL");

                        if should_add_reset_ix_coal {
                            ixs_coal.push(get_reset_ix_coal(signer.pubkey()));
                        }

                        if should_add_reset_ix_ore {
                            let reset_ix = get_reset_ix_ore(signer.pubkey());
                            ixs_ore.push(reset_ix);
                        }

                        info!(target: "server_log","Using for the transaction Signer: {:?} tool: {:?}, member: {:?}, guild_address: {:?}", signer.pubkey(), tool_address, guild_member_address, guild_address);

                        let coal_mine_ix = get_mine_ix(
                            signer.pubkey(),
                            best_solution,
                            bus,
                            Option::from(tool_address),
                            Option::from(guild_member_address),
                            Option::from(guild_address),
                        );

                        /*let coal_mine_ix =
                        get_mine_ix(signer.pubkey(), best_solution, bus, None, None, None);*/

                        ixs_coal.push(coal_mine_ix);

                        info!(target: "server_log", "boost_keys: {:?}",boost_keys);

                        let ore_mine_ix =
                            get_ore_mine_ix(signer.pubkey(), best_solution, bus, boost_keys);

                        ixs_ore.push(ore_mine_ix);

                        // Build rotation ix
                        let rotate_ix =
                            ore_boost_api::sdk::rotate(signer.pubkey(), ore_proof_address);
                        ixs_ore.push(rotate_ix);

                        info!(target: "server_log", "built ixs getting balances...");

                        let mut ore_balance_before_tx = 0;

                        if let Ok(ore_balance) = get_ore_balance(
                            app_wallet.clone().miner_wallet.pubkey(),
                            &rpc_client.clone(),
                        )
                        .await
                        {
                            ore_balance_before_tx = ore_balance
                        }

                        let mut coal_balance_before_tx = 0;

                        if let Ok(coal_balance) = get_coal_balance(
                            app_wallet.clone().miner_wallet.pubkey(),
                            &rpc_client.clone(),
                        )
                        .await
                        {
                            coal_balance_before_tx = coal_balance
                        }

                        info!(target: "server_log", "got balance {} ORE {} COAL, sending to rpc_client", ore_balance_before_tx, coal_balance_before_tx);

                        if let Ok((hash, _slot)) = rpc_client
                            .get_latest_blockhash_with_commitment(rpc_client.commitment())
                            .await
                        {
                            info!(target: "server_log", "Got block building tx...");

                            let mut tx_coal =
                                Transaction::new_with_payer(&ixs_coal, Some(&signer.pubkey()));
                            let mut tx_ore =
                                Transaction::new_with_payer(&ixs_ore, Some(&signer.pubkey()));

                            let expired_timer = Instant::now();
                            tx_coal.sign(&[&signer], hash);
                            tx_ore.sign(&[&signer], hash);

                            let serialized_tx_coal =
                                bs58::encode(bincode::serialize(&tx_coal).unwrap()).into_string();
                            let serialized_tx_ore =
                                bs58::encode(bincode::serialize(&tx_ore).unwrap()).into_string();

                            let sim_tx_coal =
                                rpc_client.simulate_transaction(&tx_coal).await.unwrap();
                            info!(target: "server_log", "Simulation result: {:?}", sim_tx_coal);
                            let sim_tx_ore =
                                rpc_client.simulate_transaction(&tx_ore).await.unwrap();
                            info!(target: "server_log", "Simulation result: {:?}", sim_tx_ore);

                            // Prepare bundle for submission (array of transactions)
                            let bundle = json!([serialized_tx_coal, serialized_tx_ore]);

                            info!(target: "server_log", "Sending bundle tx...");
                            info!(target: "server_log", "attempt: {}", i + 1);

                            let mut bundle_send_attempt = 1;

                            let (tx_message_sender, tx_message_receiver) =
                                tokio::sync::oneshot::channel::<u8>();
                            let app_app_nonce = app_nonce.clone();
                            let app_app_database = app_database.clone();
                            let app_app_config = config.clone();
                            let app_app_rpc_client = rpc_client.clone();
                            let app_app_proof = app_proof.clone();
                            let app_app_wallet = app_wallet.clone();
                            let app_app_epoch_hashes = app_epoch_hashes.clone();
                            let app_app_submission_window = app_submission_window.clone();
                            let app_app_client_nonce_ranges = app_client_nonce_ranges.clone();
                            let app_app_last_challenge = app_last_challenge.clone();
                            tokio::spawn(async move {
                                let mut stop_reciever = tx_message_receiver;
                                let app_nonce = app_app_nonce;
                                let app_database = app_app_database;
                                let app_config = app_app_config;
                                let app_rpc_client = app_app_rpc_client;
                                let app_proof = app_app_proof;
                                let app_wallet = app_app_wallet;
                                let app_epoch_hashes = app_app_epoch_hashes;
                                let app_submission_window = app_app_submission_window;
                                let app_client_nonce_ranges = app_app_client_nonce_ranges;
                                let app_last_challenge = app_app_last_challenge;
                                tokio::time::sleep(Duration::from_millis(500)).await;
                                loop {
                                    if let Ok(_) = stop_reciever.try_recv() {
                                        // Transaction has succeeded or expired
                                        break;
                                    } else {
                                        // Wait 100ms then check for updated proof
                                        tokio::time::sleep(Duration::from_millis(100)).await;

                                        info!(target: "server_log", "Checking for proof hash update.");
                                        let lock = app_proof.lock().await;
                                        let latest_proof = lock.clone();
                                        drop(lock);

                                        if old_proof.challenge.eq(&latest_proof.challenge) {
                                            // info!(target: "server_log", "Proof challenge not updated yet..");
                                            if let Ok(p) = get_proof(
                                                &app_rpc_client,
                                                app_wallet.clone().miner_wallet.pubkey(),
                                            )
                                            .await
                                            {
                                                info!(target: "server_log", "OLD PROOF CHALLENGE: {}", BASE64_STANDARD.encode(old_proof.challenge));
                                                info!(target: "server_log", "RPC PROOF CHALLENGE: {}", BASE64_STANDARD.encode(p.challenge));
                                                if old_proof.challenge.ne(&p.challenge) {
                                                    info!(target: "server_log", "Found new proof from rpc call, not websocket...");
                                                    let mut lock = app_proof.lock().await;
                                                    *lock = p;
                                                    drop(lock);

                                                    let mut lock = app_last_challenge.lock().await;
                                                    *lock = old_proof.challenge;
                                                    drop(lock);

                                                    // Add new db challenge, reset epoch_hashes,
                                                    // and open the submission window

                                                    // reset nonce
                                                    {
                                                        let mut nonce = app_nonce.lock().await;
                                                        *nonce = 0;
                                                    }
                                                    // reset client nonce ranges
                                                    {
                                                        let mut writer =
                                                            app_client_nonce_ranges.write().await;
                                                        *writer = HashMap::new();
                                                        drop(writer);
                                                    }
                                                    // reset epoch hashes
                                                    {
                                                        info!(target: "server_log", "reset epoch hashes");
                                                        let mut mut_epoch_hashes =
                                                            app_epoch_hashes.write().await;
                                                        mut_epoch_hashes.challenge = p.challenge;
                                                        mut_epoch_hashes.best_hash.solution = None;
                                                        mut_epoch_hashes.best_hash.difficulty = 0;
                                                        mut_epoch_hashes.submissions =
                                                            HashMap::new();
                                                    }
                                                    // Open submission window
                                                    info!(target: "server_log", "openning submission window.");
                                                    let mut writer =
                                                        app_submission_window.write().await;
                                                    writer.closed = false;
                                                    drop(writer);

                                                    info!(target: "server_log", "Adding new challenge to db");
                                                    let new_challenge = InsertChallenge {
                                                        pool_id: app_config.pool_id,
                                                        challenge: p.challenge.to_vec(),
                                                        rewards_earned_coal: None,
                                                        rewards_earned_ore: None,
                                                    };

                                                    while let Err(_) = app_database
                                                        .add_new_challenge(new_challenge.clone())
                                                        .await
                                                    {
                                                        tracing::error!(target: "server_log", "Failed to add new challenge to db.");
                                                        info!(target: "server_log", "Verifying challenge does not already exist.");
                                                        if let Ok(_) = app_database
                                                            .get_challenge_by_challenge(
                                                                new_challenge.challenge.clone(),
                                                            )
                                                            .await
                                                        {
                                                            info!(target: "server_log", "Challenge already exists, continuing");
                                                            break;
                                                        }

                                                        tokio::time::sleep(Duration::from_millis(
                                                            1000,
                                                        ))
                                                        .await;
                                                    }
                                                    info!(target: "server_log", "New challenge successfully added to db");

                                                    break;
                                                }
                                            }
                                        } else {
                                            let mut lock = app_last_challenge.lock().await;
                                            *lock = old_proof.challenge;
                                            drop(lock);
                                            info!(target: "server_log", "Adding new challenge to db");
                                            let new_challenge = InsertChallenge {
                                                pool_id: app_config.pool_id,
                                                challenge: latest_proof.challenge.to_vec(),
                                                rewards_earned_coal: None,
                                                rewards_earned_ore: None,
                                            };

                                            while let Err(_) = app_database
                                                .add_new_challenge(new_challenge.clone())
                                                .await
                                            {
                                                tracing::error!(target: "server_log", "Failed to add new challenge to db.");
                                                info!(target: "server_log", "Verifying challenge does not already exist.");
                                                if let Ok(_) = app_database
                                                    .get_challenge_by_challenge(
                                                        new_challenge.challenge.clone(),
                                                    )
                                                    .await
                                                {
                                                    info!(target: "server_log", "Challenge already exists, continuing");
                                                    break;
                                                }

                                                tokio::time::sleep(Duration::from_millis(1000))
                                                    .await;
                                            }
                                            info!(target: "server_log", "New challenge successfully added to db");

                                            // reset nonce
                                            {
                                                let mut nonce = app_nonce.lock().await;
                                                *nonce = 0;
                                            }
                                            // reset client nonce ranges
                                            {
                                                let mut writer =
                                                    app_client_nonce_ranges.write().await;
                                                *writer = HashMap::new();
                                                drop(writer);
                                            }
                                            // reset epoch hashes
                                            {
                                                info!(target: "server_log", "reset epoch hashes");
                                                let mut mut_epoch_hashes =
                                                    app_epoch_hashes.write().await;
                                                mut_epoch_hashes.challenge = latest_proof.challenge;
                                                mut_epoch_hashes.best_hash.solution = None;
                                                mut_epoch_hashes.best_hash.difficulty = 0;
                                                mut_epoch_hashes.submissions = HashMap::new();
                                            }
                                            // Open submission window
                                            info!(target: "server_log", "openning submission window.");
                                            let mut writer = app_submission_window.write().await;
                                            writer.closed = false;
                                            drop(writer);

                                            break;
                                        }
                                    }
                                    tokio::time::sleep(Duration::from_millis(1000)).await;
                                }
                                return;
                            });

                            let bundle_status = loop {
                                tokio::time::sleep(Duration::from_millis(1000)).await;
                                // UUID for the bundle
                                let uuid = None;

                                // Send bundle using Jito SDK
                                let response = jito_client
                                    .send_bundle(Some(bundle.clone()), uuid)
                                    .await
                                    .unwrap();

                                // Extract bundle UUID from response
                                let bundle_uuid = match response["result"].as_str() {
                                    Some(uuid) => uuid,
                                    None => {
                                        error!(target: "server_log", "Failed to get bundle id");
                                        bundle_send_attempt += 1;
                                        if bundle_send_attempt >= 5 {
                                            break Err("Failed to send tx");
                                        } else {
                                            continue;
                                        }
                                    }
                                };
                                info!(target: "server_log","Bundle sent with UUID: {}", bundle_uuid);

                                match check_final_bundle_status(
                                    &jito_client.clone(),
                                    bundle_uuid.clone(),
                                )
                                .await
                                {
                                    Ok(status) => break Ok(status),
                                    Err(_) => {
                                        error!(target: "server_log", "Failed to send bundle tx error");
                                        error!(target: "server_log", "Attempt {} Failed to send mine transaction. retrying in 1 seconds...", bundle_send_attempt);
                                        bundle_send_attempt += 1;

                                        if bundle_send_attempt >= 5 {
                                            break Err("Failed to send tx");
                                        }
                                    }
                                }
                            };

                            // stop the tx sender
                            let _ = tx_message_sender.send(0);

                            if (!bundle_status.is_err()) {
                                let bundle_status = bundle_status.unwrap();

                                info!(target: "server_log", "bundle_status: {:?}", bundle_status);

                                if let Some(transactions) = &bundle_status.transactions {
                                    let signature_coal =
                                        Signature::from_str(&transactions[0]).unwrap();
                                    let signature_ore =
                                        Signature::from_str(&transactions[1]).unwrap();

                                    // match result {
                                    //    Ok(sig) => {
                                    // success
                                    success = true;
                                    info!(target: "server_log", "Success!!");
                                    info!(target: "server_log", "signature_coal: {}", signature_coal);
                                    info!(target: "server_log", "signature_ore: {}", signature_ore);
                                    let itxn_coal = InsertTxn {
                                        txn_type: "mine_coal".to_string(),
                                        signature: signature_coal.to_string(),
                                        priority_fee: prio_fee as u32,
                                    };
                                    let itxn_ore = InsertTxn {
                                        txn_type: "mine_ore".to_string(),
                                        signature: signature_ore.to_string(),
                                        priority_fee: prio_fee as u32,
                                    };
                                    let app_db_coal = app_database.clone();
                                    tokio::spawn(async move {
                                        while let Err(_) =
                                            app_db_coal.add_new_txn(itxn_coal.clone()).await
                                        {
                                            error!(target: "server_log", "Failed to add tx to db! Retrying...");
                                            tokio::time::sleep(Duration::from_millis(2000)).await;
                                        }
                                    });
                                    let app_db_ore = app_database.clone();
                                    tokio::spawn(async move {
                                        while let Err(_) =
                                            app_db_ore.add_new_txn(itxn_ore.clone()).await
                                        {
                                            error!(target: "server_log", "Failed to add tx to db! Retrying...");
                                            tokio::time::sleep(Duration::from_millis(2000)).await;
                                        }
                                    });

                                    // get reward amount from MineEvent data and update database
                                    // and clients
                                    let app_rpc_client = rpc_client.clone();
                                    let app_app_database = app_database.clone();
                                    let app_mine_success_sender =
                                        Arc::new(mine_success_sender.clone());
                                    let app_app_proof = app_proof.clone();
                                    let app_app_config = config.clone();

                                    let app_app_wallet = app_wallet.clone();

                                    tokio::spawn(async move {
                                        let app_wallet = app_app_wallet.clone();
                                        let rpc_client = app_rpc_client;
                                        let app_database = app_app_database;
                                        let mine_success_sender = app_mine_success_sender;
                                        let app_proof = app_app_proof;
                                        let app_config = app_app_config;
                                        loop {
                                            let result_coal = rpc_client.get_transaction(
                                                &signature_coal,
                                                UiTransactionEncoding::Json,
                                            );
                                            let result_ore = rpc_client.get_transaction(
                                                &signature_ore,
                                                UiTransactionEncoding::Json,
                                            );

                                            let tnx_results =
                                                try_join(result_coal, result_ore).await;

                                            match tnx_results {
                                                Ok((txn_result_coal, txn_result_ore)) => {
                                                    // let data = txn_result.transaction.meta.unwrap();

                                                    let guild_total_stake =
                                                        guild.unwrap().total_stake as f64;
                                                    let mut guild_multiplier = calculate_multiplier(
                                                        guild_config.unwrap().total_stake,
                                                        guild_config.unwrap().total_multiplier,
                                                        guild.unwrap().total_stake,
                                                    );
                                                    let guild_last_stake_at =
                                                        guild.unwrap().last_stake_at;

                                                    info!(target: "server_log", "---> starting cascade info");

                                                    let mut ore_cloned_data;
                                                    let mut coal_cloned_data;
                                                    let mut ore_mine_event;
                                                    let mut coal_mine_event;

                                                    if let Some(meta) =
                                                        txn_result_coal.transaction.meta.clone()
                                                    {
                                                        if let OptionSerializer::Some(
                                                            log_messages,
                                                        ) = meta.log_messages
                                                        {
                                                            if let Some(return_log) =
                                                                log_messages.iter().find(|log| {
                                                                    log.starts_with(&format!(
                                                                        "Program return: {} ",
                                                                        coal_api::ID
                                                                    ))
                                                                })
                                                            {
                                                                if let Some(return_data) =
                                                                    return_log.strip_prefix(
                                                                        &format!(
                                                                            "Program return: {} ",
                                                                            coal_api::ID
                                                                        ),
                                                                    )
                                                                {
                                                                    if let Ok(return_data) =
                                                                        return_data.from_base64()
                                                                    {
                                                                        coal_cloned_data =
                                                                            return_data.clone();
                                                                        coal_mine_event =
                                                                        bytemuck::try_from_bytes::<
                                                                            coal_api::event::MineEvent,
                                                                        >(
                                                                            &coal_cloned_data
                                                                        );
                                                                        info!(target: "server_log", "COAL MineEvent: {:?}", coal_mine_event);
                                                                        info!(target: "submission_log", "COAL MineEvent: {:?}", coal_mine_event);
                                                                    } else {
                                                                        info!(target: "server_log", "COAL ELSE 5");
                                                                    }
                                                                } else {
                                                                    info!(target: "server_log", "COAL ELSE 4");
                                                                }
                                                            } else {
                                                                info!(target: "server_log", "COAL ELSE 3");
                                                            }
                                                        } else {
                                                            info!(target: "server_log", "COAL ELSE 2");
                                                        }
                                                    } else {
                                                        info!(target: "server_log", "COAL ELSE 1");
                                                    }

                                                    if let Some(meta) =
                                                        txn_result_ore.transaction.meta.clone()
                                                    {
                                                        if let OptionSerializer::Some(
                                                            log_messages,
                                                        ) = meta.log_messages
                                                        {
                                                            if let Some(return_log) =
                                                                log_messages.iter().find(|log| {
                                                                    log.starts_with(&format!(
                                                                        "Program return: {} ",
                                                                        ore_api::ID
                                                                    ))
                                                                })
                                                            {
                                                                if let Some(return_data) =
                                                                    return_log.strip_prefix(
                                                                        &format!(
                                                                            "Program return: {} ",
                                                                            ore_api::ID
                                                                        ),
                                                                    )
                                                                {
                                                                    if let Ok(return_data) =
                                                                        return_data.from_base64()
                                                                    {
                                                                        ore_cloned_data =
                                                                            return_data.clone();
                                                                        ore_mine_event =
                                                                        bytemuck::try_from_bytes::<
                                                                            ore_api::event::MineEvent,
                                                                        >(
                                                                            &ore_cloned_data
                                                                        );
                                                                        info!(target: "server_log", "ORE MineEvent: {:?}", ore_mine_event);
                                                                        info!(target: "submission_log", "ORE MineEvent: {:?}", ore_mine_event);
                                                                    } else {
                                                                        info!(target: "server_log", "ORE ELSE 5");
                                                                    }
                                                                } else {
                                                                    info!(target: "server_log", "ORE ELSE 4");
                                                                }
                                                            } else {
                                                                info!(target: "server_log", "ORE ELSE 3");
                                                            }
                                                        } else {
                                                            info!(target: "server_log", "ORE ELSE 2");
                                                        }
                                                    } else {
                                                        info!(target: "server_log", "ORE ELSE 1");
                                                    }

                                                    let mut ore_balance_after_tx = 0;

                                                    let mut coal_balance_after_tx = 0;

                                                    tokio::time::sleep(Duration::from_millis(3000))
                                                        .await;

                                                    if let Ok(ore_balance) = get_ore_balance(
                                                        app_wallet.clone().miner_wallet.pubkey(),
                                                        &rpc_client.clone(),
                                                    )
                                                    .await
                                                    {
                                                        ore_balance_after_tx = ore_balance.clone();
                                                        if (ore_balance_after_tx == std::u64::MAX
                                                            || ore_balance_after_tx <= 0)
                                                        {
                                                            ore_balance_after_tx = 0;
                                                            ore_balance_before_tx = 0;
                                                        }
                                                        info!(target: "server_log", "ORE balance before: {:?} ORE balance after: {:?}", ore_balance_before_tx, ore_balance_after_tx);
                                                    } else {
                                                        ore_balance_before_tx = 0;
                                                    }

                                                    if (ore_balance_before_tx <= 0
                                                        || ore_balance_after_tx <= 0)
                                                    {
                                                        ore_balance_after_tx = 0;
                                                        ore_balance_before_tx = 0;
                                                    }

                                                    if ore_balance_before_tx > ore_balance_after_tx
                                                    {
                                                        ore_balance_before_tx =
                                                            ore_balance_after_tx.clone();
                                                    }
                                                    info!(target: "server_log", "2 ORE balance before: {:?} ORE balance after: {:?}", ore_balance_before_tx, ore_balance_after_tx);

                                                    if let Ok(coal_balance) = get_coal_balance(
                                                        app_wallet.clone().miner_wallet.pubkey(),
                                                        &rpc_client.clone(),
                                                    )
                                                    .await
                                                    {
                                                        coal_balance_after_tx =
                                                            coal_balance.clone();
                                                        info!(target: "server_log", "1 Coal balance before: {:?} Coal balance after: {:?}", coal_balance_before_tx, coal_balance_after_tx);
                                                        if (coal_balance_after_tx == std::u64::MAX
                                                            || coal_balance_after_tx <= 0)
                                                        {
                                                            coal_balance_after_tx = 0;
                                                            coal_balance_before_tx = 0;
                                                        }
                                                    } else {
                                                        coal_balance_before_tx = 0;
                                                    }

                                                    if (coal_balance_before_tx <= 0
                                                        || coal_balance_after_tx <= 0)
                                                    {
                                                        coal_balance_after_tx = 0;
                                                        coal_balance_before_tx = 0;
                                                    }
                                                    if coal_balance_before_tx
                                                        > coal_balance_after_tx
                                                    {
                                                        coal_balance_before_tx =
                                                            coal_balance_after_tx.clone();
                                                    }
                                                    info!(target: "server_log", "2 Coal balance before: {:?} Coal balance after: {:?}", coal_balance_before_tx, coal_balance_after_tx);

                                                    let balance_coal = (coal_balance_after_tx)
                                                        as f64
                                                        / 10f64.powf(COAL_TOKEN_DECIMALS as f64);

                                                    let balance_ore = (ore_balance_after_tx) as f64
                                                        / 10f64.powf(ORE_TOKEN_DECIMALS as f64);

                                                    let mut stake_multiplier_coal =
                                                        if let Some(config) = loaded_config_coal {
                                                            if config.top_balance > 0 {
                                                                1.0 + (coal_balance_after_tx as f64
                                                                    / config.top_balance as f64)
                                                                    .min(1.0f64)
                                                            } else {
                                                                1.0f64
                                                            }
                                                        } else {
                                                            1.0f64
                                                        };

                                                    if (stake_multiplier_coal < 1.0) {
                                                        stake_multiplier_coal = 1.0f64;
                                                    }
                                                    if (tool_multiplier < 1.0) {
                                                        tool_multiplier = 1.0f64;
                                                    }
                                                    if (guild_multiplier < 1.0) {
                                                        guild_multiplier = 1.0f64;
                                                    }

                                                    info!(target: "server_log", "For Challenge: {:?}", BASE64_STANDARD.encode(old_proof.challenge));
                                                    info!(target: "submission_log", "For Challenge: {:?}", BASE64_STANDARD.encode(old_proof.challenge));
                                                    let mut full_multiplier_coal =
                                                        stake_multiplier_coal
                                                            * tool_multiplier
                                                            * guild_multiplier;
                                                    if (full_multiplier_coal < 1.0) {
                                                        full_multiplier_coal = 1.0f64;
                                                    }
                                                    info!(target: "submission_log", "stake_multiplier_coal {:?}", stake_multiplier_coal);
                                                    info!(target: "submission_log", "tool_multiplier {:?}", stake_multiplier_coal);
                                                    info!(target: "submission_log", "guild_multiplier {:?}", guild_multiplier);
                                                    info!(target: "submission_log", "full_multiplier_coal {:?}", full_multiplier_coal);
                                                    let mut full_rewards_coal =
                                                        coal_balance_after_tx
                                                            - coal_balance_before_tx;
                                                    if (full_rewards_coal > 780_000_000_000) {
                                                        info!(target: "server_log", "HIT MAX COAL: {}", full_rewards_coal);
                                                        full_rewards_coal = 780_000_000_000;
                                                    }
                                                    let commissions_coal = full_rewards_coal
                                                        .mul(5)
                                                        .saturating_div(100);
                                                    let guild_stake_rewards_coal =
                                                        ((((full_rewards_coal - commissions_coal)
                                                            as f64
                                                            / full_multiplier_coal)
                                                            .mul(guild_multiplier)
                                                            as u64)
                                                            .mul(50))
                                                        .saturating_div(100);
                                                    // let stakers_rewards_coal = ((((full_rewards_coal - commissions_coal) as f64 / full_multiplier_coal).mul(stake_multiplier_coal) as u64).mul(10)).saturating_div(100);
                                                    let rewards_coal = full_rewards_coal
                                                        - commissions_coal
                                                        - guild_stake_rewards_coal;
                                                    let mut full_rewards_ore = ore_balance_after_tx
                                                        - ore_balance_before_tx;
                                                    if (full_rewards_ore > 100_000_000_000) {
                                                        info!(target: "server_log", "HIT MAX ORE: {}", full_rewards_ore);
                                                        full_rewards_ore = 100_000_000_000;
                                                    }
                                                    let commissions_ore =
                                                        full_rewards_ore.mul(5).saturating_div(100);
                                                    let rewards_ore =
                                                        full_rewards_ore - commissions_ore;
                                                    info!(target: "server_log", "Miners Rewards COAL: {}", rewards_coal);
                                                    info!(target: "server_log", "Commission COAL: {}", commissions_coal);
                                                    info!(target: "server_log", "Guild Staker Rewards COAL: {}", guild_stake_rewards_coal);
                                                    info!(target: "server_log", "Guild total stake: {}", guild_total_stake);
                                                    info!(target: "server_log", "Guild multiplier: {}", guild_multiplier);
                                                    info!(target: "server_log", "Tool multiplier: {}", tool_multiplier);
                                                    info!(target: "server_log", "Stake multiplier: {}", stake_multiplier_coal);
                                                    info!(target: "server_log", "Miners Rewards ORE: {}", rewards_ore);
                                                    info!(target: "server_log", "Commission ORE: {}", commissions_ore);

                                                    // handle sending mine success message
                                                    let mut total_real_hashpower: u64 = 0;
                                                    let mut total_hashpower: u64 = 0;
                                                    for submission in submissions.iter() {
                                                        total_hashpower += submission.1.hashpower;
                                                        total_real_hashpower +=
                                                            submission.1.real_hashpower;
                                                    }
                                                    let challenge;
                                                    loop {
                                                        if let Ok(c) = app_database
                                                            .get_challenge_by_challenge(
                                                                old_proof.challenge.to_vec(),
                                                            )
                                                            .await
                                                        {
                                                            challenge = c;
                                                            break;
                                                        } else {
                                                            error!(target: "server_log",
                                                                "Failed to get challenge by challenge! Inserting if necessary..."
                                                            );
                                                            let new_challenge = InsertChallenge {
                                                                pool_id: app_config.pool_id,
                                                                challenge: old_proof
                                                                    .challenge
                                                                    .to_vec(),
                                                                rewards_earned_coal: None,
                                                                rewards_earned_ore: None,
                                                            };
                                                            while let Err(_) = app_database
                                                                .add_new_challenge(
                                                                    new_challenge.clone(),
                                                                )
                                                                .await
                                                            {
                                                                error!(target: "server_log", "Failed to add new challenge to db.");
                                                                info!(target: "server_log", "Verifying challenge does not already exist.");
                                                                if let Ok(_) = app_database
                                                                    .get_challenge_by_challenge(
                                                                        new_challenge
                                                                            .challenge
                                                                            .clone(),
                                                                    )
                                                                    .await
                                                                {
                                                                    info!(target: "server_log", "Challenge already exists, continuing");
                                                                    break;
                                                                }

                                                                tokio::time::sleep(
                                                                    Duration::from_millis(1000),
                                                                )
                                                                .await;
                                                            }
                                                            info!(target: "server_log", "New challenge successfully added to db");
                                                            tokio::time::sleep(
                                                                Duration::from_millis(1000),
                                                            )
                                                            .await;
                                                        }
                                                    }

                                                    // Insert commissions earning
                                                    let commissions_earning = vec![InsertEarning {
                                                        miner_id: app_config.commissions_miner_id,
                                                        pool_id: app_config.pool_id,
                                                        challenge_id: challenge.id,
                                                        amount_coal: commissions_coal,
                                                        amount_ore: commissions_ore,
                                                        difficulty: 0,
                                                    }];
                                                    tracing::info!(target: "server_log", "Inserting commissions earning");
                                                    while let Err(_) = app_database
                                                        .add_new_earnings_batch(
                                                            commissions_earning.clone(),
                                                        )
                                                        .await
                                                    {
                                                        error!(target: "server_log", "Failed to add commmissions earning... retrying...");
                                                        tokio::time::sleep(Duration::from_millis(
                                                            500,
                                                        ))
                                                        .await;
                                                    }
                                                    tracing::info!(target: "server_log", "Inserted commissions earning");

                                                    let new_commission_rewards =
                                                        vec![UpdateReward {
                                                            miner_id: app_config
                                                                .commissions_miner_id,
                                                            balance_coal: commissions_coal,
                                                            balance_ore: commissions_ore,
                                                            balance_chromium: 0,
                                                            balance_ingot: 0,
                                                            balance_sol: 0,
                                                            balance_wood: 0,
                                                        }];

                                                    tracing::info!(target: "server_log", "Updating commissions rewards...");
                                                    while let Err(_) = app_database
                                                        .update_rewards(
                                                            new_commission_rewards.clone(),
                                                        )
                                                        .await
                                                    {
                                                        error!(target: "server_log", "Failed to update commission rewards in db. Retrying...");
                                                        tokio::time::sleep(Duration::from_millis(
                                                            500,
                                                        ))
                                                        .await;
                                                    }
                                                    tracing::info!(target: "server_log", "Updated commissions rewards");

                                                    info!(target: "server_log", "Sending internal mine success for challenge: {}", BASE64_STANDARD.encode(old_proof.challenge));
                                                    let _ = mine_success_sender.send(
                                                        MessageInternalMineSuccess {
                                                            difficulty,
                                                            total_balance_coal: balance_coal,
                                                            total_balance_ore: balance_ore,
                                                            rewards_coal: full_rewards_coal,
                                                            rewards_ore: full_rewards_ore,
                                                            commissions_coal: commissions_coal,
                                                            commissions_ore: commissions_ore,
                                                            challenge_id: challenge.id,
                                                            challenge: old_proof.challenge,
                                                            best_nonce: u64::from_le_bytes(
                                                                best_solution.n,
                                                            ),
                                                            total_hashpower,
                                                            total_real_hashpower,
                                                            coal_config: loaded_config_coal,
                                                            multiplier: stake_multiplier_coal,
                                                            submissions,
                                                            guild_total_stake,
                                                            guild_multiplier,
                                                            tool_multiplier,
                                                            guild_stake_rewards_coal,
                                                            guild_members: pool_guild_members,
                                                        },
                                                    );
                                                    tokio::time::sleep(Duration::from_millis(200))
                                                        .await;

                                                    break;
                                                }
                                                Err(e) => {
                                                    error!(target: "server_log", "Failed to get confirmed transaction... Come on rpc... {:?}",e);
                                                    tokio::time::sleep(Duration::from_millis(2000))
                                                        .await;
                                                }
                                            }
                                        }
                                    });
                                };
                            }

                            break;
                        }
                    } else {
                        error!(target: "server_log", "Solution is_some but got none on best hash re-check?");
                        tokio::time::sleep(Duration::from_millis(1_000)).await;
                    }
                }
                if !success {
                    info!(target: "server_log", "Failed to send tx. Discarding and refreshing data.");
                    // reset nonce
                    {
                        let mut nonce = app_nonce.lock().await;
                        *nonce = 0;
                    }
                    // reset client nonce ranges
                    {
                        let mut writer = app_client_nonce_ranges.write().await;
                        *writer = HashMap::new();
                        drop(writer);
                    }
                    // reset epoch hashes
                    {
                        info!(target: "server_log", "reset epoch hashes");
                        let mut mut_epoch_hashes = app_epoch_hashes.write().await;
                        mut_epoch_hashes.best_hash.solution = None;
                        mut_epoch_hashes.best_hash.difficulty = 0;
                        mut_epoch_hashes.submissions = HashMap::new();
                    }
                    // Open submission window
                    info!(target: "server_log", "openning submission window.");
                    let mut writer = app_submission_window.write().await;
                    writer.closed = false;
                    drop(writer);
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            } else {
                // error!(target: "server_log", "No best solution yet.");
                tokio::time::sleep(Duration::from_millis(1000)).await;
            }
        } else {
            tokio::time::sleep(Duration::from_millis(1000)).await;
        };
    }
}

async fn check_final_bundle_status(
    jito_client: &JitoJsonRpcSDK,
    bundle_uuid: &str,
) -> Result<BundleStatus, ()> {
    let max_retries = 60;
    let retry_delay = Duration::from_secs(1);

    for attempt in 1..=max_retries {
        info!(target: "server_log",
            "Checking final bundle status (attempt {}/{})",
            attempt, max_retries
        );

        let status_response = match jito_client
            .get_bundle_statuses(vec![bundle_uuid.to_string()])
            .await
        {
            Ok(response) => Ok(response),
            Err(e) => Err(()),
        };

        if (!status_response.is_err()) {
            let bundle_status = match get_bundle_status(&status_response.unwrap()) {
                Ok(response) => Ok(response),
                Err(e) => {
                    error!(target: "server_log","Error parsing bundle status: {:?}", e);
                    Err(())
                }
            };

            if (!bundle_status.is_err()) {
                let bundle_status = bundle_status.unwrap();
                match bundle_status.confirmation_status.as_deref() {
                    Some("confirmed") => {
                        info!(target: "server_log","Bundle confirmed on-chain. Waiting for finalization...");
                        check_transaction_error(&bundle_status).unwrap();
                    }
                    Some("finalized") => {
                        info!(target: "server_log","Bundle finalized on-chain successfully!");
                        check_transaction_error(&bundle_status).unwrap();
                        print_transaction_url(&bundle_status);
                        return Ok(bundle_status);
                    }
                    Some(status) => {
                        error!(target: "server_log",
                            "Unexpected final bundle status: {}. Continuing to poll...",
                            status
                        );
                    }
                    None => {
                        error!(target: "server_log","Unable to parse final bundle status. Continuing to poll...");
                    }
                }
            }
        }

        if attempt < max_retries {
            tokio::time::sleep(retry_delay).await;
        }
    }
    Err(())
}

fn get_bundle_status(status_response: &serde_json::Value) -> Result<BundleStatus, ()> {
    status_response
        .get("result")
        .and_then(|result| result.get("value"))
        .and_then(|value| value.as_array())
        .and_then(|statuses| statuses.get(0))
        .ok_or_else(|| ())
        .map(|bundle_status| BundleStatus {
            confirmation_status: bundle_status
                .get("confirmation_status")
                .and_then(|s| s.as_str())
                .map(String::from),
            err: bundle_status.get("err").cloned(),
            transactions: bundle_status
                .get("transactions")
                .and_then(|t| t.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                }),
        })
}

fn check_transaction_error(bundle_status: &BundleStatus) -> Result<(), ()> {
    if let Some(err) = &bundle_status.err {
        if err["Ok"].is_null() {
            info!(target: "server_log","Transaction executed without errors.");
            Ok(())
        } else {
            error!(target: "server_log","Transaction encountered an error: {:?}", err);
            Err(())
        }
    } else {
        Ok(())
    }
}

fn print_transaction_url(bundle_status: &BundleStatus) {
    if let Some(transactions) = &bundle_status.transactions {
        for tx_id in transactions {
            info!(target: "server_log","Transaction ID: {}", tx_id);
            info!(target: "server_log","Transaction URL: https://solscan.io/tx/{}", tx_id);
        }
    } else {
        error!(target: "server_log","No transactions found in the bundle status.");
    }
}
