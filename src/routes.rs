use app_rr_database::AppRRDatabase;
use axum::{
    http::{Response, StatusCode},
    response::IntoResponse,
    Extension, Json,
};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use spl_associated_token_account::get_associated_token_address;
use steel::AccountDeserialize;
use tracing::error;

use crate::coal_utils::Resource;
use crate::models::ExtraResourcesGenerationType;
use crate::ore_utils::get_proof_with_authority;
use crate::{
    app_rr_database,
    coal_utils::{get_coal_mint, get_proof as get_proof_coal},
    ChallengeWithDifficulty, Config, Txn,
};
use chrono::{NaiveDateTime, Utc};
use coal_guilds_api::prelude::Member;
use coal_guilds_api::state::Guild;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use std::{str::FromStr, sync::Arc};

pub async fn get_challenges(
    Extension(app_rr_database): Extension<Arc<AppRRDatabase>>,
    Extension(app_config): Extension<Arc<Config>>,
) -> Result<Json<Vec<ChallengeWithDifficulty>>, String> {
    if app_config.stats_enabled {
        let res = app_rr_database.get_challenges().await;

        match res {
            Ok(challenges) => Ok(Json(challenges)),
            Err(_) => Err("Failed to get submissions for miner".to_string()),
        }
    } else {
        return Err("Stats not enabled for this server.".to_string());
    }
}

pub async fn get_latest_mine_txn(
    Extension(app_rr_database): Extension<Arc<AppRRDatabase>>,
    Extension(app_config): Extension<Arc<Config>>,
) -> Result<Json<Txn>, String> {
    if app_config.stats_enabled {
        let res = app_rr_database.get_latest_mine_txn().await;

        match res {
            Ok(txn) => Ok(Json(txn)),
            Err(_) => Err("Failed to get latest mine txn".to_string()),
        }
    } else {
        return Err("Stats not enabled for this server.".to_string());
    }
}

pub async fn get_guild_addresses(
    Extension(app_config): Extension<Arc<Config>>,
    Extension(rpc_client): Extension<Arc<RpcClient>>,
) -> Result<Json<PoolGuild>, String> {
    if app_config.guild_address.is_empty() {
        return Err("Failed to get guild info".to_string());
    }
    let guild_address = app_config.guild_address.clone();
    let guild_data = rpc_client
        .get_account_data(&Pubkey::from_str(&guild_address).unwrap())
        .await;

    if let Ok(guild_data) = guild_data {
        let guild = Guild::try_from_bytes(&guild_data).unwrap();

        return Ok(Json(PoolGuild {
            authority: guild.authority.to_string(),
            pubkey: guild_address,
        }));
    } else {
        Err("Failed to get guild info".to_string())
    }
}

pub async fn get_pool(
    Extension(app_rr_database): Extension<Arc<AppRRDatabase>>,
    Extension(app_config): Extension<Arc<Config>>,
) -> Result<Json<crate::models::Pool>, String> {
    if app_config.stats_enabled {
        let pubkey = Pubkey::from_str("6zbGwDbfwVS3hF8r7Yei8HuwSWm2yb541jUtmAZKhFDM").unwrap();
        let res = app_rr_database
            .get_pool_by_authority_pubkey(pubkey.to_string())
            .await;

        match res {
            Ok(pool) => Ok(Json(pool)),
            Err(_) => Err("Failed to get pool data".to_string()),
        }
    } else {
        return Err("Stats not enabled for this server.".to_string());
    }
}

pub async fn get_chromium_reprocess_info(
    Extension(app_rr_database): Extension<Arc<AppRRDatabase>>,
    Extension(app_config): Extension<Arc<Config>>,
) -> impl IntoResponse {
    if app_config.stats_enabled {
        let res = app_rr_database
            .get_last_reprocessing(
                app_config.pool_id,
                ExtraResourcesGenerationType::ChromiumReprocess,
            )
            .await;

        match res {
            Ok(pool) => {
                let next_preprocess = pool.created_at + Duration::from_secs(60 * 60 * 24 * 3);
                return Ok(Json(ChromiumReprocessInfo {
                    last_reprocess: pool.created_at,
                    next_reprocess: next_preprocess,
                }));
            }
            Err(_) => Err("Failed to get pool data".to_string()),
        }
    } else {
        return Err("Stats not enabled for this server.".to_string());
    }
}

pub async fn get_diamond_hands_reprocess_info(
    Extension(app_rr_database): Extension<Arc<AppRRDatabase>>,
    Extension(app_config): Extension<Arc<Config>>,
) -> impl IntoResponse {
    if app_config.stats_enabled {
        let res = app_rr_database
            .get_last_reprocessing(
                app_config.pool_id,
                ExtraResourcesGenerationType::DiamondHandsReprocess,
            )
            .await;

        match res {
            Ok(pool) => {
                let next_preprocess = pool.created_at + Duration::from_secs(60 * 60 * 24 * 7);
                return Ok(Json(ChromiumReprocessInfo {
                    last_reprocess: pool.created_at,
                    next_reprocess: next_preprocess,
                }));
            }
            Err(_) => Err("Failed to get pool data".to_string()),
        }
    } else {
        return Err("Stats not enabled for this server.".to_string());
    }
}

#[derive(Serialize)]
struct BalanceData {
    coal_balance: u64,
    ore_balance: u64,
}
pub async fn get_pool_staked(
    Extension(app_config): Extension<Arc<Config>>,
    Extension(rpc_client): Extension<Arc<RpcClient>>,
) -> impl IntoResponse {
    if app_config.stats_enabled {
        let pubkey = Pubkey::from_str("6zbGwDbfwVS3hF8r7Yei8HuwSWm2yb541jUtmAZKhFDM").unwrap();
        let proof_coal = if let Ok(loaded_proof) = get_proof_coal(&rpc_client, pubkey).await {
            loaded_proof
        } else {
            error!("get_pool_staked: Failed to load proof.");
            return Err("Stats not enabled for this server.".to_string());
        };

        let proof_ore =
            if let Ok(loaded_proof) = get_proof_with_authority(&rpc_client, pubkey).await {
                loaded_proof
            } else {
                error!("get_pool_staked: Failed to load proof.");
                return Err("Stats not enabled for this server.".to_string());
            };

        return Ok(Json(BalanceData {
            coal_balance: proof_coal.balance,
            ore_balance: proof_ore.balance,
        }));
    } else {
        return Err("Stats not enabled for this server.".to_string());
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PoolGuild {
    pub pubkey: String,
    pub authority: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ChromiumReprocessInfo {
    pub last_reprocess: NaiveDateTime,
    pub next_reprocess: NaiveDateTime,
}
