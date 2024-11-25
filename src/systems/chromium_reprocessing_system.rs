use crate::app_database::AppDatabase;
use crate::coal_utils::{get_reprocessor, get_resource_mint, Resource, Tip};
use crate::models::ExtraResourcesGeneration;
use crate::send_and_confirm::{send_and_confirm, ComputeBudget};
use crate::{Config, WalletExtension};
use chrono::{DateTime, NaiveDateTime, Utc};
use solana_client::client_error::reqwest;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::signer::Signer;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::info;

pub async fn chromium_reprocessing_system(
    app_wallet: Arc<WalletExtension>,
    rpc_client: Arc<RpcClient>,
    jito_client: Arc<RpcClient>,
    app_database: Arc<AppDatabase>,
    config: Arc<Config>,
) {
    loop {
        let mut last_reprocess: Option<ExtraResourcesGeneration> = None;
        match app_database.get_last_chromium_reprocessing().await {
            Ok(db_last_reprocess) => {
                info!(target: "server_log", "CHROMIUM: Last reprocessing was {}", db_last_reprocess.created_at);
                last_reprocess = Some(db_last_reprocess);
            }
            Err(e) => {
                tracing::error!(target: "server_log", "CHROMIUM: Failed to get last reprocessing {}", e);
            }
        }

        let three_days_ago = Utc::now() - Duration::from_secs(60 * 60 * 24 * 3);

        // check if the last reprocess was more than 3 days ago, if not wait for 6 hours and retry the reprocess
        if !last_reprocess.is_none()
            && last_reprocess.clone().unwrap().created_at > three_days_ago.naive_local()
        {
            info!(target: "server_log", "CHROMIUM: Last reprocessing was less than 3 days ago, waiting then retrying");
            tokio::time::sleep(Duration::from_secs(20)).await;
            continue;
        }
        if !last_reprocess.is_none() && !last_reprocess.clone().unwrap().finished_at {
            info!(target: "server_log", "CHROMIUM: Last reprocessing was not finished, waiting then retrying");
            tokio::time::sleep(Duration::from_secs(20)).await;
            continue;
        }

        while let Err(_) = app_database.add_chromium_reprocessing(config.pool_id).await {
            tracing::error!(target: "server_log", "Failed to add chromium reprocessing to db. Retrying...");
            tokio::time::sleep(Duration::from_secs(1)).await;
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

        // TODO split rewards to user

        let token_account = rpc_client
            .get_token_account(&token_account_pubkey)
            .await
            .unwrap();
        let final_balance = token_account.unwrap().token_amount.ui_amount.unwrap();
        let mut reprocessed_amount = final_balance - initial_balance;
        if reprocessed_amount <= 0.0 {
            tracing::error!(target: "server_log", "CHROMIUM: Chromium reprocessing system: Got 0 reprocessed amount");
            reprocessed_amount = 0.0;
        }
        info!(target: "server_log", "CHROMIUM: Reprocessed {} Chromium", reprocessed_amount);

        while let Err(_) = app_database
            .finish_chromium_reprocessing(reprocessed_amount as u64)
            .await
        {
            tracing::error!(target: "server_log", "Failed to finish chromium reprocessing to db. Retrying...");
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
}
