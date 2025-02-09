use crate::app_database::{AppDatabase, AppDatabaseError};
use crate::app_rr_database::AppRRDatabase;
use crate::coal_utils::{get_reprocessor, get_resource_mint, Resource, Tip, COAL_TOKEN_DECIMALS};
use crate::models::{
    Earning, ExtraResourcesGeneration, ExtraResourcesGenerationType, InsertEarningExtraResources,
    LastClaim, Miner, Submission, UpdateReward,
};
use crate::send_and_confirm::{send_and_confirm, ComputeBudget};
use crate::{
    models, Config, RewardsData, WalletExtension, DIAMOND_HANDS_DAYS, MIN_DIFF, MIN_HASHPOWER,
    NFT_DISTRIBUTION_DAYS,
};

use base64::engine::general_purpose;
use base64::Engine;
use chrono::{DateTime, NaiveDateTime, Utc};
use reqwest::Client;
use serde_json::json;
use solana_account_decoder::parse_token::UiTokenAccount;
use solana_account_decoder::UiAccountEncoding;
use solana_client::client_error::{reqwest, ClientError};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_client::rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig};
use solana_client::rpc_filter::{Memcmp, MemcmpEncodedBytes, RpcFilterType};
use solana_client::rpc_response::RpcTokenAccountBalance;
use solana_sdk::account_info::AccountInfo;
use solana_sdk::borsh1::try_from_slice_unchecked;
use solana_sdk::bs58;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::program_pack::Pack;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::Signer;
use spl_token::state::{Account, Mint};
use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::ops::Mul;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::info;

struct MinerStats {
    submission_count: u64,
    total_hash_power: u64,
}

