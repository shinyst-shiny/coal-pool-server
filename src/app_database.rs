use chrono::{DateTime, NaiveDateTime};
use deadpool_diesel::mysql::{Manager, Pool};
use diesel::sql_types::Datetime;
use diesel::{
    insert_into,
    sql_types::{BigInt, Binary, Bool, Integer, Nullable, Text, Unsigned},
    Connection, MysqlConnection, RunQueryDsl,
};
use tokio::time::Instant;
use tracing::{error, info};

use crate::models::ExtraResourcesGenerationType;
use crate::{models, Miner, SubmissionWithId};

#[derive(Debug)]
pub enum AppDatabaseError {
    FailedToGetConnectionFromPool,
    FailedToUpdateRow,
    FailedToInsertRow,
    InteractionFailed,
    QueryFailed,
}

pub struct AppDatabase {
    connection_pool: Pool,
}

impl AppDatabase {
    pub fn new(url: String) -> Self {
        let manager = Manager::new(url, deadpool_diesel::Runtime::Tokio1);

        let pool = Pool::builder(manager).build().unwrap();

        AppDatabase {
            connection_pool: pool,
        }
    }

    pub async fn get_challenge_by_challenge(
        &self,
        challenge: Vec<u8>,
    ) -> Result<models::Challenge, AppDatabaseError> {
        if let Ok(db_conn) = self.connection_pool.get().await {
            let res = db_conn.interact(move |conn: &mut MysqlConnection| {
                diesel::sql_query("SELECT id, pool_id, submission_id, challenge, rewards_earned_coal, rewards_earned_ore FROM challenges WHERE challenges.challenge = ?")
                    .bind::<Binary, _>(challenge)
                    .get_result::<models::Challenge>(conn)
            }).await;

            match res {
                Ok(interaction) => match interaction {
                    Ok(query) => {
                        return Ok(query);
                    }
                    Err(e) => {
                        error!(target: "server_log", "{:?}", e);
                        return Err(AppDatabaseError::QueryFailed);
                    }
                },
                Err(e) => {
                    error!(target: "server_log", "{:?}", e);
                    return Err(AppDatabaseError::InteractionFailed);
                }
            }
        } else {
            return Err(AppDatabaseError::FailedToGetConnectionFromPool);
        };
    }

    pub async fn get_miner_rewards_by_id(
        &self,
        miner_id: i32,
    ) -> Result<models::Reward, AppDatabaseError> {
        if let Ok(db_conn) = self.connection_pool.get().await {
            let res = db_conn
                .interact(move |conn: &mut MysqlConnection| {
                    diesel::sql_query("SELECT * FROM rewards r WHERE r.miner_id = ?")
                        .bind::<Integer, _>(miner_id)
                        .get_result::<models::Reward>(conn)
                })
                .await;

            match res {
                Ok(interaction) => match interaction {
                    Ok(query) => {
                        return Ok(query);
                    }
                    Err(e) => {
                        error!(target: "server_log", "{:?}", e);
                        return Err(AppDatabaseError::QueryFailed);
                    }
                },
                Err(e) => {
                    error!(target: "server_log", "{:?}", e);
                    return Err(AppDatabaseError::InteractionFailed);
                }
            }
        } else {
            return Err(AppDatabaseError::FailedToGetConnectionFromPool);
        };
    }

    pub async fn get_miner_rewards_coal_major_than_zero(
        &self,
    ) -> Result<Vec<models::Reward>, AppDatabaseError> {
        if let Ok(db_conn) = self.connection_pool.get().await {
            let res = db_conn
                .interact(move |conn: &mut MysqlConnection| {
                    diesel::sql_query("SELECT * FROM rewards r WHERE r.balance_coal > 0")
                        .get_results::<models::Reward>(conn)
                })
                .await;

            match res {
                Ok(interaction) => match interaction {
                    Ok(query) => {
                        return Ok(query);
                    }
                    Err(e) => {
                        error!("get_miner_rewards app_rr_database: {:?}", e);
                        return Err(AppDatabaseError::QueryFailed);
                    }
                },
                Err(e) => {
                    error!("{:?}", e);
                    return Err(AppDatabaseError::InteractionFailed);
                }
            }
        } else {
            return Err(AppDatabaseError::FailedToGetConnectionFromPool);
        };
    }

    pub async fn get_miner_rewards(
        &self,
        miner_pubkey: String,
    ) -> Result<models::Reward, AppDatabaseError> {
        if let Ok(db_conn) = self.connection_pool.get().await {
            let res = db_conn.interact(move |conn: &mut MysqlConnection| {
                diesel::sql_query("SELECT * FROM miners m JOIN rewards r ON m.id = r.miner_id WHERE m.pubkey = ?")
                    .bind::<Text, _>(miner_pubkey)
                    .get_result::<models::Reward>(conn)
            }).await;

            match res {
                Ok(interaction) => match interaction {
                    Ok(query) => {
                        return Ok(query);
                    }
                    Err(e) => {
                        error!(target: "server_log", "{:?}", e);
                        return Err(AppDatabaseError::QueryFailed);
                    }
                },
                Err(e) => {
                    error!(target: "server_log", "{:?}", e);
                    return Err(AppDatabaseError::InteractionFailed);
                }
            }
        } else {
            return Err(AppDatabaseError::FailedToGetConnectionFromPool);
        };
    }

