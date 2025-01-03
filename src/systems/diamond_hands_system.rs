use crate::app_database::AppDatabase;
use crate::app_rr_database::AppRRDatabase;
use crate::{Config, WalletExtension};
use solana_client::nonblocking::rpc_client::RpcClient;
use std::sync::Arc;

struct MinerStats {
    submission_count: u64,
    total_hash_power: u64,
}

pub async fn diamond_hands_system(
    app_wallet: Arc<WalletExtension>,
    rpc_client: Arc<RpcClient>,
    jito_client: Arc<RpcClient>,
    app_database: Arc<AppDatabase>,
    config: Arc<Config>,
    app_rr_database: Arc<AppRRDatabase>,
) {
    /*tokio::time::sleep(Duration::from_millis(10000)).await;
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

        let seven_days_ago = Utc::now() - Duration::from_secs(60 * 60 * 24 * 7);

        // check if the last reprocess was more than 3 days ago, if not wait for 6 hours and retry the reprocess
        if last_reprocess.is_some()
            && last_reprocess.unwrap().created_at > seven_days_ago.naive_utc()
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

        let commissions_chromium = full_reprocessed_amount.mul(5).saturating_div(100);

        // Insert commissions earning
        let commissions_earning = vec![InsertEarningExtraResources {
            miner_id: config.commissions_miner_id,
            pool_id: config.pool_id,
            extra_resources_generation_id: current_reprocessing.id,
            amount_chromium: commissions_chromium,
            amount_sol: 0,
            amount_coal: 0,
            amount_ingot: 0,
            amount_ore: 0,
            amount_wood: 0,
            generation_type: ExtraResourcesGenerationType::ChromiumReprocess as i32,
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
            balance_chromium: commissions_chromium,
            balance_coal: 0,
            balance_ore: 0,
            balance_sol: 0,
            balance_ingot: 0,
            balance_wood: 0,
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

        let reprocessed_amount = full_reprocessed_amount - commissions_chromium;

        let last_reprocess_time = match app_rr_database
            .get_last_reprocessing(
                config.pool_id,
                ExtraResourcesGenerationType::ChromiumReprocess,
            )
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

            let miner_factor_perc = miner_submission_perc.mul(miner_hash_power_perc);

            miner_stats_perc.insert(miner_id, miner_factor_perc);
            total_miners_perc += miner_factor_perc
        }

        info!(target: "server_log", "DIAMOND HANDS: Total Miners: {}, total miner perc: {}", miner_stats_perc.len(), total_miners_perc);

        let mut miners_earnings: Vec<InsertEarningExtraResources> = Vec::new();
        let mut miners_rewards: Vec<UpdateReward> = Vec::new();

        for (miner_id, miner_factor_perc) in miner_stats_perc {
            let chromium_earned =
                miner_factor_perc.mul(reprocessed_amount as f64) / total_miners_perc;
            println!(
                "DIAMOND HANDS Miner {}: earned: {}",
                miner_id, chromium_earned
            );
            miners_earnings.push(InsertEarningExtraResources {
                miner_id,
                pool_id: config.pool_id,
                extra_resources_generation_id: current_reprocessing.id,
                amount_chromium: chromium_earned.floor() as u64,
                amount_sol: 0,
                amount_coal: 0,
                amount_ingot: 0,
                amount_ore: 0,
                amount_wood: 0,
                generation_type: ExtraResourcesGenerationType::ChromiumReprocess as i32,
            });
            miners_rewards.push(UpdateReward {
                miner_id,
                balance_chromium: chromium_earned.floor() as u64,
                balance_coal: 0,
                balance_ore: 0,
                balance_ingot: 0,
                balance_sol: 0,
                balance_wood: 0,
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
            .update_pool_rewards(
                app_wallet.miner_wallet.pubkey().to_string(),
                0,
                0,
                0,
                full_reprocessed_amount,
                0,
                0,
            )
            .await
        {
            tracing::error!(target: "server_log",
                "Failed to update pool rewards! Retrying..."
            );
        }
        info!(target: "server_log", "Updated pool rewards");

        while let Err(_) = app_database
            .finish_extra_resources_generation(
                current_reprocessing.id,
                0,
                0,
                0,
                full_reprocessed_amount,
                0,
                0,
                ExtraResourcesGenerationType::ChromiumReprocess,
            )
            .await
        {
            tracing::error!(target: "server_log", "DIAMOND HANDS: Failed to finish chromium reprocessing to db. Retrying...");
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }*/
}
