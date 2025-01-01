-- This file should undo anything in `up.sql`

-- Undo changes to claims table
ALTER TABLE claims DROP COLUMN amount_sol;
ALTER TABLE claims DROP COLUMN amount_wood;
ALTER TABLE claims DROP COLUMN amount_ingot;

-- Undo changes to earnings_extra_resources table
ALTER TABLE earnings_extra_resources DROP COLUMN amount_sol;
ALTER TABLE earnings_extra_resources DROP COLUMN amount_coal;
ALTER TABLE earnings_extra_resources DROP COLUMN amount_ore;
ALTER TABLE earnings_extra_resources DROP COLUMN amount_wood;
ALTER TABLE earnings_extra_resources DROP COLUMN amount_ingot;
ALTER TABLE earnings_extra_resources DROP COLUMN generation_type;

-- Undo changes to extra_resources_generation table
ALTER TABLE extra_resources_generation DROP COLUMN amount_sol;
ALTER TABLE extra_resources_generation DROP COLUMN amount_coal;
ALTER TABLE extra_resources_generation DROP COLUMN amount_ore;
ALTER TABLE extra_resources_generation DROP COLUMN amount_wood;
ALTER TABLE extra_resources_generation DROP COLUMN amount_ingot;
ALTER TABLE extra_resources_generation DROP COLUMN generation_type;

-- Undo changes to pools table
ALTER TABLE pools DROP COLUMN total_rewards_sol;
ALTER TABLE pools DROP COLUMN total_rewards_wood;
ALTER TABLE pools DROP COLUMN total_rewards_ingot;
ALTER TABLE pools DROP COLUMN claimed_rewards_sol;
ALTER TABLE pools DROP COLUMN claimed_rewards_wood;
ALTER TABLE pools DROP COLUMN claimed_rewards_ingot;

-- Undo changes to rewards table
ALTER TABLE rewards DROP COLUMN balance_sol;
ALTER TABLE rewards DROP COLUMN balance_wood;
ALTER TABLE rewards DROP COLUMN balance_ingot;