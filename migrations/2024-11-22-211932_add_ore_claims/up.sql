-- Your SQL goes here
ALTER TABLE claims RENAME COLUMN amount TO amount_coal;
ALTER TABLE claims ADD COLUMN amount_ore BIGINT UNSIGNED DEFAULT 0 NOT NULL AFTER amount_coal;