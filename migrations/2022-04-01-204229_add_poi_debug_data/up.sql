ALTER TABLE proofs_of_indexing
ADD COLUMN block_contents JSONB NOT NULL DEFAULT 'null',
ADD COLUMN entity_deletions JSONB NOT NULL DEFAULT '{}',
ADD COLUMN entity_updates JSONB NOT NULL DEFAULT '{}';
