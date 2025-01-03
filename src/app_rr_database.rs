use deadpool_diesel::mysql::{Manager, Pool};
use diesel::sql_types::Integer;
use diesel::{sql_types::Text, MysqlConnection, RunQueryDsl};
use tracing::error;

use crate::models::ExtraResourcesGenerationType;
use crate::{
    app_database::AppDatabaseError, models, ChallengeWithDifficulty, Submission,
    SubmissionWithPubkey, Txn,
};

pub struct AppRRDatabase {
    connection_pool: Pool,
}

impl AppRRDatabase {
    pub fn new(url: String) -> Self {
        let manager = Manager::new(url, deadpool_diesel::Runtime::Tokio1);

        let pool = Pool::builder(manager).build().unwrap();

        AppRRDatabase {
            connection_pool: pool,
        }
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
                        error!("get_miner_rewards: {:?}", e);
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

    pub async fn get_last_challenge_submissions(
        &self,
    ) -> Result<Vec<SubmissionWithPubkey>, AppDatabaseError> {
        if let Ok(db_conn) = self.connection_pool.get().await {
            let res = db_conn
                .interact(move |conn: &mut MysqlConnection| {
                    diesel::sql_query("SELECT s.*, m.pubkey FROM submissions s JOIN miners m ON s.miner_id = m.id JOIN challenges c ON s.challenge_id = c.id WHERE c.id = (SELECT id from challenges WHERE rewards_earned_coal IS NOT NULL ORDER BY id DESC LIMIT 1)")
                        .load::<SubmissionWithPubkey>(conn)
                })
                .await;

            match res {
                Ok(interaction) => match interaction {
                    Ok(query) => {
                        return Ok(query);
                    }
                    Err(e) => {
                        error!("{:?}", e);
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

    pub async fn get_miner_submissions(
        &self,
        pubkey: String,
    ) -> Result<Vec<Submission>, AppDatabaseError> {
        if let Ok(db_conn) = self.connection_pool.get().await {
            let res = db_conn
                .interact(move |conn: &mut MysqlConnection| {
                    diesel::sql_query("SELECT s.* FROM submissions s JOIN miners m ON s.miner_id = m.id WHERE m.pubkey = ? ORDER BY s.created_at DESC LIMIT 100")
                        .bind::<Text, _>(pubkey)
                        .load::<Submission>(conn)
                })
                .await;

            match res {
                Ok(interaction) => match interaction {
                    Ok(query) => {
                        return Ok(query);
                    }
                    Err(e) => {
                        error!("{:?}", e);
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

    pub async fn get_challenges(&self) -> Result<Vec<ChallengeWithDifficulty>, AppDatabaseError> {
        if let Ok(db_conn) = self.connection_pool.get().await {
            let res = db_conn
                .interact(move |conn: &mut MysqlConnection| {
                    diesel::sql_query("SELECT c.id, c.rewards_earned_coal, c.rewards_earned_ore, c.updated_at, s.difficulty FROM challenges c JOIN submissions s ON c.submission_id = s.id WHERE c.submission_id IS NOT NULL ORDER BY c.id  DESC LIMIT 1440")
                        .load::<ChallengeWithDifficulty>(conn)
                })
                .await;

            match res {
                Ok(interaction) => match interaction {
                    Ok(query) => {
                        return Ok(query);
                    }
                    Err(e) => {
                        error!("{:?}", e);
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
                        error!("{:?}", e);
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

    pub async fn get_latest_mine_txn(&self) -> Result<Txn, AppDatabaseError> {
        if let Ok(db_conn) = self.connection_pool.get().await {
            let res = db_conn
                .interact(move |conn: &mut MysqlConnection| {
                    diesel::sql_query(
                        "SELECT * FROM txns WHERE txn_type = ? ORDER BY id DESC LIMIT 1",
                    )
                    .bind::<Text, _>("mine")
                    .get_result::<Txn>(conn)
                })
                .await;

            match res {
                Ok(interaction) => match interaction {
                    Ok(query) => {
                        return Ok(query);
                    }
                    Err(e) => {
                        error!("{:?}", e);
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

    pub async fn get_last_claim_by_pubkey(
        &self,
        pubkey: String,
    ) -> Result<models::LastClaim, AppDatabaseError> {
        if let Ok(db_conn) = self.connection_pool.get().await {
            let res = db_conn
                .interact(move |conn: &mut MysqlConnection| {
                    diesel::sql_query("SELECT c.created_at FROM claims c JOIN miners m ON c.miner_id = m.id WHERE m.pubkey = ? ORDER BY c.id DESC LIMIT 1")
                        .bind::<Text, _>(pubkey)
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

    pub async fn get_last_reprocessing(
        &self,
        pool_id: i32,
        generation_type: ExtraResourcesGenerationType,
    ) -> Result<models::ExtraResourcesGeneration, AppDatabaseError> {
        if let Ok(db_conn) = self.connection_pool.get().await {
            let res = db_conn
                .interact(move |conn: &mut MysqlConnection| {
                    diesel::sql_query(
                        "SELECT * FROM extra_resources_generation WHERE finished_at is not null AND pool_id = ? AND generation_type = ? ORDER BY created_at DESC",
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

    pub async fn get_extra_resources_rewards_by_pubkey(
        &self,
        pubkey: String,
        generation_type: ExtraResourcesGenerationType,
    ) -> Result<models::EarningExtraResources, AppDatabaseError> {
        if let Ok(db_conn) = self.connection_pool.get().await {
            let res = db_conn
                .interact(move |conn: &mut MysqlConnection| {
                    diesel::sql_query("SELECT
                                                0 as id,
                                                m.id as miner_id,
                                                eer.pool_id,
                                                0 as extra_resources_generation_id,
                                                SUM(eer.amount_sol) as amount_sol,
                                                SUM(eer.amount_coal) as amount_coal,
                                                SUM(eer.amount_ore) as amount_ore,
                                                SUM(eer.amount_chromium) as amount_chromium,
                                                SUM(eer.amount_wood) as amount_wood,
                                                SUM(eer.amount_ingot) as amount_ingot,
                                                MAX(eer.created_at) as created_at,
                                                MAX(eer.updated_at) as updated_at,
                                                eer.generation_type
                                            FROM earnings_extra_resources eer
                                            JOIN miners m ON eer.miner_id = m.id
                                            WHERE m.pubkey = ? AND eer.generation_type = ?
                                            GROUP BY m.id, m.pubkey, eer.generation_type, eer.pool_id
                                        ")
                        .bind::<Text, _>(pubkey)
                        .bind::<Integer, _>(generation_type as i32)
                        .get_result::<models::EarningExtraResources>(conn)
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

    pub async fn get_extra_resources_rewards_24h_by_pubkey(
        &self,
        pubkey: String,
        generation_type: ExtraResourcesGenerationType,
    ) -> Result<models::EarningExtraResources, AppDatabaseError> {
        if let Ok(db_conn) = self.connection_pool.get().await {
            let res = db_conn
                .interact(move |conn: &mut MysqlConnection| {
                    diesel::sql_query("
                                            SELECT
                                                0 as id,
                                                m.id as miner_id,
                                                eer.pool_id,
                                                0 as extra_resources_generation_id,
                                                SUM(eer.amount_sol) as amount_sol,
                                                SUM(eer.amount_coal) as amount_coal,
                                                SUM(eer.amount_ore) as amount_ore,
                                                SUM(eer.amount_chromium) as amount_chromium,
                                                SUM(eer.amount_wood) as amount_wood,
                                                SUM(eer.amount_ingot) as amount_ingot,
                                                MAX(eer.created_at) as created_at,
                                                MAX(eer.updated_at) as updated_at,
                                                eer.generation_type
                                            FROM earnings_extra_resources eer
                                            JOIN miners m ON eer.miner_id = m.id
                                            WHERE m.pubkey = ? AND eer.generation_type = ? AND eer.created_at >= DATE_SUB(NOW(), INTERVAL 24 HOUR)
                                            GROUP BY m.id, m.pubkey, eer.generation_type, eer.pool_id
                                            ")
                        .bind::<Text, _>(pubkey)
                        .bind::<Integer, _>(generation_type as i32)
                        .get_result::<models::EarningExtraResources>(conn)
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
}