    pub async fn update_rewards(
        &self,
        rewards: Vec<models::UpdateReward>,
    ) -> Result<(), AppDatabaseError> {
        let id = uuid::Uuid::new_v4();
        let instant = Instant::now();
        tracing::info!(target: "server_log", "{} - Getting db pool connection.", id);
        if let Ok(db_conn) = self.connection_pool.get().await {
            tracing::info!(target: "server_log", "{} - Got db pool connection in {}ms.", id, instant.elapsed().as_millis());

            let reward_types = vec!["coal", "ore", "chromium", "sol", "wood", "ingot"];

            let queries: Vec<_> = reward_types
                .into_iter()
                .map(|reward_type| {
                    let rewards_clone = rewards.clone();
                    db_conn.interact(move |conn: &mut MysqlConnection| {
                        let subquery = rewards_clone
                            .iter()
                            .map(|r| {
                                let balance = match reward_type {
                                    "coal" => r.balance_coal,
                                    "ore" => r.balance_ore,
                                    "chromium" => r.balance_chromium,
                                    "sol" => r.balance_sol,
                                    "wood" => r.balance_wood,
                                    "ingot" => r.balance_ingot,
                                    _ => unreachable!(),
                                };
                                format!("SELECT {} as miner_id, {} as balance", r.miner_id, balance)
                            })
                            .collect::<Vec<_>>()
                            .join(" UNION ALL ");

                        let query = format!(
                            "UPDATE rewards r
                         JOIN (
                             SELECT miner_id, SUM(balance) as total_balance
                             FROM ({}) as temp
                             GROUP BY miner_id
                         ) as grouped
                         ON r.miner_id = grouped.miner_id
                         SET r.balance_{} = r.balance_{} + grouped.total_balance",
                            subquery, reward_type, reward_type
                        );

                        diesel::sql_query(query).execute(conn)
                    })
                })
                .collect();

            let res = futures::future::try_join_all(queries).await;

            match res {
                Ok(_) => Ok(()),
                Err(e) => {
                    error!(target: "server_log", "update rewards error: {:?}", e);
                    Err(AppDatabaseError::QueryFailed)
                }
            }
        } else {
            Err(AppDatabaseError::FailedToGetConnectionFromPool)
        }
    }

    pub async fn decrease_miner_reward(
        &self,
        rewards: models::UpdateReward,
    ) -> Result<(), AppDatabaseError> {
        if let Ok(db_conn) = self.connection_pool.get().await {
            let res = db_conn
                .interact(move |conn: &mut MysqlConnection| {
                    diesel::sql_query("UPDATE rewards SET balance_coal = balance_coal - ?, balance_ore = balance_ore - ?, balance_chromium = balance_chromium - ?, balance_sol = balance_sol - ?, balance_wood = balance_wood - ?, balance_ingot = balance_ingot - ?  WHERE miner_id = ?")
                        .bind::<Unsigned<BigInt>, _>(rewards.balance_coal)
                        .bind::<Unsigned<BigInt>, _>(rewards.balance_ore)
                        .bind::<Unsigned<BigInt>, _>(rewards.balance_chromium)
                        .bind::<Unsigned<BigInt>, _>(rewards.balance_sol)
                        .bind::<Unsigned<BigInt>, _>(rewards.balance_wood)
                        .bind::<Unsigned<BigInt>, _>(rewards.balance_ingot)
                        .bind::<Integer, _>(rewards.miner_id)
                        .execute(conn)
                })
                .await;

            match res {
                Ok(interaction) => match interaction {
                    Ok(_query) => {
                        return Ok(());
                    }
                    Err(e) => {
                        error!(target: "server_log", "{:?}", e);
                        return Err(AppDatabaseError::QueryFailed);
                    }
                },
                Err(e) => {
                    error!(target: "server_log", "{:?}", e);
                    return Err(AppDatabaseError::InteractionFailed);
                }
            }
        } else {
            return Err(AppDatabaseError::FailedToGetConnectionFromPool);
        };
    }

    pub async fn get_submission_id_with_nonce(&self, nonce: u64) -> Result<i64, AppDatabaseError> {
        if let Ok(db_conn) = self.connection_pool.get().await {
            let res = db_conn
                .interact(move |conn: &mut MysqlConnection| {
                    diesel::sql_query(
                        "SELECT id FROM submissions WHERE submissions.nonce = ? ORDER BY id DESC",
                    )
                    .bind::<Unsigned<BigInt>, _>(nonce)
                    .get_result::<SubmissionWithId>(conn)
                })
                .await;

            match res {
                Ok(interaction) => match interaction {
                    Ok(query) => {
                        return Ok(query.id);
                    }
                    Err(e) => {
                        error!(target: "server_log", "{:?}", e);
                        return Err(AppDatabaseError::QueryFailed);
                    }
                },
                Err(e) => {
                    error!(target: "server_log", "{:?}", e);
                    return Err(AppDatabaseError::InteractionFailed);
                }
            }
        } else {
            return Err(AppDatabaseError::FailedToGetConnectionFromPool);
        };
    }

    pub async fn update_challenge_rewards(
        &self,
        challenge: Vec<u8>,
        submission_id: i64,
        rewards_coal: u64,
        rewards_ore: u64,
    ) -> Result<(), AppDatabaseError> {
        if let Ok(db_conn) = self.connection_pool.get().await {
            let res = db_conn.interact(move |conn: &mut MysqlConnection| {
                diesel::sql_query("UPDATE challenges SET rewards_earned_coal = ?, rewards_earned_ore = ?, submission_id = ? WHERE challenge = ?")
                    .bind::<Nullable<Unsigned<BigInt>>, _>(Some(rewards_coal))
                    .bind::<Nullable<Unsigned<BigInt>>, _>(Some(rewards_ore))
                    .bind::<Nullable<BigInt>, _>(submission_id)
                    .bind::<Binary, _>(challenge)
                    .execute(conn)
            }).await;

            match res {
                Ok(interaction) => match interaction {
                    Ok(query) => {
                        if query != 1 {
                            return Err(AppDatabaseError::FailedToUpdateRow);
                        }
                        info!(target: "server_log", "Updated challenge rewards!");
                        return Ok(());
                    }
                    Err(e) => {
                        error!(target: "server_log", "{:?}", e);
                        return Err(AppDatabaseError::QueryFailed);
                    }
                },
                Err(e) => {
                    error!(target: "server_log", "{:?}", e);
                    return Err(AppDatabaseError::InteractionFailed);
                }
            }
        } else {
            return Err(AppDatabaseError::FailedToGetConnectionFromPool);
        };
    }

