-- This file should undo anything in `up.sql`
ALTER TABLE
	indexers
ALTER COLUMN
	address DROP NOT NULL;
