-- This file should undo anything in `up.sql`
drop TABLE earnings_extra_resources;
drop TABLE extra_resources_generation;
ALTER TABLE earnings DROP amount_chromium;
ALTER TABLE pools DROP total_rewards_chromium;
ALTER TABLE pools DROP claimed_rewards_chromium;
ALTER TABLE rewards DROP balance_chromium;