pub async fn nft_distribution_system(
    app_wallet: Arc<WalletExtension>,
    config: Arc<Config>,
    app_database: Arc<AppDatabase>,
    app_rr_database: Arc<AppRRDatabase>,
    rpc_client: Arc<RpcClient>,
    nft_rewards_daily: RewardsData,
    nft_pubkey: Pubkey,
) {
    let nft_rewards: RewardsData = RewardsData {
        amount_sol: nft_rewards_daily.amount_sol * NFT_DISTRIBUTION_DAYS,
        amount_coal: nft_rewards_daily.amount_coal * NFT_DISTRIBUTION_DAYS,
        amount_ore: nft_rewards_daily.amount_ore * NFT_DISTRIBUTION_DAYS,
        amount_chromium: nft_rewards_daily.amount_chromium * NFT_DISTRIBUTION_DAYS,
        amount_wood: nft_rewards_daily.amount_wood * NFT_DISTRIBUTION_DAYS,
        amount_ingot: nft_rewards_daily.amount_ingot * NFT_DISTRIBUTION_DAYS,
    };
    let rewards_duration = Duration::from_secs(60 * 60 * 24 * NFT_DISTRIBUTION_DAYS);
    let current_time = Utc::now().naive_utc();
    let start_time = current_time - rewards_duration;

    info!(target: "reprocess_log", "NFT REPROCESSING: Starting reprocessing {} - {} for {} value {:?}", start_time, current_time, nft_pubkey,nft_rewards.clone() );

    tokio::time::sleep(Duration::from_millis(10000)).await;
    loop {
        let mut last_reprocess: Option<ExtraResourcesGeneration> = None;
        match app_rr_database
            .get_last_reprocessing(
                config.pool_id,
                ExtraResourcesGenerationType::NftReprocessOMC,
            )
            .await
        {
            Ok(db_last_reprocess) => {
                info!(target: "reprocess_log", "NFT REPROCESSING: Last reprocessing was {}", db_last_reprocess.created_at);
                last_reprocess = Some(db_last_reprocess);
            }
            Err(e) => {
                tracing::error!(target: "reprocess_log", "NFT REPROCESSING: Failed to get last reprocessing {:?}", e);
            }
        }

        let rewards_duration = Duration::from_secs(60 * 60 * 24 * 7);
        let rewards_date = Utc::now() - rewards_duration;

        // check if the last reprocess was more than 3 days ago, if not wait for 6 hours and retry the reprocess
        if last_reprocess.is_some() && last_reprocess.unwrap().created_at > rewards_date.naive_utc()
        {
            info!(target: "reprocess_log", "NFT REPROCESSING: Last reprocessing was less than 7 days ago, waiting then retrying");
            tokio::time::sleep(Duration::from_secs(25)).await;
            continue;
        }

        let mut check_current_reprocessing: Option<ExtraResourcesGeneration> = None;

        match app_database
            .get_pending_extra_resources_generation(
                config.pool_id,
                ExtraResourcesGenerationType::NftReprocessOMC,
            )
            .await
        {
            Ok(current_reprocessing) => {
                check_current_reprocessing = Some(current_reprocessing);
            }
            Err(_) => {}
        }

        if check_current_reprocessing.is_some() {
            info!(target: "reprocess_log", "NFT REPROCESSING: Last reprocessing was not finished, waiting then retrying");
            tokio::time::sleep(Duration::from_secs(20)).await;
            continue;
        } else {
            while let Err(_) = app_database
                .add_extra_resources_generation(
                    config.pool_id,
                    ExtraResourcesGenerationType::NftReprocessOMC,
                    None,
                )
                .await
            {
                tracing::error!(target: "reprocess_log", "Failed to add nft reprocessing to db. Retrying...");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }

        let mut current_reprocessing: ExtraResourcesGeneration;

        loop {
            match app_database
                .get_pending_extra_resources_generation(
                    config.pool_id,
                    ExtraResourcesGenerationType::NftReprocessOMC,
                )
                .await
            {
                Ok(resp) => {
                    current_reprocessing = resp;
                    break;
                }
                Err(_) => {
                    tracing::error!(target: "reprocess_log", "NFT REPROCESSING: Failed to get current reprocessing. Retrying...");
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
        info!(target: "reprocess_log", "NFT REPROCESSING: Starting reprocessing system");

        // Get the list of NFT holders
        let nft_holders = match get_nft_holders(&rpc_client.url(), &nft_pubkey).await {
            Ok(holders) => holders,
            Err(e) => {
                tracing::error!(target: "reprocess_log", "NFT REPROCESSING: Failed to get NFT holders: {:?}", e);
                return;
            }
        };

        info!(target: "reprocess_log", "NFT REPROCESSING: Found {} NFT holders", nft_holders.len());

        // get the list of miners from the database from the list of nft_holders. Mak a new hashmap of miners and the amount of nft they have
        let mut miners_and_nft: HashMap<i32, u64> = HashMap::new();
        for holder in nft_holders {
            match app_database
                .get_miner_by_pubkey_str(holder.0.to_string())
                .await
            {
                Ok(miner) => {
                    miners_and_nft.insert(miner.id, holder.1);
                }
                _ => {
                    info!(target: "reprocess_log", "NFT REPROCESSING: Skipping miner: {}", holder.0);
                }
            }
        }

        let total_rewards = nft_rewards.clone();

        let commissions_nft_rewards = RewardsData {
            amount_sol: total_rewards
                .amount_sol
                .mul(config.commission_amount as u64)
                .saturating_div(100),
            amount_coal: total_rewards
                .amount_coal
                .mul(config.commission_amount as u64)
                .saturating_div(100),
            amount_ore: total_rewards
                .amount_ore
                .mul(config.commission_amount as u64)
                .saturating_div(100),
            amount_chromium: total_rewards
                .amount_chromium
                .mul(config.commission_amount as u64)
                .saturating_div(100),
            amount_wood: total_rewards
                .amount_wood
                .mul(config.commission_amount as u64)
                .saturating_div(100),
            amount_ingot: total_rewards
                .amount_ingot
                .mul(config.commission_amount as u64)
                .saturating_div(100),
        };

        // Insert commissions earning
        let commissions_earning = vec![InsertEarningExtraResources {
            miner_id: config.commissions_miner_id,
            pool_id: config.pool_id,
            extra_resources_generation_id: current_reprocessing.id,
            amount_sol: commissions_nft_rewards.amount_sol,
            amount_coal: commissions_nft_rewards.amount_coal,
            amount_ore: commissions_nft_rewards.amount_ore,
            amount_chromium: commissions_nft_rewards.amount_chromium,
            amount_ingot: commissions_nft_rewards.amount_ingot,
            amount_wood: commissions_nft_rewards.amount_wood,
            generation_type: ExtraResourcesGenerationType::NftReprocessOMC as i32,
        }];

        tracing::info!(target: "reprocess_log", "NFT REPROCESSING: Inserting commissions earning");
        while let Err(_) = app_database
            .add_new_earnings_extra_resources_batch(commissions_earning.clone())
            .await
        {
            tracing::error!(target: "reprocess_log", "NFT REPROCESSING: Failed to add commissions earning... retrying...");
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        tracing::info!(target: "reprocess_log", "NFT REPROCESSING: Inserted commissions earning");

        tokio::time::sleep(Duration::from_millis(500)).await;

        let commission_rewards = vec![UpdateReward {
            miner_id: config.commissions_miner_id,
            balance_sol: commissions_nft_rewards.amount_sol,
            balance_coal: commissions_nft_rewards.amount_coal,
            balance_ore: commissions_nft_rewards.amount_ore,
            balance_chromium: commissions_nft_rewards.amount_chromium,
            balance_ingot: commissions_nft_rewards.amount_ingot,
            balance_wood: commissions_nft_rewards.amount_wood,
        }];

        if commission_rewards.len() > 0 {
            for batch in commission_rewards.chunks(100) {
                while let Err(_) = app_database.update_rewards(batch.to_vec()).await {
                    tracing::error!(target: "reprocess_log", "NFT REPROCESSING: Failed to add new commissions rewards to db. Retrying...");
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }

        tracing::info!(target: "reprocess_log", "NFT REPROCESSING: Inserted commissions rewards");

        let miners_reprocessed_rewards = RewardsData {
            amount_sol: total_rewards.amount_sol - commissions_nft_rewards.amount_sol,
            amount_coal: total_rewards.amount_coal - commissions_nft_rewards.amount_coal,
            amount_ore: total_rewards.amount_ore - commissions_nft_rewards.amount_ore,
            amount_chromium: total_rewards.amount_chromium
                - commissions_nft_rewards.amount_chromium,
            amount_wood: total_rewards.amount_wood - commissions_nft_rewards.amount_wood,
            amount_ingot: total_rewards.amount_ingot - commissions_nft_rewards.amount_ingot,
        };

        let last_reprocess_time = match app_rr_database
            .get_last_reprocessing(
                config.pool_id,
                ExtraResourcesGenerationType::NftReprocessOMC,
            )
            .await
        {
            Ok(db_last_reprocess) => db_last_reprocess.created_at,
            Err(_) => {
                (Utc::now() - Duration::from_secs(60 * 60 * 24 * NFT_DISTRIBUTION_DAYS)).naive_utc()
            } // default to last week
        };
        let current_time = Utc::now().naive_utc();

        let submissions_in_period: Vec<Submission>;
        loop {
            // get every submission from the last reprocess time to now. If there was no process before now get every submission
            match app_database
                .get_submissions_in_range(last_reprocess_time, current_time)
                .await
            {
                Ok(resp) => {
                    submissions_in_period = resp;
                    break;
                }
                Err(_) => {
                    tracing::error!(target: "reprocess_log", "NFT REPROCESSING: Failed to get user submissions. Retrying...");
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }

        let filtered_submissions: Vec<Submission> = submissions_in_period
            .into_iter()
            .filter(|submission| miners_and_nft.contains_key(&submission.miner_id))
            .collect();

        info!(target: "reprocess_log", "NFT REPROCESSING: Filtered {} submissions from NFT holders", filtered_submissions.len());

        let mut total_mine_event = 0;

        let mut total_hash_power: u64 = 0;

        let mut miner_stats: HashMap<i32, MinerStats> = HashMap::new();

        for submission in filtered_submissions {
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
            total_mine_event += 1;
        }

        let mut total_miners_perc = 0.0;

        let mut miner_stats_perc: HashMap<i32, f64> = HashMap::new();

        for (miner_id, stats) in miner_stats {
            // total_mine_event : 1 = stats.submission_count : x
            let miner_submission_perc = stats.submission_count as f64 / total_mine_event as f64;
            // total_hash_power : 1 = stats.total_hash_power : x
            let miner_hash_power_perc = stats.total_hash_power as f64 / total_hash_power as f64;

            // Calculate miner factor percentage
            // miner_submission_perc * miner_hash_power_perc * nft_count = miner_factor_perc
            let miner_factor_perc = miner_submission_perc
                .mul(miner_hash_power_perc)
                .mul(*miners_and_nft.get(&miner_id).unwrap_or(&0u64) as f64);

            miner_stats_perc.insert(miner_id, miner_factor_perc);
            total_miners_perc += miner_factor_perc
        }

        info!(target: "reprocess_log", "NFT REPROCESSING: Total Miners: {}, total miner perc: {}", miner_stats_perc.len(), total_miners_perc);

        let mut miners_earnings: Vec<InsertEarningExtraResources> = Vec::new();
        let mut miners_rewards: Vec<UpdateReward> = Vec::new();

        for (miner_id, miner_factor_perc) in miner_stats_perc {
            let miner_rewards = RewardsData {
                amount_sol: (miner_factor_perc.mul(miners_reprocessed_rewards.amount_sol as f64)
                    / total_miners_perc)
                    .floor() as u64,
                amount_coal: (miner_factor_perc.mul(miners_reprocessed_rewards.amount_coal as f64)
                    / total_miners_perc)
                    .floor() as u64,
                amount_ore: (miner_factor_perc.mul(miners_reprocessed_rewards.amount_ore as f64)
                    / total_miners_perc)
                    .floor() as u64,
                amount_chromium: (miner_factor_perc
                    .mul(miners_reprocessed_rewards.amount_chromium as f64)
                    / total_miners_perc)
                    .floor() as u64,
                amount_wood: (miner_factor_perc.mul(miners_reprocessed_rewards.amount_wood as f64)
                    / total_miners_perc)
                    .floor() as u64,
                amount_ingot: (miner_factor_perc
                    .mul(miners_reprocessed_rewards.amount_ingot as f64)
                    / total_miners_perc)
                    .floor() as u64,
            };

            info!(target: "reprocess_log",
                "NFT REPROCESSING Miner {}: earned: {:?}",
                miner_id, miner_rewards
            );
            miners_earnings.push(InsertEarningExtraResources {
                miner_id,
                pool_id: config.pool_id,
                extra_resources_generation_id: current_reprocessing.id,
                amount_sol: miner_rewards.amount_sol,
                amount_coal: miner_rewards.amount_coal,
                amount_ore: miner_rewards.amount_ore,
                amount_chromium: miner_rewards.amount_chromium,
                amount_ingot: miner_rewards.amount_ingot,
                amount_wood: miner_rewards.amount_wood,
                generation_type: ExtraResourcesGenerationType::NftReprocessOMC as i32,
            });
            miners_rewards.push(UpdateReward {
                miner_id,
                balance_sol: miner_rewards.amount_sol,
                balance_coal: miner_rewards.amount_coal,
                balance_ore: miner_rewards.amount_ore,
                balance_chromium: miner_rewards.amount_chromium,
                balance_ingot: miner_rewards.amount_ingot,
                balance_wood: miner_rewards.amount_wood,
            });
        }

        let batch_size = 200;
        if miners_earnings.len() > 0 {
            for batch in miners_earnings.chunks(batch_size) {
                while let Err(_) = app_database
                    .add_new_earnings_extra_resources_batch(batch.to_vec())
                    .await
                {
                    tracing::error!(target: "reprocess_log", "NFT REPROCESSING: Failed to add new earnings batch to db. Retrying...");
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            info!(target: "reprocess_log", "NFT REPROCESSING: Successfully added earnings batch");
        }

        tokio::time::sleep(Duration::from_millis(500)).await;

        if miners_rewards.len() > 0 {
            for batch in miners_rewards.chunks(batch_size) {
                while let Err(_) = app_database.update_rewards(batch.to_vec()).await {
                    tracing::error!(target: "reprocess_log", "NFT REPROCESSING: Failed to add new rewards batch to db. Retrying...");
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            info!(target: "reprocess_log", "NFT REPROCESSING: Successfully added rewards batch");
        }

        while let Err(_) = app_database
            .update_pool_rewards(
                app_wallet.miner_wallet.pubkey().to_string(),
                total_rewards.amount_sol,
                total_rewards.amount_coal,
                total_rewards.amount_ore,
                total_rewards.amount_chromium,
                total_rewards.amount_wood,
                total_rewards.amount_ingot,
            )
            .await
        {
            tracing::error!(target: "reprocess_log",
                "Failed to update pool rewards! Retrying..."
            );
        }
        info!(target: "reprocess_log", "Updated pool rewards");

        while let Err(_) = app_database
            .finish_extra_resources_generation(
                current_reprocessing.id,
                total_rewards.amount_sol,
                total_rewards.amount_coal,
                total_rewards.amount_ore,
                total_rewards.amount_chromium,
                total_rewards.amount_wood,
                total_rewards.amount_ingot,
                ExtraResourcesGenerationType::NftReprocessOMC,
            )
            .await
        {
            tracing::error!(target: "reprocess_log", "NFT REPROCESSING: Failed to finish reprocessing to db. Retrying...");
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
}

async fn get_nft_holders(
    rpc_url: &str,
    nft_mint: &Pubkey,
) -> Result<HashMap<Pubkey, u64>, Box<dyn std::error::Error>> {
    let mut holders = HashMap::new();
    let mut page = 1;
    let limit = 1000;
    let client = Client::new();

    info!(target: "reprocess_log", "NFT REPROCESSING: rpc_url: {}", rpc_url);

    loop {
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getAssetsByGroup",
            "params": {
                "groupKey": "collection",
                "groupValue": nft_mint.to_string(),
                "page": page,
                "limit": limit,
            }
        });

        let response: serde_json::Value = client
            .post(rpc_url)
            .json(&request)
            .send()
            .await?
            .json()
            .await?;

        let items = response["result"]["items"]
            .as_array()
            .ok_or("Invalid response format")?;

        if items.is_empty() {
            break;
        }

        for item in items {
            if let Some(owner) = item["ownership"]["owner"].as_str() {
                let owner_pubkey = Pubkey::from_str(owner)?;
                *holders.entry(owner_pubkey).or_insert(0) += 1;
            }
        }

        page += 1;
    }

    info!(target: "reprocess_log", "NFT REPROCESSING: Total holders found: {:?}", holders);

    Ok(holders)
}
