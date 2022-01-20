table! {
    proofs_of_indexing (indexer, deployment, block_hash) {
        timestamp -> Timestamp,
        indexer -> Varchar,
        deployment -> Varchar,
        block_number -> Int8,
        block_hash -> Nullable<Varchar>,
        proof_of_indexing -> Varchar,
    }
}

table! {
     poi_cross_check_reports (indexer1, indexer2, deployment, block_hash) {
         timestamp -> Timestamp,
         indexer1 -> Varchar,
         indexer2 -> Varchar,
         deployment-> Varchar,
         block_number -> Int8,
         block_hash -> Nullable<Varchar>,
         proof_of_indexing1 -> Varchar,
         proof_of_indexing2 -> Varchar,
     }
}
