-- Your SQL goes here

-- claims
ALTER TABLE claims ADD COLUMN amount_sol BIGINT UNSIGNED DEFAULT 0 NOT NULL AFTER txn_id;
ALTER TABLE claims ADD COLUMN amount_wood BIGINT UNSIGNED DEFAULT 0 NOT NULL AFTER amount_chromium;
ALTER TABLE claims ADD COLUMN amount_ingot BIGINT UNSIGNED DEFAULT 0 NOT NULL AFTER amount_wood;

-- earnings_extra_resources
ALTER TABLE earnings_extra_resources ADD COLUMN amount_sol BIGINT UNSIGNED DEFAULT 0 NOT NULL AFTER extra_resources_generation_id;
ALTER TABLE earnings_extra_resources ADD COLUMN amount_coal BIGINT UNSIGNED DEFAULT 0 NOT NULL AFTER amount_sol;
ALTER TABLE earnings_extra_resources ADD COLUMN amount_ore BIGINT UNSIGNED DEFAULT 0 NOT NULL AFTER amount_coal;
ALTER TABLE earnings_extra_resources ADD COLUMN amount_wood BIGINT UNSIGNED DEFAULT 0 NOT NULL AFTER amount_chromium;
ALTER TABLE earnings_extra_resources ADD COLUMN amount_ingot BIGINT UNSIGNED DEFAULT 0 NOT NULL AFTER amount_wood;
ALTER TABLE earnings_extra_resources ADD COLUMN generation_type BIGINT UNSIGNED DEFAULT 1 NOT NULL AFTER updated_at;

-- extra_resources_generation
ALTER TABLE extra_resources_generation ADD COLUMN amount_sol BIGINT UNSIGNED DEFAULT 0 NOT NULL AFTER pool_id;
ALTER TABLE extra_resources_generation ADD COLUMN amount_coal BIGINT UNSIGNED DEFAULT 0 NOT NULL AFTER amount_sol;
ALTER TABLE extra_resources_generation ADD COLUMN amount_ore BIGINT UNSIGNED DEFAULT 0 NOT NULL AFTER amount_coal;
ALTER TABLE extra_resources_generation ADD COLUMN amount_wood BIGINT UNSIGNED DEFAULT 0 NOT NULL AFTER amount_chromium;
ALTER TABLE extra_resources_generation ADD COLUMN amount_ingot BIGINT UNSIGNED DEFAULT 0 NOT NULL AFTER amount_wood;
ALTER TABLE extra_resources_generation ADD COLUMN generation_type BIGINT UNSIGNED DEFAULT 1 NOT NULL AFTER updated_at;

-- pools
ALTER TABLE pools ADD COLUMN total_rewards_sol BIGINT UNSIGNED DEFAULT 0 NOT NULL AFTER authority_pubkey;
ALTER TABLE pools ADD COLUMN total_rewards_wood BIGINT UNSIGNED DEFAULT 0 NOT NULL AFTER total_rewards_chromium;
ALTER TABLE pools ADD COLUMN total_rewards_ingot BIGINT UNSIGNED DEFAULT 0 NOT NULL AFTER total_rewards_wood;
ALTER TABLE pools ADD COLUMN claimed_rewards_sol BIGINT UNSIGNED DEFAULT 0 NOT NULL AFTER total_rewards_ingot;
ALTER TABLE pools ADD COLUMN claimed_rewards_wood BIGINT UNSIGNED DEFAULT 0 NOT NULL AFTER claimed_rewards_chromium;
ALTER TABLE pools ADD COLUMN claimed_rewards_ingot BIGINT UNSIGNED DEFAULT 0 NOT NULL AFTER claimed_rewards_wood;

-- rewards
ALTER TABLE rewards ADD COLUMN balance_sol BIGINT UNSIGNED DEFAULT 0 NOT NULL AFTER pool_id;
ALTER TABLE rewards ADD COLUMN balance_wood BIGINT UNSIGNED DEFAULT 0 NOT NULL AFTER balance_chromium;
ALTER TABLE rewards ADD COLUMN balance_ingot BIGINT UNSIGNED DEFAULT 0 NOT NULL AFTER balance_wood;