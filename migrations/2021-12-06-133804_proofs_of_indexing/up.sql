CREATE TABLE proofs_of_indexing (
    indexer VARCHAR(255),
    deployment VARCHAR(46),
    block_number BIGINT,
    block_hash VARCHAR(64) NOT NULL,
    proof_of_indexing VARCHAR(64) NOT NULL,

    PRIMARY KEY (indexer, deployment, block_number)
)
