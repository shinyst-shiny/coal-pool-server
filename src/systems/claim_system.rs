use std::{sync::Arc, time::Duration};

use solana_client::{nonblocking::rpc_client::RpcClient, rpc_config::RpcSendTransactionConfig};
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction,
    signature::{Keypair, Signature},
    signer::Signer,
    transaction::Transaction,
};
use solana_transaction_status::TransactionConfirmationStatus;
use spl_associated_token_account::get_associated_token_address;
use tokio::time::Instant;
use tracing::{error, info};

use crate::coal_utils::get_chromium_mint;
use crate::ore_utils::{get_ore_mint, ORE_TOKEN_DECIMALS};
use crate::{
    app_database::AppDatabase,
    coal_utils::{get_coal_mint, COAL_TOKEN_DECIMALS},
    ClaimsQueue, InsertClaim, InsertTxn,
};

pub async fn claim_system(
    claims_queue: Arc<ClaimsQueue>,
    rpc_client: Arc<RpcClient>,
    wallet: Arc<Keypair>,
    app_database: Arc<AppDatabase>,
) {
    loop {
        let mut claim = None;
        let reader = claims_queue.queue.read().await;
        let item = reader.iter().next();
        if let Some(item) = item {
            claim = Some((item.0.clone(), item.1.clone()));
        }
        drop(reader);

        if let Some((miner_pubkey, claim_queue_item)) = claim {
            info!(target: "server_log", "Processing claim");
            let coal_mint = get_coal_mint();
            let ore_mint = get_ore_mint();
            let chromium_mint = get_chromium_mint();
            let receiver_pubkey = claim_queue_item.receiver_pubkey;
            let receiver_token_account_coal =
                get_associated_token_address(&receiver_pubkey, &coal_mint);
            let receiver_token_account_ore =
                get_associated_token_address(&receiver_pubkey, &ore_mint);
            let receiver_token_account_chromium =
                get_associated_token_address(&receiver_pubkey, &chromium_mint);

            let prio_fee: u32 = 10_000;

            let mut is_creating_ata_coal = false;
            let mut is_creating_ata_ore = false;
            let mut is_creating_ata_chromium = false;
            let mut ixs = Vec::new();
            let prio_fee_ix = ComputeBudgetInstruction::set_compute_unit_price(prio_fee as u64);
            ixs.push(prio_fee_ix);
            if let Ok(response) = rpc_client
                .get_token_account_balance(&receiver_token_account_coal)
                .await
            {
                if let Some(_amount) = response.ui_amount {
                    info!(target: "server_log", "miner has valid token account COAL.");
                } else {
                    info!(target: "server_log", "will create token account for miner COAL");
                    ixs.push(
                        spl_associated_token_account::instruction::create_associated_token_account(
                            &wallet.pubkey(),
                            &receiver_pubkey,
                            &coal_api::consts::COAL_MINT_ADDRESS,
                            &spl_token::id(),
                        ),
                    )
                }
            } else {
                info!(target: "server_log", "Adding create ata ix for miner claim COAL");
                is_creating_ata_coal = true;
                ixs.push(
                    spl_associated_token_account::instruction::create_associated_token_account(
                        &wallet.pubkey(),
                        &receiver_pubkey,
                        &coal_api::consts::COAL_MINT_ADDRESS,
                        &spl_token::id(),
                    ),
                )
            }

            if let Ok(response) = rpc_client
                .get_token_account_balance(&receiver_token_account_ore)
                .await
            {
                if let Some(_amount) = response.ui_amount {
                    info!(target: "server_log", "miner has valid token account ORE.");
                } else {
                    info!(target: "server_log", "will create token account for miner ORE");
                    ixs.push(
                        spl_associated_token_account::instruction::create_associated_token_account(
                            &wallet.pubkey(),
                            &receiver_pubkey,
                            &ore_api::consts::MINT_ADDRESS,
                            &spl_token::id(),
                        ),
                    )
                }
            } else {
                info!(target: "server_log", "Adding create ata ix for miner claim ORE");
                is_creating_ata_ore = true;
                ixs.push(
                    spl_associated_token_account::instruction::create_associated_token_account(
                        &wallet.pubkey(),
                        &receiver_pubkey,
                        &ore_api::consts::MINT_ADDRESS,
                        &spl_token::id(),
                    ),
                )
            }

            if let Ok(response) = rpc_client
                .get_token_account_balance(&receiver_token_account_chromium)
                .await
            {
                if let Some(_amount) = response.ui_amount {
                    info!(target: "server_log", "miner has valid token account CHROMIUM.");
                } else {
                    info!(target: "server_log", "will create token account for miner CHROMIUM");
                    ixs.push(
                        spl_associated_token_account::instruction::create_associated_token_account(
                            &wallet.pubkey(),
                            &receiver_pubkey,
                            &coal_api::consts::CHROMIUM_MINT_ADDRESS,
                            &spl_token::id(),
                        ),
                    )
                }
            } else {
                info!(target: "server_log", "Adding create ata ix for miner claim CHROMIUM");
                is_creating_ata_chromium = true;
                ixs.push(
                    spl_associated_token_account::instruction::create_associated_token_account(
                        &wallet.pubkey(),
                        &receiver_pubkey,
                        &coal_api::consts::CHROMIUM_MINT_ADDRESS,
                        &spl_token::id(),
                    ),
                )
            }

            let amount_coal = claim_queue_item.amount_coal;
            let amount_ore = claim_queue_item.amount_ore;
            let amount_chromium = claim_queue_item.amount_chromium;

            let mut claim_amount_coal = amount_coal;
            let mut claim_amount_ore = amount_ore;
            let claim_amount_chromium = amount_chromium;

            // 4 COAL or 0.02 ORE
            if is_creating_ata_coal {
                if claim_amount_coal >= 400_000_000_000 {
                    claim_amount_coal = claim_amount_coal - 400_000_000_000
                } else if claim_amount_ore >= 2_000_000_000 {
                    claim_amount_ore = claim_amount_ore - 2_000_000_000
                } else {
                    error!(target: "server_log", "miner has not enough COAL or ORE to claim.");
                    let mut writer = claims_queue.queue.write().await;
                    writer.remove(&miner_pubkey);
                    drop(writer);
                    continue;
                }
            }

            // 4 COAL or 0.02 ORE
            if is_creating_ata_ore {
                if claim_amount_ore >= 2_000_000_000 {
                    claim_amount_ore = claim_amount_ore - 2_000_000_000
                } else if claim_amount_coal >= 400_000_000_000 {
                    claim_amount_coal = claim_amount_coal - 400_000_000_000
                } else {
                    error!(target: "server_log", "miner has not enough COAL or ORE to claim.");
                    let mut writer = claims_queue.queue.write().await;
                    writer.remove(&miner_pubkey);
                    drop(writer);
                    continue;
                }
            }

            // 4 COAL or 0.02 ORE
            if is_creating_ata_chromium {
                if claim_amount_coal >= 400_000_000_000 {
                    claim_amount_coal = claim_amount_coal - 400_000_000_000
                } else if claim_amount_ore >= 2_000_000_000 {
                    claim_amount_ore = claim_amount_ore - 2_000_000_000
                } else {
                    error!(target: "server_log", "miner has not enough COAL or ORE to claim.");
                    let mut writer = claims_queue.queue.write().await;
                    writer.remove(&miner_pubkey);
                    drop(writer);
                    continue;
                }
            }

            if claim_amount_coal > 0 {
                let ix = crate::coal_utils::get_claim_ix(
                    wallet.pubkey(),
                    receiver_token_account_coal,
                    claim_amount_coal,
                );
                ixs.push(ix);
            }

            if claim_amount_ore > 0 {
                let ix = crate::ore_utils::get_claim_ix(
                    wallet.pubkey(),
                    receiver_token_account_ore,
                    claim_amount_ore,
                );
                ixs.push(ix);
            }

            if claim_amount_chromium > 0 {
                let ix = crate::coal_utils::get_claim_ix(
                    wallet.pubkey(),
                    receiver_token_account_chromium,
                    claim_amount_chromium,
                );
                ixs.push(ix);
            }

            if let Ok((hash, _slot)) = rpc_client
                .get_latest_blockhash_with_commitment(rpc_client.commitment())
                .await
            {
                let expired_timer = Instant::now();
                let mut tx = Transaction::new_with_payer(&ixs, Some(&wallet.pubkey()));

                tx.sign(&[&wallet], hash);

                let rpc_config = RpcSendTransactionConfig {
                    preflight_commitment: Some(rpc_client.commitment().commitment),
                    ..RpcSendTransactionConfig::default()
                };

                let mut attempts = 0;
                let mut signature: Option<Signature> = None;
                loop {
                    match rpc_client
                        .send_transaction_with_config(&tx, rpc_config)
                        .await
                    {
                        Ok(sig) => {
                            signature = Some(sig);
                            break;
                        }
                        Err(e) => {
                            error!(target: "server_log", "Failed to send claim transaction: {:?}. Retrying in 2 seconds...", e);
                            tokio::time::sleep(Duration::from_millis(2000)).await;
                            attempts += 1;
                            if attempts >= 5 {
                                error!(target: "server_log", "Failed to send claim transaction after 5 attempts.");
                                break;
                            }
                        }
                    }
                }

                if signature.is_some() {
                    let signature = signature.unwrap();
                    let result: Result<Signature, String> = loop {
                        if expired_timer.elapsed().as_secs() >= 200 {
                            break Err("Transaction Expired".to_string());
                        }
                        let results = rpc_client.get_signature_statuses(&[signature]).await;
                        if let Ok(response) = results {
                            let statuses = response.value;
                            if let Some(status) = &statuses[0] {
                                if status.confirmation_status()
                                    == TransactionConfirmationStatus::Confirmed
                                {
                                    if status.err.is_some() {
                                        let e_str = format!("Transaction Failed: {:?}", status.err);
                                        break Err(e_str);
                                    }
                                    break Ok(signature);
                                }
                            }
                        }
                        tokio::time::sleep(Duration::from_millis(500)).await;
                    };

                    match result {
                        Ok(sig) => {
                            let amount_dec_coal =
                                amount_coal as f64 / 10f64.powf(COAL_TOKEN_DECIMALS as f64);
                            let amount_dec_ore =
                                amount_ore as f64 / 10f64.powf(ORE_TOKEN_DECIMALS as f64);
                            let amount_dec_chromium =
                                amount_chromium as f64 / 10f64.powf(ORE_TOKEN_DECIMALS as f64);
                            info!(target: "server_log", "Miner {} successfully claimed {} COAL and {} ORE and {} CHROMIUM.\nSig: {}", miner_pubkey.to_string(), amount_dec_coal, amount_dec_ore, amount_dec_chromium, sig.to_string());

                            // TODO: use transacions, or at least put them into one query
                            let miner = app_database
                                .get_miner_by_pubkey_str(miner_pubkey.to_string())
                                .await
                                .unwrap();
                            let db_pool = app_database
                                .get_pool_by_authority_pubkey(wallet.pubkey().to_string())
                                .await
                                .unwrap();
                            while let Err(_) = app_database
                                .decrease_miner_reward(
                                    miner.id,
                                    amount_coal,
                                    amount_ore,
                                    amount_chromium,
                                )
                                .await
                            {
                                error!(target: "server_log", "Failed to decrease miner rewards! Retrying...");
                                tokio::time::sleep(Duration::from_millis(2000)).await;
                            }
                            while let Err(_) = app_database
                                .update_pool_claimed(
                                    wallet.pubkey().to_string(),
                                    amount_coal,
                                    amount_ore,
                                    amount_chromium,
                                )
                                .await
                            {
                                error!(target: "server_log", "Failed to increase pool claimed amount! Retrying...");
                                tokio::time::sleep(Duration::from_millis(2000)).await;
                            }

                            let itxn = InsertTxn {
                                txn_type: "claim".to_string(),
                                signature: sig.to_string(),
                                priority_fee: prio_fee,
                            };
                            while let Err(_) = app_database.add_new_txn(itxn.clone()).await {
                                error!(target: "server_log", "Failed to increase pool claimed amount! Retrying...");
                                tokio::time::sleep(Duration::from_millis(2000)).await;
                            }

                            let txn_id;
                            loop {
                                if let Ok(ntxn) = app_database.get_txn_by_sig(sig.to_string()).await
                                {
                                    txn_id = ntxn.id;
                                    break;
                                } else {
                                    error!(target: "server_log", "Failed to get tx by sig! Retrying...");
                                    tokio::time::sleep(Duration::from_millis(2000)).await;
                                }
                            }

                            let iclaim = InsertClaim {
                                miner_id: miner.id,
                                pool_id: db_pool.id,
                                txn_id,
                                amount_coal,
                                amount_ore,
                            };
                            while let Err(_) = app_database.add_new_claim(iclaim).await {
                                error!(target: "server_log", "Failed add new claim to db! Retrying...");
                                tokio::time::sleep(Duration::from_millis(2000)).await;
                            }

                            let mut writer = claims_queue.queue.write().await;
                            writer.remove(&miner_pubkey);
                            drop(writer);

                            info!(target: "server_log", "Claim successfully processed!");
                        }
                        Err(e) => {
                            error!(target: "server_log", "ERROR: {:?}", e);
                        }
                    }
                }
            } else {
                error!(target: "server_log", "Failed to confirm transaction, will retry on next iteration.");
            }
        }

        tokio::time::sleep(Duration::from_secs(10)).await;
    }
}
