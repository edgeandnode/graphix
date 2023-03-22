CREATE TABLE proofs_of_indexing (
    timestamp TIMESTAMP NOT NULL,
    indexer VARCHAR(255) NOT NULL,
    deployment VARCHAR(46) NOT NULL,
    block_number BIGINT NOT NULL,
    block_hash VARCHAR(64),
    proof_of_indexing VARCHAR(64) NOT NULL,

    PRIMARY KEY (indexer, deployment, block_number)
)
