use std::{sync::Arc, time::Duration};

use crate::coal_utils::{proof_pubkey, Resource};
use base64::{prelude::BASE64_STANDARD, Engine};
use futures::StreamExt;
use solana_account_decoder::UiAccountEncoding;
use solana_client::{nonblocking::pubsub_client::PubsubClient, rpc_config::RpcAccountInfoConfig};
use solana_sdk::{commitment_config::CommitmentConfig, signature::Keypair, signer::Signer};
use steel::AccountDeserialize;
use tokio::sync::Mutex;

use crate::coal_api;

pub async fn proof_tracking_system(
    ws_url: String,
    wallet: Arc<Keypair>,
    proof: Arc<Mutex<coal_api::state::Proof>>,
    app_last_challenge: Arc<Mutex<[u8; 32]>>,
) {
    loop {
        tracing::info!(target: "server_log", "Establishing rpc websocket connection...");
        let mut ps_client = PubsubClient::new(&ws_url).await;
        let mut attempts = 0;

        while ps_client.is_err() && attempts < 3 {
            tracing::error!(target: "server_log", "Failed to connect to websocket, retrying...");
            ps_client = PubsubClient::new(&ws_url).await;
            tokio::time::sleep(Duration::from_millis(1000)).await;
            attempts += 1;
        }
        tracing::info!(target: "server_log", "RPC WS connection established!");

        let app_wallet = wallet.clone();
        if let Ok(ps_client) = ps_client {
            let ps_client = Arc::new(ps_client);
            let account_pubkey = proof_pubkey(app_wallet.pubkey(), Resource::Coal);
            let pubsub = ps_client
                .account_subscribe(
                    &account_pubkey,
                    Some(RpcAccountInfoConfig {
                        encoding: Some(UiAccountEncoding::Base64),
                        data_slice: None,
                        commitment: Some(CommitmentConfig::confirmed()),
                        min_context_slot: None,
                    }),
                )
                .await;

            tracing::info!(target: "server_log", "Tracking pool proof updates with websocket");
            if let Ok((mut account_sub_notifications, _account_unsub)) = pubsub {
                while let Some(response) = account_sub_notifications.next().await {
                    let data = response.value.data.decode();
                    if let Some(data_bytes) = data {
                        // if let Ok(bus) = Bus::try_from_bytes(&data_bytes) {
                        //     let _ = sender.send(AccountUpdatesData::BusData(*bus));
                        // }
                        // if let Ok(coal_config) = coal_api::state::Config::try_from_bytes(&data_bytes) {
                        //     let _ = sender.send(AccountUpdatesData::TreasuryConfigData(*coal_config));
                        // }
                        if let Ok(new_proof) = coal_api::state::Proof::try_from_bytes(&data_bytes) {
                            tracing::info!(target: "server_log", "Got new proof data");
                            tracing::info!(target: "server_log", "Challenge: {}", BASE64_STANDARD.encode(new_proof.challenge));

                            let lock = app_last_challenge.lock().await;
                            let last_challenge = lock.clone();
                            drop(lock);

                            if last_challenge.eq(&new_proof.challenge) {
                                tracing::error!(target: "server_log", "Websocket tried to update proof with old challenge!");
                            } else {
                                let mut app_proof = proof.lock().await;
                                *app_proof = *new_proof;
                                drop(app_proof);
                            }
                        }
                    }
                }
            }
        }
    }
}
