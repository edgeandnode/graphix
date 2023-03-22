CREATE TABLE poi_cross_check_reports (
    timestamp TIMESTAMP NOT NULL,
    indexer1 VARCHAR(255) NOT NULL,
    indexer2 VARCHAR(255) NOT NULL,
    deployment VARCHAR(46) NOT NULL,
    block_number BIGINT NOT NULL,
    block_hash VARCHAR(64),
    proof_of_indexing1 VARCHAR(64) NOT NULL,
    proof_of_indexing2 VARCHAR(64) NOT NULL,

    PRIMARY KEY (indexer1, indexer2, deployment, block_number)
)
