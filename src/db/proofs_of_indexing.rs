use diesel::{r2d2, PgConnection, RunQueryDsl};
use futures::{FutureExt, Stream, StreamExt, TryFutureExt};
use futures_retry::{FutureRetry, RetryPolicy};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

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
            .for_each(move |chunk: Vec<POISummary>| {
                let connection_pool = connection_pool.clone();
                let mut consecutive_errors = 0;

                async move {
                    FutureRetry::new(
                        || async {
                            let pois = chunk
                                .clone()
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

                            let connection = connection_pool.get()?;
                            diesel::insert_into(schema::proofs_of_indexing::table)
                                .values(pois)
                                .on_conflict_do_nothing()
                                .execute(&connection)?;

                            info!(%number_of_pois, "Wrote POIs to database");

                            Ok(()) as Result<_, anyhow::Error>
                        },
                        |e| {
                            if consecutive_errors >= 5 {
                                consecutive_errors = 0;
                                RetryPolicy::ForwardError(e)
                            } else {
                                consecutive_errors += 1;
                                RetryPolicy::WaitRetry(Duration::from_secs(1))
                            }
                        },
                    )
                    .await
                }
                .map_err(|(error, attempts)| {
                    warn!(%error, %attempts, "Failed to write POIs to database");
                })
                .map(|_| ())
            })
            .await;
    });
}
