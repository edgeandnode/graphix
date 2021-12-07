use super::schema::*;
use diesel::{Insertable, Queryable};

#[derive(Debug, Insertable, Queryable)]
#[table_name = "proofs_of_indexing"]
pub struct ProofOfIndexing {
    pub indexer: String,
    pub deployment: String,
    pub block_number: i64,
    pub block_hash: String,
    pub proof_of_indexing: String,
}
