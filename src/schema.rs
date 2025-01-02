// @generated automatically by Diesel CLI.

diesel::table! {
    challenges (id) {
        id -> Integer,
        pool_id -> Integer,
        submission_id -> Nullable<Integer>,
        #[max_length = 32]
        challenge -> Binary,
        rewards_earned_coal -> Nullable<Unsigned<Bigint>>,
        rewards_earned_ore -> Nullable<Unsigned<Bigint>>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    claims (id) {
        id -> Integer,
        miner_id -> Integer,
        pool_id -> Integer,
        txn_id -> Integer,
        amount_sol -> Unsigned<Bigint>,
        amount_coal -> Unsigned<Bigint>,
        amount_ore -> Unsigned<Bigint>,
        amount_chromium -> Unsigned<Bigint>,
        amount_wood -> Unsigned<Bigint>,
        amount_ingot -> Unsigned<Bigint>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    earnings (id) {
        id -> Integer,
        miner_id -> Integer,
        pool_id -> Integer,
        challenge_id -> Integer,
        amount_coal -> Unsigned<Bigint>,
        amount_ore -> Unsigned<Bigint>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        created_at_day -> Nullable<Date>,
    }
}

diesel::table! {
    earnings_extra_resources (id) {
        id -> Integer,
        miner_id -> Integer,
        pool_id -> Integer,
        extra_resources_generation_id -> Integer,
        amount_sol -> Unsigned<Bigint>,
        amount_coal -> Unsigned<Bigint>,
        amount_ore -> Unsigned<Bigint>,
        amount_chromium -> Unsigned<Bigint>,
        amount_wood -> Unsigned<Bigint>,
        amount_ingot -> Unsigned<Bigint>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        generation_type -> Integer,
    }
}

diesel::table! {
    extra_resources_generation (id) {
        id -> Integer,
        pool_id -> Integer,
        linked_challenge_id -> Nullable<Integer>,
        amount_sol -> Unsigned<Bigint>,
        amount_coal -> Unsigned<Bigint>,
        amount_ore -> Unsigned<Bigint>,
        amount_chromium -> Unsigned<Bigint>,
        amount_wood -> Unsigned<Bigint>,
        amount_ingot -> Unsigned<Bigint>,
        created_at -> Timestamp,
        finished_at -> Nullable<Timestamp>,
        updated_at -> Timestamp,
        generation_type -> Integer,
    }
}

diesel::table! {
    miners (id) {
        id -> Integer,
        #[max_length = 44]
        pubkey -> Varchar,
        enabled -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    pools (id) {
        id -> Integer,
        #[max_length = 44]
        proof_pubkey -> Varchar,
        #[max_length = 44]
        authority_pubkey -> Varchar,
        total_rewards_sol -> Unsigned<Bigint>,
        total_rewards_coal -> Unsigned<Bigint>,
        total_rewards_ore -> Unsigned<Bigint>,
        total_rewards_chromium -> Unsigned<Bigint>,
        total_rewards_wood -> Unsigned<Bigint>,
        total_rewards_ingot -> Unsigned<Bigint>,
        claimed_rewards_sol -> Unsigned<Bigint>,
        claimed_rewards_coal -> Unsigned<Bigint>,
        claimed_rewards_ore -> Unsigned<Bigint>,
        claimed_rewards_chromium -> Unsigned<Bigint>,
        claimed_rewards_wood -> Unsigned<Bigint>,
        claimed_rewards_ingot -> Unsigned<Bigint>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    rewards (id) {
        id -> Integer,
        miner_id -> Integer,
        pool_id -> Integer,
        balance_sol -> Unsigned<Bigint>,
        balance_coal -> Unsigned<Bigint>,
        balance_ore -> Unsigned<Bigint>,
        balance_chromium -> Unsigned<Bigint>,
        balance_wood -> Unsigned<Bigint>,
        balance_ingot -> Unsigned<Bigint>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    submissions (id) {
        id -> Bigint,
        miner_id -> Integer,
        challenge_id -> Integer,
        difficulty -> Tinyint,
        nonce -> Unsigned<Bigint>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        #[max_length = 16]
        digest -> Nullable<Binary>,
    }
}

diesel::table! {
    txns (id) {
        id -> Integer,
        #[max_length = 15]
        txn_type -> Varchar,
        #[max_length = 200]
        signature -> Varchar,
        priority_fee -> Unsigned<Integer>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    challenges,
    claims,
    earnings,
    earnings_extra_resources,
    extra_resources_generation,
    miners,
    pools,
    rewards,
    submissions,
    txns,
);
