CREATE TABLE cached_ethereum_calls (
    indexer2 VARCHAR(255) NOT NULL,
    deployment VARCHAR(46) NOT NULL,
    block_number BIGINT NOT NULL,
    id_hash BYTEA PRIMARY KEY,
    contract_address BYTEA NOT NULL,
    return_value BYTEA NOT NULL
);