    pub async fn add_new_challenge(
        &self,
        challenge: models::InsertChallenge,
    ) -> Result<(), AppDatabaseError> {
        if let Ok(db_conn) = self.connection_pool.get().await {
            let res = db_conn.interact(move |conn: &mut MysqlConnection| {
                diesel::sql_query("INSERT INTO challenges (pool_id, challenge, rewards_earned_coal, rewards_earned_ore) VALUES (?, ?, ?, ?)")
                    .bind::<Integer, _>(challenge.pool_id)
                    .bind::<Binary, _>(challenge.challenge)
                    .bind::<Nullable<Unsigned<BigInt>>, _>(challenge.rewards_earned_coal)
                    .bind::<Nullable<Unsigned<BigInt>>, _>(challenge.rewards_earned_ore)
                    .execute(conn)
            }).await;

            match res {
                Ok(interaction) => match interaction {
                    Ok(query) => {
                        if query != 1 {
                            return Err(AppDatabaseError::FailedToInsertRow);
                        }
                        return Ok(());
                    }
                    Err(e) => {
                        error!(target: "server_log", "{:?}", e);
                        return Err(AppDatabaseError::QueryFailed);
                    }
                },
                Err(e) => {
                    error!(target: "server_log", "{:?}", e);
                    return Err(AppDatabaseError::InteractionFailed);
                }
            }
        } else {
            return Err(AppDatabaseError::FailedToGetConnectionFromPool);
        };
    }

    pub async fn get_pool_by_authority_pubkey(
        &self,
        pool_pubkey: String,
    ) -> Result<models::Pool, AppDatabaseError> {
        if let Ok(db_conn) = self.connection_pool.get().await {
            let res = db_conn
                .interact(move |conn: &mut MysqlConnection| {
                    diesel::sql_query("SELECT * FROM pools WHERE pools.authority_pubkey = ?")
                        .bind::<Text, _>(pool_pubkey)
                        .get_result::<models::Pool>(conn)
                })
                .await;

            match res {
                Ok(interaction) => match interaction {
                    Ok(query) => {
                        return Ok(query);
                    }
                    Err(e) => {
                        error!(target: "server_log", "{:?}", e);
                        return Err(AppDatabaseError::QueryFailed);
                    }
                },
                Err(e) => {
                    error!(target: "server_log", "{:?}", e);
                    return Err(AppDatabaseError::InteractionFailed);
                }
            }
        } else {
            return Err(AppDatabaseError::FailedToGetConnectionFromPool);
        };
    }

    pub async fn add_new_pool(
        &self,
        authority_pubkey: String,
        proof_pubkey: String,
    ) -> Result<(), AppDatabaseError> {
        if let Ok(db_conn) = self.connection_pool.get().await {
            let res = db_conn
                .interact(move |conn: &mut MysqlConnection| {
                    diesel::sql_query(
                        "INSERT INTO pools (authority_pubkey, proof_pubkey) VALUES (?, ?)",
                    )
                    .bind::<Text, _>(authority_pubkey)
                    .bind::<Text, _>(proof_pubkey)
                    .execute(conn)
                })
                .await;

            match res {
                Ok(interaction) => match interaction {
                    Ok(query) => {
                        if query != 1 {
                            return Err(AppDatabaseError::FailedToInsertRow);
                        }
                        return Ok(());
                    }
                    Err(e) => {
                        error!(target: "server_log", "{:?}", e);
                        return Err(AppDatabaseError::QueryFailed);
                    }
                },
                Err(e) => {
                    error!(target: "server_log", "{:?}", e);
                    return Err(AppDatabaseError::InteractionFailed);
                }
            }
        } else {
            return Err(AppDatabaseError::FailedToGetConnectionFromPool);
        };
    }

    pub async fn update_pool_rewards(
        &self,
        pool_authority_pubkey: String,
        earned_rewards_sol: u64,
        earned_rewards_coal: u64,
        earned_rewards_ore: u64,
        earned_rewards_chromium: u64,
        earned_rewards_wood: u64,
        earned_rewards_ingot: u64,
    ) -> Result<(), AppDatabaseError> {
        info!(target: "server_log", "Updating pool rewards for {} with {} coal and {} ore", pool_authority_pubkey, earned_rewards_coal, earned_rewards_ore);
        if let Ok(db_conn) = self.connection_pool.get().await {
            let res = db_conn.interact(move |conn: &mut MysqlConnection| {
                diesel::sql_query("UPDATE pools SET total_rewards_coal = total_rewards_coal + ?, total_rewards_ore = total_rewards_ore + ?, total_rewards_chromium = total_rewards_chromium + ?, total_rewards_sol = total_rewards_sol + ?, total_rewards_wood = total_rewards_wood + ?, total_rewards_ingot = total_rewards_ingot + ? WHERE authority_pubkey = ?")
                    .bind::<Unsigned<BigInt>, _>(earned_rewards_coal)
                    .bind::<Unsigned<BigInt>, _>(earned_rewards_ore)
                    .bind::<Unsigned<BigInt>, _>(earned_rewards_chromium)
                    .bind::<Unsigned<BigInt>, _>(earned_rewards_sol)
                    .bind::<Unsigned<BigInt>, _>(earned_rewards_wood)
                    .bind::<Unsigned<BigInt>, _>(earned_rewards_ingot)
                    .bind::<Text, _>(pool_authority_pubkey)
                    .execute(conn)
            }).await;

            match res {
                Ok(interaction) => match interaction {
                    Ok(query) => {
                        info!(target: "server_log", "Successfully updated pool rewards");
                        return Ok(());
                    }
                    Err(e) => {
                        error!(target: "server_log", "{:?}", e);
                        return Err(AppDatabaseError::QueryFailed);
                    }
                },
                Err(e) => {
                    error!(target: "server_log", "{:?}", e);
                    return Err(AppDatabaseError::InteractionFailed);
                }
            }
        } else {
            return Err(AppDatabaseError::FailedToGetConnectionFromPool);
        };
    }

