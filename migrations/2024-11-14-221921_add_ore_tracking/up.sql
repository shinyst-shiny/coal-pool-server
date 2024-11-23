-- Your SQL goes here
ALTER TABLE earnings RENAME COLUMN amount TO amount_coal;
ALTER TABLE earnings ADD COLUMN amount_ore BIGINT UNSIGNED DEFAULT 0 NOT NULL AFTER amount_coal;

ALTER TABLE pools RENAME COLUMN total_rewards TO total_rewards_coal;
ALTER TABLE pools ADD COLUMN total_rewards_ore BIGINT UNSIGNED DEFAULT 0 NOT NULL AFTER total_rewards_coal;

ALTER TABLE pools RENAME COLUMN claimed_rewards TO claimed_rewards_coal;
ALTER TABLE pools ADD COLUMN claimed_rewards_ore BIGINT UNSIGNED DEFAULT 0 NOT NULL AFTER claimed_rewards_coal;

ALTER TABLE challenges RENAME COLUMN rewards_earned TO rewards_earned_coal;
ALTER TABLE challenges ADD COLUMN rewards_earned_ore BIGINT UNSIGNED DEFAULT 0 AFTER rewards_earned_coal;

ALTER TABLE rewards RENAME COLUMN balance TO balance_coal;
ALTER TABLE rewards ADD COLUMN balance_ore BIGINT UNSIGNED DEFAULT 0 NOT NULL AFTER balance_coal;