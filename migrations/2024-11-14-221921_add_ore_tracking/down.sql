-- This file should undo anything in `up.sql`
ALTER TABLE earnings RENAME COLUMN amount_coal TO amount;
ALTER TABLE earnings DROP amount_ore;

ALTER TABLE pools RENAME COLUMN total_rewards_coal TO total_rewards;
ALTER TABLE pools DROP total_rewards_ore;

ALTER TABLE pools RENAME COLUMN claimed_rewards_coal TO claimed_rewards;
ALTER TABLE pools DROP claimed_rewards_ore;

ALTER TABLE challenges RENAME COLUMN rewards_earned_coal TO rewards_earned;
ALTER TABLE challenges DROP rewards_earned_ore;

ALTER TABLE rewards RENAME COLUMN balance_coal TO balance;
ALTER TABLE rewards DROP balance_ore;