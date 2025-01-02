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
            let rewards_1 = rewards.clone();
            let query_1 = db_conn
                .interact(move |conn: &mut MysqlConnection| {
                    let rewards = rewards_1.clone();
                    let query = diesel::sql_query(
                        "UPDATE rewards SET balance_coal = balance_coal + CASE miner_id "
                            .to_string()
                            + &rewards
                                .iter()
                                .map(|r| format!("WHEN {} THEN {}", r.miner_id, r.balance_coal))
                                .collect::<Vec<_>>()
                                .join(" ")
                            + " END WHERE miner_id IN ("
                            + &rewards
                                .iter()
                                .map(|r| r.miner_id.to_string())
                                .collect::<Vec<_>>()
                                .join(",")
                            + ");\n",
                    );
                    query.execute(conn)
                })
                .await;

            let rewards_2 = rewards.clone();
            let query_2 = db_conn
                .interact(move |conn: &mut MysqlConnection| {
                    let rewards = rewards_2.clone();
                    let query = diesel::sql_query(
                        "UPDATE rewards SET balance_ore = balance_ore + CASE miner_id ".to_string()
                            + &rewards
                                .iter()
                                .map(|r| format!("WHEN {} THEN {}", r.miner_id, r.balance_ore))
                                .collect::<Vec<_>>()
                                .join(" ")
                            + " END WHERE miner_id IN ("
                            + &rewards
                                .iter()
                                .map(|r| r.miner_id.to_string())
                                .collect::<Vec<_>>()
                                .join(",")
                            + ");\n",
                    );
                    query.execute(conn)
                })
                .await;

            let rewards_3 = rewards.clone();
            let query_3 = db_conn
                .interact(move |conn: &mut MysqlConnection| {
                    let rewards = rewards_3.clone();
                    let query = diesel::sql_query(
                        "UPDATE rewards SET balance_chromium = balance_chromium + CASE miner_id "
                            .to_string()
                            + &rewards
                                .iter()
                                .map(|r| format!("WHEN {} THEN {}", r.miner_id, r.balance_chromium))
                                .collect::<Vec<_>>()
                                .join(" ")
                            + " END WHERE miner_id IN ("
                            + &rewards
                                .iter()
                                .map(|r| r.miner_id.to_string())
                                .collect::<Vec<_>>()
                                .join(",")
                            + ");\n",
                    );
                    query.execute(conn)
                })
                .await;

            let rewards_4 = rewards.clone();
            let query_4 = db_conn
                .interact(move |conn: &mut MysqlConnection| {
                    let rewards = rewards_4.clone();
                    let query = diesel::sql_query(
                        "UPDATE rewards SET balance_sol = balance_sol + CASE miner_id ".to_string()
                            + &rewards
                                .iter()
                                .map(|r| format!("WHEN {} THEN {}", r.miner_id, r.balance_sol))
                                .collect::<Vec<_>>()
                                .join(" ")
                            + " END WHERE miner_id IN ("
                            + &rewards
                                .iter()
                                .map(|r| r.miner_id.to_string())
                                .collect::<Vec<_>>()
                                .join(",")
                            + ");\n",
                    );
                    query.execute(conn)
                })
                .await;

            let rewards_5 = rewards.clone();
            let query_5 = db_conn
                .interact(move |conn: &mut MysqlConnection| {
                    let rewards = rewards_5.clone();
                    let query = diesel::sql_query(
                        "UPDATE rewards SET balance_wood = balance_wood + CASE miner_id "
                            .to_string()
                            + &rewards
                                .iter()
                                .map(|r| format!("WHEN {} THEN {}", r.miner_id, r.balance_wood))
                                .collect::<Vec<_>>()
                                .join(" ")
                            + " END WHERE miner_id IN ("
                            + &rewards
                                .iter()
                                .map(|r| r.miner_id.to_string())
                                .collect::<Vec<_>>()
                                .join(",")
                            + ");\n",
                    );
                    query.execute(conn)
                })
                .await;

            let rewards_6 = rewards.clone();
            let query_6 = db_conn
                .interact(move |conn: &mut MysqlConnection| {
                    let rewards = rewards_6.clone();
                    let query = diesel::sql_query(
                        "UPDATE rewards SET balance_ingot = balance_ingot + CASE miner_id "
                            .to_string()
                            + &rewards
                                .iter()
                                .map(|r| format!("WHEN {} THEN {}", r.miner_id, r.balance_ingot))
                                .collect::<Vec<_>>()
                                .join(" ")
                            + " END WHERE miner_id IN ("
                            + &rewards
                                .iter()
                                .map(|r| r.miner_id.to_string())
                                .collect::<Vec<_>>()
                                .join(",")
                            + ");\n",
                    );
                    query.execute(conn)
                })
                .await;

            let res = query_1
                .and(query_2)
                .and(query_3)
                .and(query_4)
                .and(query_5)
                .and(query_6);

            match res {
                Ok(interaction) => match interaction {
                    Ok(_query) => {
                        return Ok(());
                    }
                    Err(e) => {
                        error!(target: "server_log", "update rewards query error: {:?}", e);
                        return Err(AppDatabaseError::QueryFailed);
                    }
                },
                Err(e) => {
                    error!(target: "server_log", "update rewards interaction error: {:?}", e);
                    return Err(AppDatabaseError::InteractionFailed);
                }
            }
        } else {
            return Err(AppDatabaseError::FailedToGetConnectionFromPool);
        };
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

    // pub async fn add_new_earning(
    //     &self,
    //     earning: models::InsertEarning,
    // ) -> Result<(), AppDatabaseError> {
    //     if let Ok(db_conn) = self.connection_pool.get().await {
    //         let res = db_conn.interact(move |conn: &mut MysqlConnection| {
    //             diesel::sql_query("INSERT INTO earnings (miner_id, pool_id, challenge_id, amount) VALUES (?, ?, ?, ?)")
    //             .bind::<Integer, _>(earning.miner_id)
    //             .bind::<Integer, _>(earning.pool_id)
    //             .bind::<Integer, _>(earning.challenge_id)
    //             .bind::<Unsigned<BigInt>, _>(earning.amount)
    //             .execute(conn)
    //         }).await;

    //         match res {
    //             Ok(interaction) => match interaction {
    //                 Ok(_query) => {
    //                     return Ok(());
    //                 }
    //                 Err(e) => {
    //                     error!(target: "server_log", "{:?}", e);
    //                     return Err(AppDatabaseError::QueryFailed);
    //                 }
    //             },
    //             Err(e) => {
    //                 error!(target: "server_log", "{:?}", e);
    //                 return Err(AppDatabaseError::InteractionFailed);
    //             }
    //         }
    //     } else {
    //         return Err(AppDatabaseError::FailedToGetConnectionFromPool);
    //     };
    // }

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
