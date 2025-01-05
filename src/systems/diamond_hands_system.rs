use crate::app_database::{AppDatabase, AppDatabaseError};
use crate::app_rr_database::AppRRDatabase;
use crate::coal_utils::{get_reprocessor, get_resource_mint, Resource, Tip, COAL_TOKEN_DECIMALS};
use crate::models::{
    Earning, ExtraResourcesGeneration, ExtraResourcesGenerationType, InsertEarningExtraResources,
    LastClaim, Submission, UpdateReward,
};
use crate::send_and_confirm::{send_and_confirm, ComputeBudget};
use crate::{models, Config, WalletExtension, MIN_DIFF, MIN_HASHPOWER};
use chrono::{DateTime, NaiveDateTime, Utc};
use solana_client::client_error::reqwest;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::signer::Signer;
use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::ops::Mul;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::info;

#[derive(Debug)]
struct RewardsData {
    amount_sol: u64,
    amount_coal: u64,
    amount_ore: u64,
    amount_chromium: u64,
    amount_wood: u64,
    amount_ingot: u64,
}

struct MinerStats {
    submission_count: u64,
    total_hash_power: u64,
}

pub async fn diamond_hands_system(
    config: Arc<Config>,
    app_database: Arc<AppDatabase>,
    app_rr_database: Arc<AppRRDatabase>,
) {
    tokio::time::sleep(Duration::from_millis(10000)).await;
    loop {
        let mut last_reprocess: Option<ExtraResourcesGeneration> = None;
        match app_rr_database
            .get_last_reprocessing(
                config.pool_id,
                ExtraResourcesGenerationType::DiamondHandsReprocess,
            )
            .await
        {
            Ok(db_last_reprocess) => {
                info!(target: "server_log", "DIAMOND HANDS: Last reprocessing was {}", db_last_reprocess.created_at);
                last_reprocess = Some(db_last_reprocess);
            }
            Err(e) => {
                tracing::error!(target: "server_log", "DIAMOND HANDS: Failed to get last reprocessing {:?}", e);
            }
        }

        let rewards_duration = Duration::from_secs(60 * 60 * 24 * 7);
        let rewards_date = Utc::now() - rewards_duration;

        // check if the last reprocess was more than 3 days ago, if not wait for 6 hours and retry the reprocess
        if last_reprocess.is_some() && last_reprocess.unwrap().created_at > rewards_date.naive_utc()
        {
            info!(target: "server_log", "DIAMOND HANDS: Last reprocessing was less than 7 days ago, waiting then retrying");
            tokio::time::sleep(Duration::from_secs(25)).await;
            continue;
        }

        let mut check_current_reprocessing: Option<ExtraResourcesGeneration> = None;

        match app_database
            .get_pending_extra_resources_generation(
                config.pool_id,
                ExtraResourcesGenerationType::DiamondHandsReprocess,
            )
            .await
        {
            Ok(current_reprocessing) => {
                check_current_reprocessing = Some(current_reprocessing);
            }
            Err(_) => {}
        }

        if check_current_reprocessing.is_some() {
            info!(target: "server_log", "DIAMOND HANDS: Last reprocessing was not finished, waiting then retrying");
            tokio::time::sleep(Duration::from_secs(20)).await;
            continue;
        } else {
            while let Err(_) = app_database
                .add_extra_resources_generation(
                    config.pool_id,
                    ExtraResourcesGenerationType::DiamondHandsReprocess,
                    None,
                )
                .await
            {
                tracing::error!(target: "server_log", "Failed to add chromium reprocessing to db. Retrying...");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }

        let mut current_reprocessing: ExtraResourcesGeneration;

        loop {
            match app_database
                .get_pending_extra_resources_generation(
                    config.pool_id,
                    ExtraResourcesGenerationType::DiamondHandsReprocess,
                )
                .await
            {
                Ok(resp) => {
                    current_reprocessing = resp;
                    break;
                }
                Err(_) => {
                    tracing::error!(target: "server_log", "DIAMOND HANDS: Failed to get current reprocessing. Retrying...");
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
        info!(target: "server_log", "DIAMOND HANDS: Starting reprocessing system");

        let total_rewards =
            get_diamond_hands_rewards(&app_rr_database, &config, rewards_duration).await;

        let commissions_diamond_hands = RewardsData {
            amount_sol: total_rewards.amount_sol.mul(5).saturating_div(100),
            amount_coal: total_rewards.amount_coal.mul(5).saturating_div(100),
            amount_ore: total_rewards.amount_ore.mul(5).saturating_div(100),
            amount_chromium: total_rewards.amount_chromium.mul(5).saturating_div(100),
            amount_wood: total_rewards.amount_wood.mul(5).saturating_div(100),
            amount_ingot: total_rewards.amount_ingot.mul(5).saturating_div(100),
        };

        // Insert commissions earning
        let commissions_earning = vec![InsertEarningExtraResources {
            miner_id: config.commissions_miner_id,
            pool_id: config.pool_id,
            extra_resources_generation_id: current_reprocessing.id,
            amount_sol: commissions_diamond_hands.amount_sol,
            amount_coal: commissions_diamond_hands.amount_coal,
            amount_ore: commissions_diamond_hands.amount_ore,
            amount_chromium: commissions_diamond_hands.amount_chromium,
            amount_ingot: commissions_diamond_hands.amount_ingot,
            amount_wood: commissions_diamond_hands.amount_wood,
            generation_type: ExtraResourcesGenerationType::DiamondHandsReprocess as i32,
        }];

        tracing::info!(target: "server_log", "DIAMOND HANDS: Inserting commissions earning");
        while let Err(_) = app_database
            .add_new_earnings_extra_resources_batch(commissions_earning.clone())
            .await
        {
            tracing::error!(target: "server_log", "DIAMOND HANDS: Failed to add commissions earning... retrying...");
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        tracing::info!(target: "server_log", "DIAMOND HANDS: Inserted commissions earning");

        tokio::time::sleep(Duration::from_millis(500)).await;

        let commission_rewards = vec![UpdateReward {
            miner_id: config.commissions_miner_id,
            balance_sol: commissions_diamond_hands.amount_sol,
            balance_coal: commissions_diamond_hands.amount_coal,
            balance_ore: commissions_diamond_hands.amount_ore,
            balance_chromium: commissions_diamond_hands.amount_chromium,
            balance_ingot: commissions_diamond_hands.amount_ingot,
            balance_wood: commissions_diamond_hands.amount_wood,
        }];

        if commission_rewards.len() > 0 {
            for batch in commission_rewards.chunks(100) {
                while let Err(_) = app_database.update_rewards(batch.to_vec()).await {
                    tracing::error!(target: "server_log", "DIAMOND HANDS: Failed to add new commissions rewards to db. Retrying...");
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }

        tracing::info!(target: "server_log", "DIAMOND HANDS: Inserted commissions rewards");

        let miners_reprocessed_rewards = RewardsData {
            amount_sol: total_rewards.amount_sol - commissions_diamond_hands.amount_sol,
            amount_coal: total_rewards.amount_coal - commissions_diamond_hands.amount_coal,
            amount_ore: total_rewards.amount_ore - commissions_diamond_hands.amount_ore,
            amount_chromium: total_rewards.amount_chromium
                - commissions_diamond_hands.amount_chromium,
            amount_wood: total_rewards.amount_wood - commissions_diamond_hands.amount_wood,
            amount_ingot: total_rewards.amount_ingot - commissions_diamond_hands.amount_ingot,
        };

        let last_reprocess_time = match app_rr_database
            .get_last_reprocessing(
                config.pool_id,
                ExtraResourcesGenerationType::DiamondHandsReprocess,
            )
            .await
        {
            Ok(db_last_reprocess) => db_last_reprocess.created_at,
            Err(_) => (Utc::now() - Duration::from_secs(60 * 60 * 24 * 7)).naive_utc(), // default to last week
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
                    tracing::error!(target: "server_log", "DIAMOND HANDS: Failed to get user submissions. Retrying...");
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

            let mut no_withdraw_bonus = 0.0;

            let last_claim = app_database.get_last_claim(miner_id).await.ok();

            if let Some(claim) = last_claim {
                // loop to check if the last claim is more than 1, 2, 3 or 4 weeks ago
                for i in 1..=4 {
                    let last_claim_date =
                        claim.created_at + Duration::from_secs(60 * 60 * 24 * (i * 7));
                    info!(target: "server_log", "DIAMOND HANDS: Miner: {}, checking: {} - {}", miner_id, i, last_claim_date);
                    if Utc::now().naive_utc() > last_claim_date {
                        // Check if there is one submission in the last_claim_date week, if so give the bonus
                        let start_submission_check =
                            (Utc::now() - Duration::from_secs(60 * 60 * 24 * (i * 7))).naive_utc();
                        let end_submission_check = (Utc::now()
                            - Duration::from_secs(60 * 60 * 24 * ((i - 1) * 7)))
                        .naive_utc();
                        let submissions_in_range = app_database
                            .get_submissions_in_range(start_submission_check, end_submission_check)
                            .await
                            .unwrap_or(Vec::new());
                        if submissions_in_range.len() > 0 {
                            no_withdraw_bonus += 1.0;
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
            } else {
                no_withdraw_bonus = 4.0;
            }
            // Calculate miner factor percentage
            // miner_submission_perc * miner_hash_power_perc * no_withdraw_bonus = miner_factor_perc
            let miner_factor_perc = miner_submission_perc
                .mul(miner_hash_power_perc)
                .mul(no_withdraw_bonus);

            miner_stats_perc.insert(miner_id, miner_factor_perc);
            total_miners_perc += miner_factor_perc
        }

        info!(target: "server_log", "DIAMOND HANDS: Total Miners: {}, total miner perc: {}", miner_stats_perc.len(), total_miners_perc);

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

            println!(
                "DIAMOND HANDS Miner {}: earned: {:?}",
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
                generation_type: ExtraResourcesGenerationType::DiamondHandsReprocess as i32,
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
                    tracing::error!(target: "server_log", "DIAMOND HANDS: Failed to add new earnings batch to db. Retrying...");
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            info!(target: "server_log", "DIAMOND HANDS: Successfully added earnings batch");
        }

        tokio::time::sleep(Duration::from_millis(500)).await;

        if miners_rewards.len() > 0 {
            for batch in miners_rewards.chunks(batch_size) {
                while let Err(_) = app_database.update_rewards(batch.to_vec()).await {
                    tracing::error!(target: "server_log", "DIAMOND HANDS: Failed to add new rewards batch to db. Retrying...");
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            info!(target: "server_log", "DIAMOND HANDS: Successfully added rewards batch");
        }

        while let Err(_) = app_database
            .finish_extra_resources_generation(
                current_reprocessing.id,
                total_rewards.amount_sol,
                total_rewards.amount_coal,
                total_rewards.amount_ore,
                total_rewards.amount_chromium,
                total_rewards.amount_wood,
                total_rewards.amount_ingot,
                ExtraResourcesGenerationType::DiamondHandsReprocess,
            )
            .await
        {
            tracing::error!(target: "server_log", "DIAMOND HANDS: Failed to finish chromium reprocessing to db. Retrying...");
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
}

async fn get_diamond_hands_rewards(
    app_rr_database: &Arc<AppRRDatabase>,
    config: &Arc<Config>,
    rewards_duration: Duration,
) -> RewardsData {
    // get the earnings from the commission wallet from now to now - rewards_duration
    let commission_wallet_pubkey = config.commissions_pubkey.clone();
    let current_time = Utc::now().naive_utc();
    let start_time = current_time - rewards_duration;

    let mut commission_earnings: Earning;
    loop {
        match app_rr_database
            .get_earning_in_period_by_pubkey(
                commission_wallet_pubkey.clone(),
                start_time,
                current_time,
            )
            .await
        {
            Ok(resp) => {
                commission_earnings = resp;
                break;
            }
            Err(_) => {
                tracing::error!(target: "server_log", "DIAMOND HANDS: Failed to get commission earnings. Retrying...");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }

    // 0.25 : 5 = x : 100
    // 25 : 500 = x : 100

    let pool_commission = 5 * 100;
    let percentage_to_distribute = (0.25f64 * 100.0f64) as u64;
    let percentage_from_commission = percentage_to_distribute * 100 / pool_commission;

    // get how much to distribute in COAL and ORE to diamond hands
    let coal_to_distribute = commission_earnings
        .amount_coal
        .saturating_mul(percentage_from_commission)
        .saturating_div(10000);

    let ore_to_distribute = commission_earnings
        .amount_ore
        .saturating_mul(percentage_from_commission)
        .saturating_div(10000);

    return RewardsData {
        amount_sol: 0,
        amount_coal: coal_to_distribute,
        amount_ore: ore_to_distribute,
        amount_chromium: 0,
        amount_wood: 0,
        amount_ingot: 0,
    };
}