    pub async fn update_pool_claimed(
        &self,
        pool_authority_pubkey: String,
        claimed_rewards_sol: u64,
        claimed_rewards_coal: u64,
        claimed_rewards_ore: u64,
        claimed_rewards_chromium: u64,
        claimed_rewards_wood: u64,
        claimed_rewards_ingot: u64,
    ) -> Result<(), AppDatabaseError> {
        if let Ok(db_conn) = self.connection_pool.get().await {
            let res = db_conn.interact(move |conn: &mut MysqlConnection| {
                diesel::sql_query("UPDATE pools SET claimed_rewards_coal = claimed_rewards_coal + ?, claimed_rewards_ore = claimed_rewards_ore + ?, claimed_rewards_chromium = claimed_rewards_chromium + ?, claimed_rewards_sol = claimed_rewards_sol + ?, claimed_rewards_wood = claimed_rewards_wood + ?, claimed_rewards_ingot = claimed_rewards_ingot + ? WHERE authority_pubkey = ?")
                    .bind::<Unsigned<BigInt>, _>(claimed_rewards_coal)
                    .bind::<Unsigned<BigInt>, _>(claimed_rewards_ore)
                    .bind::<Unsigned<BigInt>, _>(claimed_rewards_chromium)
                    .bind::<Unsigned<BigInt>, _>(claimed_rewards_sol)
                    .bind::<Unsigned<BigInt>, _>(claimed_rewards_wood)
                    .bind::<Unsigned<BigInt>, _>(claimed_rewards_ingot)
                    .bind::<Text, _>(pool_authority_pubkey)
                    .execute(conn)
            }).await;

            match res {
                Ok(interaction) => match interaction {
                    Ok(query) => {
                        if query != 1 {
                            return Err(AppDatabaseError::FailedToUpdateRow);
                        }
                        return Ok(());
                    }
                    Err(e) => {
                        error!(target: "server_log", "{:?}", e);
                        return Err(AppDatabaseError::QueryFailed);
                    }
                },
                Err(e) => {
                    error!(target: "server_log", "{:?}", e);
                    return Err(AppDatabaseError::InteractionFailed);
                }
            }
        } else {
            return Err(AppDatabaseError::FailedToGetConnectionFromPool);
        };
    }

    pub async fn get_miner_by_id(&self, miner_id: i32) -> Result<Miner, AppDatabaseError> {
        if let Ok(db_conn) = self.connection_pool.get().await {
            let res = db_conn
                .interact(move |conn: &mut MysqlConnection| {
                    diesel::sql_query("SELECT * FROM miners WHERE miners.id = ?")
                        .bind::<Integer, _>(miner_id)
                        .get_result::<Miner>(conn)
                })
                .await;
            match res {
                Ok(interaction) => match interaction {
                    Ok(query) => {
                        return Ok(query);
                    }
                    Err(e) => {
                        error!(target: "server_log", "{:?}", e);
                        return Err(AppDatabaseError::QueryFailed);
                    }
                },
                Err(e) => {
                    error!(target: "server_log", "{:?}", e);
                    return Err(AppDatabaseError::InteractionFailed);
                }
            }
        } else {
            return Err(AppDatabaseError::FailedToGetConnectionFromPool);
        };
    }

    pub async fn get_miner_by_pubkey_str(
        &self,
        miner_pubkey: String,
    ) -> Result<Miner, AppDatabaseError> {
        if let Ok(db_conn) = self.connection_pool.get().await {
            let res = db_conn
                .interact(move |conn: &mut MysqlConnection| {
                    diesel::sql_query(
                        "SELECT id, pubkey, enabled FROM miners WHERE miners.pubkey = ?",
                    )
                    .bind::<Text, _>(miner_pubkey)
                    .get_result::<Miner>(conn)
                })
                .await;
            match res {
                Ok(interaction) => match interaction {
                    Ok(query) => {
                        return Ok(query);
                    }
                    Err(e) => {
                        error!(target: "server_log", "{:?}", e);
                        return Err(AppDatabaseError::QueryFailed);
                    }
                },
                Err(e) => {
                    error!(target: "server_log", "{:?}", e);
                    return Err(AppDatabaseError::InteractionFailed);
                }
            }
        } else {
            return Err(AppDatabaseError::FailedToGetConnectionFromPool);
        };
    }

