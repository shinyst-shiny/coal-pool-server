-- Your SQL goes here
ALTER TABLE earnings ADD COLUMN difficulty TINYINT NOT NULL DEFAULT 0 AFTER amount_ore;