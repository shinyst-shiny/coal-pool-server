use axum::extract::ws::Message;
use base64::{prelude::BASE64_STANDARD, Engine};
use futures::SinkExt;
use solana_sdk::signer::Signer;
use std::str::FromStr;
use std::{ops::Div, sync::Arc, time::Duration};
use steel::Pubkey;
use tokio::{
    sync::{mpsc::UnboundedReceiver, RwLock},
    time::Instant,
};
use tracing::info;

use crate::message::{CoalDetails, MinerDetails, OreBoost, OreDetails, RewardDetails};
use crate::ore_utils::ORE_TOKEN_DECIMALS;
use crate::{
    app_database::AppDatabase, coal_utils::COAL_TOKEN_DECIMALS,
    message::ServerMessagePoolSubmissionResult, AppState, ClientVersion, Config, InsertEarning,
    InsertSubmission, MessageInternalMineSuccess, UpdateReward, WalletExtension,
};

pub async fn pool_mine_success_system(
    app_shared_state: Arc<RwLock<AppState>>,
    app_database: Arc<AppDatabase>,
    app_config: Arc<Config>,
    app_wallet: Arc<WalletExtension>,
    mut mine_success_receiver: UnboundedReceiver<MessageInternalMineSuccess>,
) {
    let guild_pubkey = Pubkey::from_str(&app_config.guild_address).unwrap();
    info!(target: "server_log", "guild_pubkey {}",guild_pubkey);
    loop {
        while let Some(msg) = mine_success_receiver.recv().await {
            let id = uuid::Uuid::new_v4();
            let c = BASE64_STANDARD.encode(msg.challenge);
            info!(target: "server_log", "{} - Processing internal mine success for challenge: {}", id, c);
            {
                let instant = Instant::now();
                info!(target: "server_log", "{} - Getting sockets.", id);
                let shared_state = app_shared_state.read().await;
                let len = shared_state.sockets.len();
                let socks = shared_state.sockets.clone();
                drop(shared_state);
                info!(target: "server_log", "{} - Got sockets in {}.", id, instant.elapsed().as_millis());

                let mut i_earnings = Vec::new();
                let mut i_rewards = Vec::new();
                let mut i_submissions = Vec::new();

                let instant = Instant::now();
                info!(target: "server_log", "{} - Processing submission results for challenge: {}.", id, c);
                let total_rewards_ore = msg.rewards_ore - msg.commissions_ore;
                let total_rewards_coal = msg.rewards_coal - msg.commissions_coal;
                for (miner_pubkey, msg_submission) in msg.submissions.iter() {
                    let miner_rewards = app_database
                        .get_miner_rewards(miner_pubkey.to_string())
                        .await
                        .unwrap();

                    let hashpower_percent = (msg_submission.hashpower as u128)
                        .saturating_mul(1_000_000)
                        .saturating_div(msg.total_hashpower as u128);

                    let real_hashpower_percent = (msg_submission.real_hashpower as u128)
                        .saturating_mul(1_000_000)
                        .saturating_div(msg.total_real_hashpower as u128);

                    let decimals_coal = 10f64.powf(COAL_TOKEN_DECIMALS as f64);
                    let earned_rewards_coal = hashpower_percent
                        .saturating_mul(total_rewards_coal as u128)
                        .saturating_div(1_000_000)
                        as u64;

                    let decimals_ore = 10f64.powf(ORE_TOKEN_DECIMALS as f64);
                    let earned_rewards_ore = real_hashpower_percent
                        .saturating_mul(total_rewards_ore as u128)
                        .saturating_div(1_000_000)
                        as u64;

                    let new_earning = InsertEarning {
                        miner_id: msg_submission.miner_id,
                        pool_id: app_config.pool_id,
                        challenge_id: msg.challenge_id,
                        amount_coal: earned_rewards_coal,
                        amount_ore: earned_rewards_ore,
                    };

                    let new_submission = InsertSubmission {
                        miner_id: msg_submission.miner_id,
                        challenge_id: msg.challenge_id,
                        nonce: msg_submission.supplied_nonce,
                        difficulty: msg_submission.supplied_diff as i8,
                    };

                    let new_reward = UpdateReward {
                        miner_id: msg_submission.miner_id,
                        balance_coal: earned_rewards_coal,
                        balance_ore: earned_rewards_ore,
                        balance_chromium: 0,
                    };

                    i_earnings.push(new_earning);
                    i_rewards.push(new_reward);
                    i_submissions.push(new_submission);
                    //let _ = app_database.add_new_earning(new_earning).await.unwrap();

                    let earned_rewards_dec_coal = (earned_rewards_coal as f64).div(decimals_coal);
                    let pool_rewards_dec_coal = (msg.rewards_coal as f64).div(decimals_coal);

                    let percentage_coal = if pool_rewards_dec_coal != 0.0 {
                        (earned_rewards_dec_coal / pool_rewards_dec_coal) * 100.0
                    } else {
                        0.0 // Handle the case where pool_rewards_dec is 0 to avoid division by zero
                    };

                    let earned_rewards_dec_ore = (earned_rewards_ore as f64).div(decimals_ore);
                    let pool_rewards_dec_ore = (msg.rewards_ore as f64).div(decimals_ore);

                    let percentage_ore = if pool_rewards_dec_ore != 0.0 {
                        (earned_rewards_dec_ore / pool_rewards_dec_ore) * 100.0
                    } else {
                        0.0 // Handle the case where pool_rewards_dec is 0 to avoid division by zero
                    };

                    let top_stake_coal = if let Some(config) = msg.coal_config {
                        (config.top_balance as f64).div(decimals_coal)
                    } else {
                        1.0f64
                    };

                    for (_addr, client_connection) in socks.iter() {
                        if client_connection.pubkey.eq(&miner_pubkey) {
                            let socket_sender = client_connection.socket.clone();

                            match client_connection.client_version {
                                ClientVersion::V2 => {
                                    let coal_details = CoalDetails {
                                        reward_details: RewardDetails {
                                            total_balance: msg.total_balance_coal,
                                            miner_earned_rewards: earned_rewards_dec_coal,
                                            miner_percentage: percentage_coal,
                                            total_rewards: pool_rewards_dec_coal,
                                            miner_supplied_difficulty: msg_submission.supplied_diff
                                                as u32,
                                        },
                                        tool_multiplier: msg.tool_multiplier,
                                        guild_total_stake: msg.guild_total_stake,
                                        guild_multiplier: msg.guild_multiplier,
                                        top_stake: top_stake_coal,
                                        stake_multiplier: msg.multiplier,
                                    };

                                    let ore_details = OreDetails {
                                        reward_details: RewardDetails {
                                            total_balance: msg.total_balance_ore,
                                            miner_earned_rewards: earned_rewards_dec_ore,
                                            miner_percentage: percentage_ore,
                                            total_rewards: pool_rewards_dec_ore,
                                            miner_supplied_difficulty: msg_submission.supplied_diff
                                                as u32,
                                        },
                                        top_stake: 0.0,
                                        stake_multiplier: 0.0,
                                        ore_boosts: Vec::from([
                                            OreBoost {
                                                stake_multiplier: 0.0,
                                                top_stake: 0.0,
                                                total_stake: 0.0,
                                                name: "".to_string(),
                                                mint_address: [0; 32],
                                            },
                                            OreBoost {
                                                stake_multiplier: 0.0,
                                                top_stake: 0.0,
                                                total_stake: 0.0,
                                                name: "".to_string(),
                                                mint_address: [0; 32],
                                            },
                                            OreBoost {
                                                stake_multiplier: 0.0,
                                                top_stake: 0.0,
                                                total_stake: 0.0,
                                                name: "".to_string(),
                                                mint_address: [0; 32],
                                            },
                                        ]),
                                    };

                                    let miner_details = MinerDetails {
                                        total_coal: miner_rewards.balance_coal as f64,
                                        total_ore: miner_rewards.balance_ore as f64,
                                        total_chromium: miner_rewards.balance_chromium as f64,
                                        guild_address: guild_pubkey.to_bytes(),
                                        miner_address: miner_pubkey.to_bytes(),
                                    };

                                    let server_message = ServerMessagePoolSubmissionResult {
                                        difficulty: msg.difficulty,
                                        challenge: msg.challenge,
                                        best_nonce: msg.best_nonce,
                                        active_miners: len as u32,
                                        coal_details,
                                        ore_details,
                                        miner_details,
                                    };
                                    let mut message = vec![1u8];
                                    message.extend_from_slice(&server_message.to_binary());
                                    tokio::spawn(async move {
                                        if let Ok(_) = socket_sender
                                            .lock()
                                            .await
                                            .send(Message::Binary(message))
                                            .await
                                        {
                                        } else {
                                            tracing::error!(target: "server_log", "Failed to send client pool submission result binary message");
                                        }
                                    });
                                }
                            }
                        }
                    }
                }

                info!(target: "server_log", "{} - Finished processing submission results in {}ms for challenge: {}.", id, instant.elapsed().as_millis(), c);

                let instant = Instant::now();
                info!(target: "server_log", "{} - Adding earnings", id);
                let batch_size = 200;
                if i_earnings.len() > 0 {
                    for batch in i_earnings.chunks(batch_size) {
                        while let Err(_) = app_database.add_new_earnings_batch(batch.to_vec()).await
                        {
                            tracing::error!(target: "server_log", "{} - Failed to add new earnings batch to db. Retrying...", id);
                            tokio::time::sleep(Duration::from_millis(500)).await;
                        }
                        tokio::time::sleep(Duration::from_millis(200)).await;
                    }
                    info!(target: "server_log", "{} - Successfully added earnings batch", id);
                }
                info!(target: "server_log", "{} - Added earnings in {}ms", id, instant.elapsed().as_millis());

                tokio::time::sleep(Duration::from_millis(500)).await;

                let instant = Instant::now();
                info!(target: "server_log", "{} - Updating rewards", id);
                if i_rewards.len() > 0 {
                    let mut batch_num = 1;
                    for batch in i_rewards.chunks(batch_size) {
                        let instant = Instant::now();
                        info!(target: "server_log", "{} - Updating reward batch {}", id, batch_num);
                        while let Err(_) = app_database.update_rewards(batch.to_vec()).await {
                            tracing::error!(target: "server_log", "{} - Failed to update rewards in db. Retrying...", id);
                            tokio::time::sleep(Duration::from_millis(500)).await;
                        }
                        info!(target: "server_log", "{} - Updated reward batch {} in {}ms", id, batch_num, instant.elapsed().as_millis());
                        batch_num += 1;
                        tokio::time::sleep(Duration::from_millis(200)).await;
                    }
                    info!(target: "server_log", "{} - Successfully updated rewards", id);
                }
                info!(target: "server_log", "{} - Updated rewards in {}ms", id, instant.elapsed().as_millis());

                tokio::time::sleep(Duration::from_millis(500)).await;

                let instant = Instant::now();
                info!(target: "server_log", "{} - Adding submissions", id);
                if i_submissions.len() > 0 {
                    for batch in i_submissions.chunks(batch_size) {
                        info!(target: "server_log", "{} - Submissions batch size: {}", id, i_submissions.len());
                        while let Err(_) =
                            app_database.add_new_submissions_batch(batch.to_vec()).await
                        {
                            tracing::error!(target: "server_log", "{} - Failed to add new submissions batch. Retrying...", id);
                            tokio::time::sleep(Duration::from_millis(500)).await;
                        }
                        tokio::time::sleep(Duration::from_millis(200)).await;
                    }

                    info!(target: "server_log", "{} - Successfully added submissions batch", id);
                }
                info!(target: "server_log", "{} - Added submissions in {}ms", id, instant.elapsed().as_millis());

                tokio::time::sleep(Duration::from_millis(500)).await;

                let instant = Instant::now();
                info!(target: "server_log", "{} - Updating pool rewards", id);
                while let Err(_) = app_database
                    .update_pool_rewards(
                        app_wallet.miner_wallet.pubkey().to_string(),
                        msg.rewards_coal,
                        msg.rewards_ore,
                        0,
                    )
                    .await
                {
                    tracing::error!(target: "server_log",
                        "{} - Failed to update pool rewards! Retrying...", id
                    );
                    tokio::time::sleep(Duration::from_millis(1000)).await;
                }
                info!(target: "server_log", "{} - Updated pool rewards in {}ms", id, instant.elapsed().as_millis());

                tokio::time::sleep(Duration::from_millis(200)).await;

                let instant = Instant::now();
                info!(target: "server_log", "{} - Updating challenge rewards", id);
                if let Ok(s) = app_database
                    .get_submission_id_with_nonce(msg.best_nonce)
                    .await
                {
                    if let Err(_) = app_database
                        .update_challenge_rewards(
                            msg.challenge.to_vec(),
                            s,
                            msg.rewards_coal,
                            msg.rewards_ore,
                        )
                        .await
                    {
                        tracing::error!(target: "server_log", "{} - Failed to update challenge rewards! Skipping! Devs check!", id);
                        let err_str = format!("{} - Challenge UPDATE FAILED - Challenge: {:?}\nSubmission ID: {}\nRewards COAL: {}\nRewards ORE: {}\n", id, msg.challenge.to_vec(), s, msg.rewards_coal, msg.rewards_ore);
                        tracing::error!(target: "server_log", err_str);
                    }
                    info!(target: "server_log", "{} - Updated challenge rewards in {}ms", id, instant.elapsed().as_millis());
                } else {
                    tracing::error!(target: "server_log", "{} - Failed to get submission id with nonce: {} for challenge_id: {}", id, msg.best_nonce, msg.challenge_id);
                    tracing::error!(target: "server_log", "{} - Failed update challenge rewards!", id);
                    let mut found_best_nonce = false;
                    for submission in i_submissions {
                        if submission.nonce == msg.best_nonce {
                            found_best_nonce = true;
                            break;
                        }
                    }

                    if found_best_nonce {
                        info!(target: "server_log", "{} - Found best nonce in i_submissions", id);
                    } else {
                        info!(target: "server_log", "{} - Failed to find best nonce in i_submissions", id);
                    }
                }
                info!(target: "server_log", "{} - Finished processing internal mine success for challenge: {}", id, c);
            }
        }
    }
}
