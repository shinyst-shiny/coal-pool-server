use crate::app_database::{AppDatabase, AppDatabaseError};
use crate::coal_utils::{get_reprocessor, get_resource_mint, Resource, Tip};
use crate::models::{
    ExtraResourcesGeneration, InsertEarningExtraResources, Submission, UpdateReward,
};
use crate::send_and_confirm::{send_and_confirm, ComputeBudget};
use crate::{models, Config, WalletExtension, MIN_DIFF, MIN_HASHPOWER};
use chrono::{DateTime, NaiveDateTime, Utc};
use solana_client::client_error::reqwest;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::signer::Signer;
use std::collections::{HashMap, HashSet};
use std::ops::Mul;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::info;

struct MinerStats {
    submission_count: u64,
    total_hash_power: u64,
}

pub async fn chromium_reprocessing_system(
    app_wallet: Arc<WalletExtension>,
    rpc_client: Arc<RpcClient>,
    jito_client: Arc<RpcClient>,
    app_database: Arc<AppDatabase>,
    config: Arc<Config>,
) {
    tokio::time::sleep(Duration::from_millis(10000)).await;
    loop {
        let mut last_reprocess: Option<ExtraResourcesGeneration> = None;
        match app_database
            .get_last_chromium_reprocessing(config.pool_id)
            .await
        {
            Ok(db_last_reprocess) => {
                info!(target: "server_log", "CHROMIUM: Last reprocessing was {}", db_last_reprocess.created_at);
                last_reprocess = Some(db_last_reprocess);
            }
            Err(e) => {
                tracing::error!(target: "server_log", "CHROMIUM: Failed to get last reprocessing {:?}", e);
            }
        }

        let three_days_ago = Utc::now() - Duration::from_secs(60 * 60 * 24 * 3);

        // check if the last reprocess was more than 3 days ago, if not wait for 6 hours and retry the reprocess
        if last_reprocess.is_some()
            && last_reprocess.unwrap().created_at > three_days_ago.naive_utc()
        {
            info!(target: "server_log", "CHROMIUM: Last reprocessing was less than 3 days ago, waiting then retrying");
            tokio::time::sleep(Duration::from_secs(20)).await;
            continue;
        }

        let mut check_current_reprocessing: Option<ExtraResourcesGeneration> = None;

        match app_database
            .get_pending_chromium_reprocessing(config.pool_id)
            .await
        {
            Ok(current_reprocessing) => {
                check_current_reprocessing = Some(current_reprocessing);
            }
            Err(_) => {}
        }

        if check_current_reprocessing.is_some() {
            info!(target: "server_log", "CHROMIUM: Last reprocessing was not finished, waiting then retrying");
            tokio::time::sleep(Duration::from_secs(20)).await;
            continue;
        } else {
            while let Err(_) = app_database.add_chromium_reprocessing(config.pool_id).await {
                tracing::error!(target: "server_log", "Failed to add chromium reprocessing to db. Retrying...");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }

        let mut current_reprocessing: ExtraResourcesGeneration;

        loop {
            // get every submission from the last reprocess time to now. If there was no process before now get every submission
            match app_database
                .get_pending_chromium_reprocessing(config.pool_id)
                .await
            {
                Ok(resp) => {
                    current_reprocessing = resp;
                    break;
                }
                Err(_) => {
                    tracing::error!(target: "server_log", "CHROMIUM: Failed to get current reprocessing. Retrying...");
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }

        let signer = app_wallet.clone().miner_wallet.clone();
        let fee_payer = app_wallet.clone().fee_wallet.clone();
        info!(target: "server_log", "CHROMIUM: Starting reprocessing system");
        // Init ata

        let mint_address = get_resource_mint(&Resource::Chromium);

        // Build instructions.
        let token_account_pubkey = spl_associated_token_account::get_associated_token_address(
            &signer.pubkey(),
            &mint_address,
        );

        // Check if ata already exists
        if let Ok(Some(_ata)) = rpc_client.get_token_account(&token_account_pubkey).await {
            info!(target: "server_log", "CHROMIUM: Chromium ATA already exists");
        } else {
            info!(target: "server_log", "CHROMIUM: Generating Chromium ATA");
            // Sign and send transaction.
            let ix = spl_associated_token_account::instruction::create_associated_token_account(
                &signer.pubkey(),
                &signer.pubkey(),
                &mint_address,
                &spl_token::id(),
            );
            match send_and_confirm(
                &[ix],
                ComputeBudget::Fixed(400_000),
                &rpc_client.clone(),
                &jito_client.clone(),
                &signer.clone(),
                &fee_payer.clone(),
                None,
                None,
            )
            .await
            {
                Ok(_) => {
                    info!(target: "server_log", "CHROMIUM: Chromium ATA created");
                }
                Err(e) => {
                    tracing::error!(target: "server_log", "CHROMIUM: Exiting Chromium reprocessing system: Failed to create Chromium ATA {}", e);
                    continue;
                }
            }
        }

        tokio::time::sleep(Duration::from_millis(20000)).await;

        // Build instructions.
        let token_account_pubkey = spl_associated_token_account::get_associated_token_address(
            &signer.pubkey(),
            &mint_address,
        );

        let token_account = rpc_client
            .get_token_account(&token_account_pubkey)
            .await
            .unwrap();
        let initial_balance = token_account.unwrap().token_amount.ui_amount.unwrap();

        let mut reprocessor = get_reprocessor(&rpc_client, &signer.pubkey()).await;

        while reprocessor.is_none() {
            info!(target: "server_log", "CHROMIUM: Reprocessor not found. Creating it...");
            let reprocessor_creation_ix = coal_api::instruction::init_reprocess(signer.pubkey());
            send_and_confirm(
                &[reprocessor_creation_ix],
                ComputeBudget::Fixed(100_000),
                &rpc_client.clone(),
                &jito_client.clone(),
                &signer.clone(),
                &fee_payer.clone(),
                None,
                None,
            )
            .await
            .ok();
            tokio::time::sleep(Duration::from_millis(5000)).await;
            reprocessor = get_reprocessor(&rpc_client, &signer.pubkey()).await;
        }
        info!(target: "server_log", "CHROMIUM: Reprocessor found, moving with next steps");

        let target_slot = reprocessor.unwrap().slot;

        info!(target: "server_log", "CHROMIUM: Waiting for slot {}...", target_slot);

        let url = "https://bundles.jito.wtf/api/v1/bundles/tip_floor";
        let mut tip = 0_u64;

        // fetch the url data using a GET
        let client = reqwest::Client::new();
        let response = client
            .get(url)
            .header("Content-Type", "application/json")
            .send()
            .await;

        let response = match response {
            Ok(r) => r.text(),
            Err(e) => {
                tracing::error!(target: "server_log", "CHROMIUM: Exiting Chromium reprocessing system: Failed to fetch tip floor {}", e);
                continue;
            }
        };

        if let Ok(tips) = serde_json::from_str::<Vec<Tip>>(&response.await.unwrap()) {
            for item in tips {
                tip = (item.landed_tips_50th_percentile * (10_f64).powf(9.0)) as u64;
            }
        } else {
            tracing::error!(target: "server_log", "CHROMIUM: Exiting Chromium reprocessing system: Failed to deserialize tip floor");
        }

        if tip == 0 {
            tracing::error!(target: "server_log", "CHROMIUM: Exiting Chromium reprocessing system: Got 0 tips");
            continue;
        }

        loop {
            match rpc_client.get_slot().await {
                Ok(current_slot) => {
                    if current_slot >= target_slot - 1 {
                        info!(target: "server_log", "CHROMIUM: Target slot {} reached", target_slot);
                        let ix = coal_api::instruction::reprocess(signer.pubkey());
                        send_and_confirm(
                            &[ix],
                            ComputeBudget::Fixed(200_000),
                            &rpc_client.clone(),
                            &jito_client.clone(),
                            &signer.clone(),
                            &fee_payer.clone(),
                            Some(10_000),
                            Some(tip),
                        )
                        .await
                        .ok();
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(400)).await;
                }
                Err(e) => {
                    tracing::error!(target: "server_log", "CHROMIUM: Failed to get current slot {}...", e);
                    tokio::time::sleep(Duration::from_secs(400)).await;
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(25000)).await;

        let token_account = rpc_client
            .get_token_account(&token_account_pubkey)
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(25000)).await;
        let final_balance = token_account.unwrap().token_amount.ui_amount.unwrap();
        tokio::time::sleep(Duration::from_millis(25000)).await;
        tracing::error!(target: "server_log", "CHROMIUM: Waiting a bit more for good measure");
        tokio::time::sleep(Duration::from_millis(25000)).await;
        let mut full_reprocessed_amount = (final_balance - initial_balance) as u64;
        if full_reprocessed_amount <= 0 {
            tracing::error!(target: "server_log", "CHROMIUM: Chromium reprocessing system: Got 0 reprocessed amount");
            full_reprocessed_amount = 0;
        }
        info!(target: "server_log", "CHROMIUM: Reprocessed {} Chromium", full_reprocessed_amount);

        let commissions_chromium = full_reprocessed_amount.mul(5).saturating_div(100);

        // Insert commissions earning
        let commissions_earning = vec![InsertEarningExtraResources {
            miner_id: config.commissions_miner_id,
            pool_id: config.pool_id,
            extra_resources_generation_id: current_reprocessing.id,
            amount_chromium: commissions_chromium,
        }];
        tracing::info!(target: "server_log", "CHROMIUM: Inserting commissions earning");
        while let Err(_) = app_database
            .add_new_earnings_extra_resources_batch(commissions_earning.clone())
            .await
        {
            tracing::error!(target: "server_log", "CHROMIUM: Failed to add commmissions earning... retrying...");
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        tracing::info!(target: "server_log", "CHROMIUM: Inserted commissions earning");

        let reprocessed_amount = full_reprocessed_amount - commissions_chromium;

        let last_reprocess_time = match app_database
            .get_last_chromium_reprocessing(config.pool_id)
            .await
        {
            Ok(db_last_reprocess) => db_last_reprocess.created_at,
            Err(_) => (Utc::now() - Duration::from_secs(60 * 60 * 24 * 365)).naive_utc(), // default to last year
        };
        let current_time = Utc::now().naive_utc();

        let submissions: Vec<Submission>;
        loop {
            // get every submission from the last reprocess time to now. If there was no process before now get every submission
            match app_database
                .get_submissions_in_range(last_reprocess_time, current_time)
                .await
            {
                Ok(resp) => {
                    submissions = resp;
                    break;
                }
                Err(_) => {
                    tracing::error!(target: "server_log", "CHROMIUM: Failed to get user submissions. Retrying...");
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }

        let total_mine_event = submissions
            .iter()
            .map(|submission| submission.challenge_id)
            .collect::<Vec<i32>>()
            .len();

        let mut total_hash_power: u64 = 0;

        let mut miner_stats: HashMap<i32, MinerStats> = HashMap::new();

        for submission in submissions {
            let entry = miner_stats
                .entry(submission.miner_id)
                .or_insert(MinerStats {
                    total_hash_power: 0,
                    submission_count: 0,
                });

            let mut hash_power = 0;
            // Calculate and add the hash power
            if submission.difficulty as u32 > MIN_DIFF {
                hash_power = MIN_HASHPOWER * 2u64.pow(submission.difficulty as u32 - MIN_DIFF);
            }
            total_hash_power += hash_power;
            entry.total_hash_power += hash_power;
            entry.submission_count += 1;
        }

        let mut total_miners_perc = 0.0;

        let mut miner_stats_perc: HashMap<i32, f64> = HashMap::new();

        for (miner_id, stats) in miner_stats {
            // total_mine_event : 1 = stats.submission_count : x
            let miner_submission_perc = stats.submission_count as f64 / total_mine_event as f64;
            // total_hash_power : 1 = stats.total_hash_power : x
            let miner_hash_power_perc = stats.total_hash_power as f64 / total_hash_power as f64;

            // info!(target: "server_log", "CHROMIUM: Miner: {}, set of data {}, {}, {}, {}", miner_id, stats.submission_count, total_mine_event, stats.total_hash_power, total_hash_power);
            // info!(target: "server_log", "CHROMIUM: Miner: {}, submission perc: {:.11}, hash power perc: {:.11}", miner_id, miner_submission_perc, miner_hash_power_perc);

            let miner_factor_perc = miner_submission_perc.mul(miner_hash_power_perc);

            miner_stats_perc.insert(miner_id, miner_factor_perc);
            total_miners_perc += miner_factor_perc
        }

        info!(target: "server_log", "CHROMIUM: Total Miners: {}, total miner perc: {}", miner_stats_perc.len(), total_miners_perc);

        let mut miners_earnings: Vec<InsertEarningExtraResources> = Vec::new();
        let mut miners_rewards: Vec<UpdateReward> = Vec::new();

        for (miner_id, miner_factor_perc) in miner_stats_perc {
            let chromium_earned =
                miner_factor_perc.mul(reprocessed_amount as f64) / total_miners_perc;
            println!("CHROMIUM Miner {}: earned: {}", miner_id, chromium_earned);
            miners_earnings.push(InsertEarningExtraResources {
                miner_id,
                pool_id: config.pool_id,
                extra_resources_generation_id: current_reprocessing.id,
                amount_chromium: chromium_earned.floor() as u64,
            });
            miners_rewards.push(UpdateReward {
                miner_id,
                balance_chromium: chromium_earned.floor() as u64,
                balance_coal: 0,
                balance_ore: 0,
            });
        }

        let batch_size = 200;
        if miners_earnings.len() > 0 {
            for batch in miners_earnings.chunks(batch_size) {
                while let Err(_) = app_database
                    .add_new_earnings_extra_resources_batch(batch.to_vec())
                    .await
                {
                    tracing::error!(target: "server_log", "CHROMIUM: Failed to add new earnings batch to db. Retrying...");
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            info!(target: "server_log", "CHROMIUM: Successfully added earnings batch");
        }

        tokio::time::sleep(Duration::from_millis(500)).await;

        if miners_rewards.len() > 0 {
            for batch in miners_rewards.chunks(batch_size) {
                while let Err(_) = app_database.update_rewards(batch.to_vec()).await {
                    tracing::error!(target: "server_log", "CHROMIUM: Failed to add new rewards batch to db. Retrying...");
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            info!(target: "server_log", "CHROMIUM: Successfully added rewards batch");
        }

        while let Err(_) = app_database
            .update_pool_rewards(
                app_wallet.miner_wallet.pubkey().to_string(),
                0,
                0,
                full_reprocessed_amount,
            )
            .await
        {
            tracing::error!(target: "server_log",
                "Failed to update pool rewards! Retrying..."
            );
        }
        info!(target: "server_log", "Updated pool rewards");

        while let Err(_) = app_database
            .finish_chromium_reprocessing(current_reprocessing.id, full_reprocessed_amount)
            .await
        {
            tracing::error!(target: "server_log", "CHROMIUM: Failed to finish chromium reprocessing to db. Retrying...");
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
}
