use std::time::Duration;

use futures::{FutureExt, Stream, StreamExt, TryFutureExt};
use futures_retry::{FutureRetry, RetryPolicy};
use tracing::warn;

use crate::types;

use super::{PoiLiveness, Store};

/// Write any POIs that we receive to the database.
pub fn write<S>(store: Store, proofs_of_indexing: S)
where
    S: Stream<Item = types::ProofOfIndexing> + Send + 'static,
{
    tokio::spawn(async move {
        proofs_of_indexing
            .ready_chunks(100)
            .for_each(move |chunk: Vec<types::ProofOfIndexing>| {
                let store = store.clone();
                let mut consecutive_errors = 0;

                async move {
                    FutureRetry::new(
                        || async {
                            // TODO: Asyncify this instead of blocking
                            store.write_pois(&chunk, PoiLiveness::NotLive)
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

/// Write any POI cross-check reports that we receive to the database.
pub fn write_reports<S>(_store: Store, _reports: S)
where
    S: Stream<Item = types::POICrossCheckReport> + Send + 'static,
{
    // TODO writing cross check reports to the database not implemented
    // tokio::spawn(async move {
    //     reports
    //         .ready_chunks(100)
    //         .for_each(move |chunk: Vec<types::POICrossCheckReport<I>>| {
    //             let store = store.clone();
    //             let mut consecutive_errors = 0;

    //             async move {
    //                 FutureRetry::new(
    //                     || async {
    //                         let reports = chunk
    //                             .clone()
    //                             .into_iter()
    //                             .map(|report| PoiCrossCheckReport {
    //                                 timestamp: Utc::now().naive_utc(),
    //                                 indexer1: report
    //                                     .poi1
    //                                     .indexer
    //                                     .id()
    //                                     .trim_start_matches("0x")
    //                                     .into(),
    //                                 indexer2: report
    //                                     .poi2
    //                                     .indexer
    //                                     .id()
    //                                     .trim_start_matches("0x")
    //                                     .into(),
    //                                 deployment: report.poi1.deployment.to_string(),
    //                                 block_hash: report.poi1.block.hash.map(|hash| hash.to_string()),
    //                                 block_number: report.poi1.block.number as i64,
    //                                 proof_of_indexing1: report.poi1.proof_of_indexing.to_string(),
    //                                 proof_of_indexing2: report.poi2.proof_of_indexing.to_string(),
    //                                 diverging_block: report.diverging_block.map(From::from),
    //                             })
    //                             .collect::<Vec<_>>();

    //                         store.write_poi_cross_check_reports(reports)
    //                     },
    //                     |e| {
    //                         if consecutive_errors >= 5 {
    //                             consecutive_errors = 0;
    //                             RetryPolicy::ForwardError(e)
    //                         } else {
    //                             consecutive_errors += 1;
    //                             RetryPolicy::WaitRetry(Duration::from_secs(1))
    //                         }
    //                     },
    //                 )
    //                 .await
    //             }
    //             .map_err(|(error, attempts)| {
    //                 warn!(%error, %attempts, "Failed to write POI cross-check reports to database");
    //             })
    //             .map(|_| ())
    //         })
    //         .await;
    // });
}