    pub async fn add_new_claim(&self, claim: models::InsertClaim) -> Result<(), AppDatabaseError> {
        if let Ok(db_conn) = self.connection_pool.get().await {
            let res = db_conn.interact(move |conn: &mut MysqlConnection| {
                diesel::sql_query("INSERT INTO claims (miner_id, pool_id, txn_id, amount_coal, amount_ore, amount_chromium, amount_sol, amount_wood, amount_ingot) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)")
                    .bind::<Integer, _>(claim.miner_id)
                    .bind::<Integer, _>(claim.pool_id)
                    .bind::<Integer, _>(claim.txn_id)
                    .bind::<Unsigned<BigInt>, _>(claim.amount_coal)
                    .bind::<Unsigned<BigInt>, _>(claim.amount_ore)
                    .bind::<Unsigned<BigInt>, _>(claim.amount_chromium)
                    .bind::<Unsigned<BigInt>, _>(claim.amount_sol)
                    .bind::<Unsigned<BigInt>, _>(claim.amount_wood)
                    .bind::<Unsigned<BigInt>, _>(claim.amount_ingot)
                    .execute(conn)
            }).await;

            match res {
                Ok(interaction) => match interaction {
                    Ok(_query) => {
                        return Ok(());
                    }
                    Err(e) => {
                        error!(target: "server_log", "{:?}", e);
                        return Err(AppDatabaseError::QueryFailed);
                    }
                },
                Err(e) => {
                    error!(target: "server_log", "{:?}", e);
                    return Err(AppDatabaseError::InteractionFailed);
                }
            }
        } else {
            return Err(AppDatabaseError::FailedToGetConnectionFromPool);
        };
    }

    pub async fn get_last_claim(
        &self,
        miner_id: i32,
    ) -> Result<models::LastClaim, AppDatabaseError> {
        if let Ok(db_conn) = self.connection_pool.get().await {
            let res = db_conn
                .interact(move |conn: &mut MysqlConnection| {
                    diesel::sql_query(
                        "SELECT created_at FROM claims WHERE miner_id = ? ORDER BY id DESC",
                    )
                    .bind::<Integer, _>(miner_id)
                    .get_result::<models::LastClaim>(conn)
                })
                .await;

            match res {
                Ok(interaction) => match interaction {
                    Ok(query) => {
                        return Ok(query);
                    }
                    Err(e) => {
                        error!(target: "server_log", "get_last_claim {:?}", e);
                        return Err(AppDatabaseError::QueryFailed);
                    }
                },
                Err(e) => {
                    error!(target: "server_log", "get_last_claim {:?}", e);
                    return Err(AppDatabaseError::InteractionFailed);
                }
            }
        } else {
            return Err(AppDatabaseError::FailedToGetConnectionFromPool);
        };
    }

    pub async fn add_new_txn(&self, txn: models::InsertTxn) -> Result<(), AppDatabaseError> {
        if let Ok(db_conn) = self.connection_pool.get().await {
            let res = db_conn
                .interact(move |conn: &mut MysqlConnection| {
                    diesel::sql_query(
                        "INSERT INTO txns (txn_type, signature, priority_fee) VALUES (?, ?, ?)",
                    )
                    .bind::<Text, _>(txn.txn_type)
                    .bind::<Text, _>(txn.signature)
                    .bind::<Unsigned<Integer>, _>(txn.priority_fee)
                    .execute(conn)
                })
                .await;

            match res {
                Ok(interaction) => match interaction {
                    Ok(_query) => {
                        return Ok(());
                    }
                    Err(e) => {
                        error!(target: "server_log", "{:?}", e);
                        return Err(AppDatabaseError::QueryFailed);
                    }
                },
                Err(e) => {
                    error!(target: "server_log", "{:?}", e);
                    return Err(AppDatabaseError::InteractionFailed);
                }
            }
        } else {
            return Err(AppDatabaseError::FailedToGetConnectionFromPool);
        };
    }

    pub async fn get_txn_by_sig(&self, sig: String) -> Result<models::TxnId, AppDatabaseError> {
        if let Ok(db_conn) = self.connection_pool.get().await {
            let res = db_conn
                .interact(move |conn: &mut MysqlConnection| {
                    diesel::sql_query("SELECT id FROM txns WHERE signature = ?")
                        .bind::<Text, _>(sig)
                        .get_result::<models::TxnId>(conn)
                })
                .await;

            match res {
                Ok(interaction) => match interaction {
                    Ok(query) => {
                        return Ok(query);
                    }
                    Err(e) => {
                        error!(target: "server_log", "{:?}", e);
                        return Err(AppDatabaseError::QueryFailed);
                    }
                },
                Err(e) => {
                    error!(target: "server_log", "{:?}", e);
                    return Err(AppDatabaseError::InteractionFailed);
                }
            }
        } else {
            return Err(AppDatabaseError::FailedToGetConnectionFromPool);
        };
    }

    pub async fn add_new_earnings_batch(
        &self,
        earnings: Vec<models::InsertEarning>,
    ) -> Result<(), AppDatabaseError> {
        if let Ok(db_conn) = self.connection_pool.get().await {
            let res = db_conn
                .interact(move |conn: &mut MysqlConnection| {
                    insert_into(crate::schema::earnings::dsl::earnings)
                        .values(&earnings)
                        .execute(conn)
                })
                .await;

            match res {
                Ok(interaction) => match interaction {
                    Ok(query) => {
                        info!(target: "server_log", "Earnings inserted: {}", query);
                        if query == 0 {
                            return Err(AppDatabaseError::FailedToInsertRow);
                        }
                        return Ok(());
                    }
                    Err(e) => {
                        error!(target: "server_log", "{:?}", e);
                        return Err(AppDatabaseError::QueryFailed);
                    }
                },
                Err(e) => {
                    error!(target: "server_log", "{:?}", e);
                    return Err(AppDatabaseError::InteractionFailed);
                }
            }
        } else {
            return Err(AppDatabaseError::FailedToGetConnectionFromPool);
        };
    }

