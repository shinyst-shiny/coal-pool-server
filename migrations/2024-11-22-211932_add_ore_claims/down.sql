-- This file should undo anything in `up.sql`
ALTER TABLE earnings RENAME COLUMN amount_coal TO amount;
ALTER TABLE earnings DROP amount_ore;