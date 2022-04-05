table! {
    proofs_of_indexing (indexer, deployment, block_number) {
        timestamp -> Timestamp,
        indexer -> Varchar,
        deployment -> Varchar,
        block_number -> Int8,
        block_hash -> Nullable<Varchar>,
        block_contents -> Jsonb,
        proof_of_indexing -> Varchar,
        entity_deletions -> Jsonb,
        entity_updates -> Jsonb,
    }
}

table! {
    cached_ethereum_calls (id_hash) {
        indexer -> Varchar,
        deployment -> Varchar,
        block_number -> Int8,
        id_hash -> Binary,
        contract_address -> Binary,
        return_value -> Binary,
    }
}

table! {
    poi_cross_check_reports (indexer1, indexer2, deployment, block_number) {
        timestamp -> Timestamp,
        indexer1 -> Varchar,
        indexer2 -> Varchar,
        deployment-> Varchar,
        block_number -> Int8,
        block_hash -> Nullable<Varchar>,
        proof_of_indexing1 -> Varchar,
        proof_of_indexing2 -> Varchar,
        diverging_block -> Nullable<Jsonb>,
    }
}
