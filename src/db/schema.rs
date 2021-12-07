table! {
    proofs_of_indexing (indexer, deployment, block_number) {
        indexer -> Varchar,
        deployment -> Varchar,
        block_number -> Int8,
        block_hash -> Varchar,
        proof_of_indexing -> Varchar,
    }
}
