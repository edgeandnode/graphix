use diesel::{r2d2, PgConnection, RunQueryDsl};
use futures::{Stream, StreamExt};
use std::sync::Arc;
use tracing::info;

use crate::proofs_of_indexing::POISummary;

use super::models::ProofOfIndexing;
use super::schema;

/// Write any POIs that we receive to the database.
pub fn write<S>(
    connection_pool: Arc<r2d2::Pool<r2d2::ConnectionManager<PgConnection>>>,
    proofs_of_indexing: S,
) where
    S: Stream<Item = POISummary> + Send + 'static,
{
    tokio::spawn(async move {
        proofs_of_indexing
            .ready_chunks(100)
            .for_each(move |chunk| {
                let connection_pool = connection_pool.clone();
                async move {
                    let pois = chunk
                        .into_iter()
                        .map(|poi_summary| ProofOfIndexing {
                            indexer: poi_summary.indexer.trim_start_matches("0x").into(),
                            deployment: poi_summary.deployment.to_string(),
                            block_number: poi_summary.block_number as i64,
                            block_hash: poi_summary.block_hash.into(),
                            proof_of_indexing: poi_summary.proof_of_indexing.into(),
                        })
                        .collect::<Vec<_>>();

                    let number_of_pois = pois.len();

                    let connection = connection_pool.get().expect("connection");
                    diesel::insert_into(schema::proofs_of_indexing::table)
                        .values(pois)
                        .on_conflict_do_nothing()
                        .execute(&connection)
                        .expect("insertion");

                    info!(%number_of_pois, "Wrote POIs to database");
                }
            })
            .await;
    });
}
