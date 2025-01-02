-- This file should undo anything in `up.sql`
ALTER TABLE extra_resources_generation MODIFY COLUMN amount_chromium BIGINT UNSIGNED DEFAULT 0;