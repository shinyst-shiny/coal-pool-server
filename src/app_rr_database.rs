use chrono::{NaiveDate, NaiveDateTime};
use deadpool_diesel::mysql::{Manager, Pool};
use diesel::mysql::Mysql;
use diesel::sql_types::{BigInt, Date, Datetime, Integer};
use diesel::{sql_types::Text, MysqlConnection, RunQueryDsl};
use tracing::{error, info};

use crate::{
    app_database::AppDatabaseError, models, ChallengeWithDifficulty, Submission,
    SubmissionWithPubkey, Txn, MIN_DIFF, MIN_HASHPOWER,
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
                    diesel::sql_query(
                        "SELECT
                        c.id,
                        c.rewards_earned_coal,
                        c.rewards_earned_ore,
                        s.difficulty,
                        ? * POW(2, s.difficulty - ?) as challenge_hashpower,
                        c.updated_at
                    FROM challenges c
                    JOIN submissions s ON c.submission_id = s.id
                    WHERE c.submission_id IS NOT NULL ORDER BY c.id  DESC LIMIT 1440
                    ",
                    )
                    .bind::<BigInt, _>(MIN_HASHPOWER as i64)
                    .bind::<Integer, _>(MIN_DIFF as i32)
                    .load::<ChallengeWithDifficulty>(conn)
                })
                .await;

            match res {
                Ok(interaction) => match interaction {
                    Ok(query) => {
                        return Ok(query);
                    }
                    Err(e) => {
                        error!("{:?} -->", e);
                        return Err(AppDatabaseError::QueryFailed);
                    }
                },
                Err(e) => {
                    error!("{:?} -->", e);
                    return Err(AppDatabaseError::InteractionFailed);
                }
            }
        } else {
            error!("--> failed connection");
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
        generation_type: models::ExtraResourcesGenerationType,
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
    pub async fn get_extra_resources_rewards_for_id_by_pubkey(
        &self,
        pubkey: String,
        extra_resources_generation_id: i32,
    ) -> Result<models::EarningExtraResources, AppDatabaseError> {
        if let Ok(db_conn) = self.connection_pool.get().await {
            let res = db_conn
                .interact(move |conn: &mut MysqlConnection| {
                    diesel::sql_query("SELECT
                                               eer.*
                                            FROM earnings_extra_resources eer
                                            JOIN miners m ON eer.miner_id = m.id
                                            WHERE m.pubkey = ? AND eer.extra_resources_generation_id = ?
                                        ")
                        .bind::<Text, _>(pubkey)
                        .bind::<Integer, _>(extra_resources_generation_id)
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

    pub async fn get_extra_resources_rewards_by_pubkey(
        &self,
        pubkey: String,
        generation_type: models::ExtraResourcesGenerationType,
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

    pub async fn get_extra_resources_rewards_in_period_by_pubkey(
        &self,
        pubkey: String,
        generation_type: models::ExtraResourcesGenerationType,
        start_date: NaiveDateTime,
        end_date: NaiveDateTime,
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
                                            WHERE m.pubkey = ? AND eer.generation_type = ?
                                                AND eer.created_at >=? AND eer.created_at <= ?
                                            GROUP BY m.id, m.pubkey, eer.generation_type, eer.pool_id
                                            ")
                        .bind::<Text, _>(pubkey)
                        .bind::<Integer, _>(generation_type as i32)
                        .bind::<Datetime, _>(start_date)
                        .bind::<Datetime, _>(end_date)
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

    pub async fn get_extra_resources_rewards_in_period(
        &self,
        generation_type: models::ExtraResourcesGenerationType,
        start_date: NaiveDateTime,
        end_date: NaiveDateTime,
    ) -> Result<models::EarningExtraResources, AppDatabaseError> {
        if let Ok(db_conn) = self.connection_pool.get().await {
            let res = db_conn
                .interact(move |conn: &mut MysqlConnection| {
                    diesel::sql_query("
                                            SELECT
                                                0 as id,
                                                0 as miner_id,
                                                eer.pool_id,
                                                eer.generation_type as extra_resources_generation_id,
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
                                            WHERE eer.generation_type = ? AND eer.created_at >= ? AND eer.created_at <= ?
                                            GROUP BY eer.generation_type, eer.pool_id
                                            ")
                        .bind::<Integer, _>(generation_type as i32)
                        .bind::<Datetime, _>(start_date)
                        .bind::<Datetime, _>(end_date)
                        .get_result::<models::EarningExtraResources>(conn)
                })
                .await;

            println!(
                "get_extra_resources_rewards_in_period {:?} {:?} {:?}",
                generation_type, start_date, end_date
            );
            println!("get_extra_resources_rewards_in_period {:?}", res);

            match res {
                Ok(interaction) => match interaction {
                    Ok(query) => {
                        return Ok(query);
                    }
                    Err(diesel::result::Error::NotFound) => {
                        // Return a default value if nothing is found
                        return Ok(models::EarningExtraResources {
                            id: 0,
                            miner_id: 0,
                            pool_id: 0,
                            extra_resources_generation_id: 0,
                            amount_sol: 0,
                            amount_coal: 0,
                            amount_ore: 0,
                            amount_chromium: 0,
                            amount_wood: 0,
                            amount_ingot: 0,
                            created_at: start_date,
                            updated_at: end_date,
                            generation_type: generation_type as i32,
                        });
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

    pub async fn get_earnings_by_pubkey(
        &self,
        pubkey: String,
    ) -> Result<models::Earning, AppDatabaseError> {
        if let Ok(db_conn) = self.connection_pool.get().await {
            let res = db_conn
                .interact(move |conn: &mut MysqlConnection| {
                    diesel::sql_query(
                        "SELECT
                                                0 as id,
                                                m.id as miner_id,
                                                e.pool_id,
                                                0 as challenge_id,
                                                SUM(e.amount_coal) as amount_coal,
                                                SUM(e.amount_ore) as amount_ore,
                                                0 as difficulty,
                                                MAX(e.created_at) as created_at,
                                                MAX(e.updated_at) as updated_at
                                            FROM earnings e
                                            JOIN miners m ON e.miner_id = m.id
                                            WHERE m.pubkey = ?
                                            GROUP BY m.id, m.pubkey, e.pool_id
                                        ",
                    )
                    .bind::<Text, _>(pubkey)
                    .get_result::<models::Earning>(conn)
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

    pub async fn get_earning_in_period_by_pubkey(
        &self,
        pubkey: String,
        start_date: NaiveDateTime,
        end_date: NaiveDateTime,
    ) -> Result<models::Earning, AppDatabaseError> {
        if let Ok(db_conn) = self.connection_pool.get().await {
            let res = db_conn
                .interact(move |conn: &mut MysqlConnection| {
                    diesel::sql_query(
                        "
                                            SELECT
                                                0 as id,
                                                m.id as miner_id,
                                                e.pool_id,
                                                0 as challenge_id,
                                                SUM(e.amount_coal) as amount_coal,
                                                SUM(e.amount_ore) as amount_ore,
                                                0 as difficulty,
                                                MAX(e.created_at) as created_at,
                                                MAX(e.updated_at) as updated_at
                                            FROM earnings e
                                            JOIN miners m ON e.miner_id = m.id
                                            WHERE m.pubkey = ?
                                                AND e.challenge_id != -1
                                                AND e.created_at >= ? AND e.created_at <= ?
                                            GROUP BY m.id, m.pubkey, e.pool_id
                                            ",
                    )
                    .bind::<Text, _>(pubkey)
                    .bind::<Datetime, _>(start_date)
                    .bind::<Datetime, _>(end_date)
                    .get_result::<models::Earning>(conn)
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

    pub async fn get_earnings_with_challenge_and_submission(
        &self,
        pubkey: String,
        start_time: NaiveDateTime,
        end_time: NaiveDateTime,
    ) -> Result<Vec<models::EarningWithChallengeWithSubmission>, AppDatabaseError> {
        if let Ok(db_conn) = self.connection_pool.get().await {
            let res = db_conn
                .interact(move |conn: &mut MysqlConnection| {
                    diesel::sql_query(
                        "
                WITH challenge_best_difficulty AS (
                    SELECT challenge_id, MAX(difficulty) as best_difficulty
                    FROM earnings
                    WHERE created_at BETWEEN ? AND ?
                        AND challenge_id != -1
                    GROUP BY challenge_id
                )
                SELECT
                    e.miner_id,
                    e.challenge_id,
                    m.pubkey,
                    e.amount_coal as miner_amount_coal,
                    e.amount_ore as miner_amount_ore,
                    cbd.best_difficulty,
                    e.difficulty as miner_difficulty,
                    ? * POW(2, e.difficulty - ?) as miner_hashpower,
                    ? * POW(2, cbd.best_difficulty - ?) as best_challenge_hashpower,
                    e.created_at,
                    c.rewards_earned_coal as total_rewards_earned_coal,
                    c.rewards_earned_ore as total_rewards_earned_ore
                FROM
                    earnings e
                JOIN
                    miners m ON e.miner_id = m.id
                JOIN
                    challenges c ON e.challenge_id = c.id
                JOIN
                    challenge_best_difficulty cbd ON e.challenge_id = cbd.challenge_id
                WHERE
                    m.pubkey = ?
                    AND e.created_at BETWEEN ? AND ?
                    AND e.challenge_id != -1
                ORDER BY
                    e.created_at DESC
                ",
                    )
                    .bind::<Datetime, _>(start_time)
                    .bind::<Datetime, _>(end_time)
                    .bind::<BigInt, _>(MIN_HASHPOWER as i64)
                    .bind::<Integer, _>(MIN_DIFF as i32)
                    .bind::<BigInt, _>(MIN_HASHPOWER as i64)
                    .bind::<Integer, _>(MIN_DIFF as i32)
                    .bind::<Text, _>(pubkey)
                    .bind::<Datetime, _>(start_time)
                    .bind::<Datetime, _>(end_time)
                    .get_results::<models::EarningWithChallengeWithSubmission>(conn)
                })
                .await;

            match res {
                Ok(interaction) => match interaction {
                    Ok(query) => {
                        return Ok(query);
                    }
                    Err(e) => {
                        error!("get_earnings_with_challenge_and_submission: {:?}", e);
                        return Err(AppDatabaseError::QueryFailed);
                    }
                },
                Err(e) => {
                    error!("get_earnings_with_challenge_and_submission {:?}", e);
                    return Err(AppDatabaseError::InteractionFailed);
                }
            }
        } else {
            return Err(AppDatabaseError::FailedToGetConnectionFromPool);
        }
    }

    pub async fn get_average_connected_miners_24h(
        &self,
    ) -> Result<models::ConnectedMiners, AppDatabaseError> {
        if let Ok(db_conn) = self.connection_pool.get().await {
            let res = db_conn
                .interact(move |conn: &mut MysqlConnection| {
                    diesel::sql_query(
                        "
                        SELECT COALESCE(AVG(connected_miners),0) as average_connected_miners
                        FROM (
                            SELECT COUNT(miner_id) as connected_miners
                            FROM submissions
                            WHERE created_at >= NOW() - INTERVAL 24 HOUR
                            GROUP BY challenge_id
                        ) AS daily_connected_miners
                        ",
                    )
                    .get_result::<models::ConnectedMiners>(conn)
                })
                .await;

            match res {
                Ok(interaction) => match interaction {
                    Ok(query) => {
                        return Ok(query);
                    }
                    Err(e) => {
                        error!("get_average_connected_miners_24h: {:?}", e);
                        return Err(AppDatabaseError::QueryFailed);
                    }
                },
                Err(e) => {
                    error!("get_average_connected_miners_24h {:?}", e);
                    return Err(AppDatabaseError::InteractionFailed);
                }
            }
        } else {
            return Err(AppDatabaseError::FailedToGetConnectionFromPool);
        }
    }

    pub async fn get_difficulty_distribution_24h(
        &self,
    ) -> Result<Vec<models::DifficultyDistribution>, AppDatabaseError> {
        if let Ok(db_conn) = self.connection_pool.get().await {
            let res = db_conn
                .interact(move |conn: &mut MysqlConnection| {
                    diesel::sql_query(
                        "
                    WITH total_submissions AS (
                        SELECT COUNT(*) as total FROM submissions
                        WHERE created_at >= NOW() - INTERVAL 24 HOUR
                    )
                    SELECT
                        s.difficulty,
                        COUNT(*) as count,
                        (COUNT(*) * 100.0 / (SELECT total FROM total_submissions)) as percentage
                    FROM
                        submissions s
                    WHERE
                        s.created_at >= NOW() - INTERVAL 24 HOUR
                    GROUP BY
                        s.difficulty
                    ORDER BY
                        s.difficulty
                    ",
                    )
                    .load::<models::DifficultyDistribution>(conn)
                })
                .await;

            match res {
                Ok(interaction) => match interaction {
                    Ok(query) => {
                        return Ok(query);
                    }
                    Err(e) => {
                        error!("get_difficulty_distribution_24h: {:?}", e);
                        return Err(AppDatabaseError::QueryFailed);
                    }
                },
                Err(e) => {
                    error!("get_difficulty_distribution_24h: {:?}", e);
                    return Err(AppDatabaseError::InteractionFailed);
                }
            }
        } else {
            return Err(AppDatabaseError::FailedToGetConnectionFromPool);
        }
    }
}
