CREATE TABLE indexer_network_subgraph_entry (
	indexer_id TEXT NOT NULL,
	staked_tokens BIGINT NOT NULL,
	allocated_tokens BIGINT NOT NULL,
	locked_tokens BIGINT NOT NULL,
	query_fees_collected BIGINT NOT NULL,
	delegated_capacity BIGINT NOT NULL,
	available_stake BIGINT NOT NULL
);