    pub async fn add_new_submissions_batch(
        &self,
        submissions: Vec<models::InsertSubmission>,
    ) -> Result<(), AppDatabaseError> {
        if let Ok(db_conn) = self.connection_pool.get().await {
            let res = db_conn
                .interact(move |conn: &mut MysqlConnection| {
                    insert_into(crate::schema::submissions::dsl::submissions)
                        .values(&submissions)
                        .execute(conn)
                })
                .await;

            match res {
                Ok(interaction) => match interaction {
                    Ok(query) => {
                        info!(target: "server_log", "Submissions inserted: {}", query);
                        if query == 0 {
                            return Err(AppDatabaseError::FailedToInsertRow);
                        }
                        return Ok(());
                    }
                    Err(e) => {
                        error!(target: "server_log", "{:?}", e);
                        return Err(AppDatabaseError::QueryFailed);
                    }
                },
                Err(e) => {
                    error!(target: "server_log", "{:?}", e);
                    return Err(AppDatabaseError::InteractionFailed);
                }
            }
        } else {
            return Err(AppDatabaseError::FailedToGetConnectionFromPool);
        };
    }

    pub async fn signup_user_transaction(
        &self,
        user_pubkey: String,
        pool_authority_pubkey: String,
    ) -> Result<(), AppDatabaseError> {
        if let Ok(db_conn) = self.connection_pool.get().await {
            let user_pk = user_pubkey.clone();
            let res = db_conn
                .interact(move |conn: &mut MysqlConnection| {
                    let user_pubkey = user_pk;
                    conn.transaction(|conn| {
                        diesel::sql_query("INSERT INTO miners (pubkey, enabled) VALUES (?, ?)")
                            .bind::<Text, _>(&user_pubkey)
                            .bind::<Bool, _>(true)
                            .execute(conn)?;

                        let miner: Miner = diesel::sql_query(
                            "SELECT id, pubkey, enabled FROM miners WHERE miners.pubkey = ?",
                        )
                        .bind::<Text, _>(&user_pubkey)
                        .get_result(conn)?;

                        let pool: models::Pool = diesel::sql_query(
                            "SELECT * FROM pools WHERE pools.authority_pubkey = ?",
                        )
                        .bind::<Text, _>(&pool_authority_pubkey)
                        .get_result(conn)?;

                        diesel::sql_query("INSERT INTO rewards (miner_id, pool_id) VALUES (?, ?)")
                            .bind::<Integer, _>(miner.id)
                            .bind::<Integer, _>(pool.id)
                            .execute(conn)
                    })
                })
                .await;

            match res {
                Ok(interaction) => match interaction {
                    Ok(query) => {
                        if query == 0 {
                            info!(target: "server_log", "Failed to insert signup for pubkey: {}", user_pubkey);
                            return Err(AppDatabaseError::FailedToInsertRow);
                        }
                        info!(target: "server_log", "Successfully inserted signup for pubkey: {}", user_pubkey);
                        return Ok(());
                    }
                    Err(e) => {
                        error!(target: "server_log", "{:?}", e);
                        return Err(AppDatabaseError::QueryFailed);
                    }
                },
                Err(e) => {
                    error!(target: "server_log", "{:?}", e);
                    return Err(AppDatabaseError::InteractionFailed);
                }
            }
        } else {
            return Err(AppDatabaseError::FailedToGetConnectionFromPool);
        };
    }

    pub async fn add_extra_resources_generation(
        &self,
        pool_id: i32,
        generation_type: ExtraResourcesGenerationType,
        linked_challenge_id: Option<i32>,
    ) -> Result<(), AppDatabaseError> {
        if let Ok(db_conn) = self.connection_pool.get().await {
            let res = db_conn
                .interact(move |conn: &mut MysqlConnection| {
                    diesel::sql_query("INSERT INTO extra_resources_generation (pool_id, generation_type, linked_challenge_id) VALUES (?, ?, ?)")
                        .bind::<Integer, _>(pool_id)
                        .bind::<Integer, _>(generation_type as i32)
                        .bind::<Nullable<Integer>, _>(linked_challenge_id)
                        .execute(conn)
                })
                .await;

            match res {
                Ok(interaction) => match interaction {
                    Ok(query) => {
                        info!(target: "server_log", "Extra resources reprocessing inserted: {}", query);
                        return Ok(());
                    }
                    Err(e) => {
                        error!(target: "server_log", "{:?}", e);
                        return Err(AppDatabaseError::QueryFailed);
                    }
                },
                Err(e) => {
                    error!(target: "server_log", "{:?}", e);
                    return Err(AppDatabaseError::InteractionFailed);
                }
            }
        } else {
            return Err(AppDatabaseError::FailedToGetConnectionFromPool);
        };
    }

