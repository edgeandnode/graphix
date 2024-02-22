-- Your SQL goes here
CREATE TABLE failed_queries (
	id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
	indexer_id INTEGER NOT NULL REFERENCES indexers(id) ON DELETE CASCADE,
	query_name TEXT NOT NULL,
	raw_query TEXT NOT NULL,
	response TEXT NOT NULL,
	request_timestamp TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX ON failed_queries (indexer_id, query_name);
