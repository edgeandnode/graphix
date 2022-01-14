CREATE TABLE poi_cross_check_reports (
    timestamp TIMESTAMP,
    indexer1 VARCHAR(255),
    indexer2 VARCHAR(255),
    deployment VARCHAR(46),
    block_number BIGINT,
    block_hash VARCHAR(64) NOT NULL,
    proof_of_indexing1 VARCHAR(64) NOT NULL,
    proof_of_indexing2 VARCHAR(64) NOT NULL,

    PRIMARY KEY (indexer1, indexer2, deployment, block_hash)
)