    pub async fn finish_extra_resources_generation(
        &self,
        id: i32,
        amount_sol: u64,
        amount_coal: u64,
        amount_ore: u64,
        amount_chromium: u64,
        amount_wood: u64,
        amount_ingot: u64,
        generation_type: ExtraResourcesGenerationType,
    ) -> Result<(), AppDatabaseError> {
        if let Ok(db_conn) = self.connection_pool.get().await {
            let res = db_conn
                .interact(move |conn: &mut MysqlConnection| {
                    diesel::sql_query("UPDATE extra_resources_generation SET amount_sol = ?, amount_coal = ?, amount_ore = ?, amount_chromium = ?, amount_wood = ?, amount_ingot = ?, finished_at = now() WHERE id = ? AND generation_type = ?")
                        .bind::<Unsigned<BigInt>, _>(amount_sol)
                        .bind::<Unsigned<BigInt>, _>(amount_coal)
                        .bind::<Unsigned<BigInt>, _>(amount_ore)
                        .bind::<Unsigned<BigInt>, _>(amount_chromium)
                        .bind::<Unsigned<BigInt>, _>(amount_wood)
                        .bind::<Unsigned<BigInt>, _>(amount_ingot)
                        .bind::<Integer, _>(id)
                        .bind::<Integer, _>(generation_type as i32)
                        .execute(conn)
                })
                .await;

            match res {
                Ok(interaction) => match interaction {
                    Ok(query) => {
                        info!(target: "server_log", "Extra resources reprocessing updated: {}", query);
                        return Ok(());
                    }
                    Err(e) => {
                        error!(target: "server_log", "{:?}", e);
                        return Err(AppDatabaseError::QueryFailed);
                    }
                },
                Err(e) => {
                    error!(target: "server_log", "{:?}", e);
                    return Err(AppDatabaseError::InteractionFailed);
                }
            }
        } else {
            return Err(AppDatabaseError::FailedToGetConnectionFromPool);
        };
    }

    pub async fn get_pending_extra_resources_generation(
        &self,
        pool_id: i32,
        generation_type: ExtraResourcesGenerationType,
    ) -> Result<models::ExtraResourcesGeneration, AppDatabaseError> {
        if let Ok(db_conn) = self.connection_pool.get().await {
            let res = db_conn
                .interact(move |conn: &mut MysqlConnection| {
                    diesel::sql_query(
                        "SELECT * FROM extra_resources_generation WHERE finished_at is null AND pool_id = ? AND generation_type = ? ORDER BY created_at DESC",
                    )
                        .bind::<Integer, _>(pool_id)
                        .bind::<Integer, _>(generation_type as i32)
                        .get_result::<models::ExtraResourcesGeneration>(conn)
                })
                .await;

            match res {
                Ok(interaction) => match interaction {
                    Ok(query) => {
                        return Ok(query);
                    }
                    Err(e) => {
                        error!(target: "server_log", "{:?}", e);
                        return Err(AppDatabaseError::QueryFailed);
                    }
                },
                Err(e) => {
                    error!(target: "server_log", "{:?}", e);
                    return Err(AppDatabaseError::InteractionFailed);
                }
            }
        } else {
            return Err(AppDatabaseError::FailedToGetConnectionFromPool);
        };
    }

    pub async fn get_submissions_in_range(
        &self,
        start_time: NaiveDateTime,
        end_time: NaiveDateTime,
    ) -> Result<Vec<models::Submission>, AppDatabaseError> {
        if let Ok(db_conn) = self.connection_pool.get().await {
            let res = db_conn
                .interact(move |conn: &mut MysqlConnection| {
                    diesel::sql_query(
                        "SELECT * FROM submissions WHERE created_at BETWEEN ? AND ? ORDER BY miner_id DESC",
                    )
                        .bind::<Datetime, _>(start_time)
                        .bind::<Datetime, _>(end_time)
                        .get_results::<models::Submission>(conn)
                })
                .await;

            match res {
                Ok(interaction) => match interaction {
                    Ok(query) => {
                        return Ok(query);
                    }
                    Err(e) => {
                        error!(target: "server_log", "get_submissions_in_range {:?}", e);
                        return Err(AppDatabaseError::QueryFailed);
                    }
                },
                Err(e) => {
                    error!(target: "server_log", "get_submissions_in_range {:?}", e);
                    return Err(AppDatabaseError::InteractionFailed);
                }
            }
        } else {
            return Err(AppDatabaseError::FailedToGetConnectionFromPool);
        };
    }

    pub async fn add_new_earnings_extra_resources_batch(
        &self,
        earnings: Vec<models::InsertEarningExtraResources>,
    ) -> Result<(), AppDatabaseError> {
        if let Ok(db_conn) = self.connection_pool.get().await {
            let res = db_conn
                .interact(move |conn: &mut MysqlConnection| {
                    insert_into(
                        crate::schema::earnings_extra_resources::dsl::earnings_extra_resources,
                    )
                    .values(&earnings)
                    .execute(conn)
                })
                .await;

            match res {
                Ok(interaction) => match interaction {
                    Ok(query) => {
                        info!(target: "server_log", "Earnings extra resources inserted: {}", query);
                        if query == 0 {
                            return Err(AppDatabaseError::FailedToInsertRow);
                        }
                        return Ok(());
                    }
                    Err(e) => {
                        error!(target: "server_log", "{:?}", e);
                        return Err(AppDatabaseError::QueryFailed);
                    }
                },
                Err(e) => {
                    error!(target: "server_log", "{:?}", e);
                    return Err(AppDatabaseError::InteractionFailed);
                }
            }
        } else {
            return Err(AppDatabaseError::FailedToGetConnectionFromPool);
        };
    }
}

/*
WITH earnings_totals AS (
    SELECT
        miner_id,
        COALESCE(SUM(amount_coal), 0) AS total_coal,
        COALESCE(SUM(amount_ore), 0) AS total_ore
    FROM earnings
    WHERE miner_id = 2
    GROUP BY miner_id
),
extra_totals AS (
    SELECT
        miner_id,
        COALESCE(SUM(amount_sol), 0) AS total_sol,
        COALESCE(SUM(amount_coal), 0) AS total_coal,
        COALESCE(SUM(amount_ore), 0) AS total_ore,
        COALESCE(SUM(amount_chromium), 0) AS total_chromium,
        COALESCE(SUM(amount_wood), 0) AS total_wood,
        COALESCE(SUM(amount_ingot), 0) AS total_ingot
    FROM earnings_extra_resources
    WHERE miner_id = 2
    GROUP BY miner_id
),
claims_totals AS (
    SELECT
        miner_id,
        COALESCE(SUM(amount_sol), 0) AS total_sol,
        COALESCE(SUM(amount_coal), 0) AS total_coal,
        COALESCE(SUM(amount_ore), 0) AS total_ore,
        COALESCE(SUM(amount_chromium), 0) AS total_chromium,
        COALESCE(SUM(amount_wood), 0) AS total_wood,
        COALESCE(SUM(amount_ingot), 0) AS total_ingot
    FROM claims
    WHERE miner_id = 2
    GROUP BY miner_id
)
SELECT
    m.id AS miner_id,
    COALESCE(et.total_coal, 0) + COALESCE(er.total_coal, 0) - COALESCE(ct.total_coal, 0) AS total_coal,
    COALESCE(et.total_ore, 0) + COALESCE(er.total_ore, 0) - COALESCE(ct.total_ore, 0) AS total_ore,
    COALESCE(er.total_sol, 0) - COALESCE(ct.total_sol, 0) AS total_sol,
    COALESCE(er.total_chromium, 0) - COALESCE(ct.total_chromium, 0) AS total_chromium,
    COALESCE(er.total_wood, 0) - COALESCE(ct.total_wood, 0) AS total_wood,
    COALESCE(er.total_ingot, 0) - COALESCE(ct.total_ingot, 0) AS total_ingot
FROM miners m
LEFT JOIN earnings_totals et ON m.id = et.miner_id
LEFT JOIN extra_totals er ON m.id = er.miner_id
LEFT JOIN claims_totals ct ON m.id = ct.miner_id
WHERE m.id = 2;


select SUM(ern.amount_coal) from earnings ern where ern.miner_id = 2; #302691018911074
select SUM(eer.amount_coal) from earnings_extra_resources eer where eer.miner_id = 2; #181121810792
select SUM(c.amount_coal) from claims c where c.miner_id = 2; #3029700633725

select * from earnings ern where ern.miner_id = 2 AND ern.challenge_id = 99910; #9603311909238
select * from earnings_extra_resources eer where eer.miner_id = 2; #1715506919
select * from claims c where c.miner_id = 2; #5512848693641

SELECT * from rewards rew where rew.miner_id = 2; #299842440088141


SELECT SUM(r.balance_coal) from rewards r;

SELECT SUM(c.rewards_earned_coal) from challenges c;

#------


WITH earnings_totals AS (
    SELECT
        miner_id,
        COALESCE(SUM(amount_coal), 0) AS total_coal,
        COALESCE(SUM(amount_ore), 0) AS total_ore
    FROM earnings
    GROUP BY miner_id
),
extra_totals AS (
    SELECT
        miner_id,
        COALESCE(SUM(amount_sol), 0) AS total_sol,
        COALESCE(SUM(amount_coal), 0) AS total_coal,
        COALESCE(SUM(amount_ore), 0) AS total_ore,
        COALESCE(SUM(amount_chromium), 0) AS total_chromium,
        COALESCE(SUM(amount_wood), 0) AS total_wood,
        COALESCE(SUM(amount_ingot), 0) AS total_ingot
    FROM earnings_extra_resources
    GROUP BY miner_id
),
claims_totals AS (
    SELECT
        miner_id,
        COALESCE(SUM(amount_sol), 0) AS total_sol,
        COALESCE(SUM(amount_coal), 0) AS total_coal,
        COALESCE(SUM(amount_ore), 0) AS total_ore,
        COALESCE(SUM(amount_chromium), 0) AS total_chromium,
        COALESCE(SUM(amount_wood), 0) AS total_wood,
        COALESCE(SUM(amount_ingot), 0) AS total_ingot
    FROM claims
    GROUP BY miner_id
),
miner_rewards AS (
    SELECT
        m.id AS miner_id,
        COALESCE(et.total_coal, 0) + COALESCE(er.total_coal, 0) - COALESCE(ct.total_coal, 0) AS total_coal,
        COALESCE(et.total_ore, 0) + COALESCE(er.total_ore, 0) - COALESCE(ct.total_ore, 0) AS total_ore,
        COALESCE(er.total_sol, 0) - COALESCE(ct.total_sol, 0) AS total_sol,
        COALESCE(er.total_chromium, 0) - COALESCE(ct.total_chromium, 0) AS total_chromium,
        COALESCE(er.total_wood, 0) - COALESCE(ct.total_wood, 0) AS total_wood,
        COALESCE(er.total_ingot, 0) - COALESCE(ct.total_ingot, 0) AS total_ingot
    FROM miners m
    LEFT JOIN earnings_totals et ON m.id = et.miner_id
    LEFT JOIN extra_totals er ON m.id = er.miner_id
    LEFT JOIN claims_totals ct ON m.id = ct.miner_id
)
UPDATE rewards r
JOIN miner_rewards mr ON r.miner_id = mr.miner_id
SET
    r.balance_coal = mr.total_coal,
    r.balance_ore = mr.total_ore,
    r.balance_sol = mr.total_sol,
    r.balance_chromium = mr.total_chromium,
    r.balance_wood = mr.total_wood,
    r.balance_ingot = mr.total_ingot;


select * from earnings e where e.created_at >= '2025-01-20 02:37:00';

UPDATE earnings SET amount_coal = 0 where created_at >= '2025-01-20 02:30:00';
UPDATE earnings_extra_resources SET amount_coal = 0 where created_at >= '2025-01-20 02:30:00';


select SUM(r.balance_coal) from rewards r;
 */
