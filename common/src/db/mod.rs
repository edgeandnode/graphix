use chrono::Utc;
use diesel::{r2d2, PgConnection, RunQueryDsl};
use futures::{FutureExt, Stream, StreamExt, TryFutureExt};
use futures_retry::{FutureRetry, RetryPolicy};
use std::{ops::RangeInclusive, path::Path, time::Duration};
use tracing::{info, warn};

use crate::{indexer::Indexer, types};

pub mod models;
pub mod schema;

type ConnectionPool = r2d2::Pool<r2d2::ConnectionManager<PgConnection>>;

const WRITE_TO_DB_CHUNK_SIZE: usize = 100;
const WRITE_TO_DB_RETRY_BACKOFF: Duration = Duration::from_secs(1);
const WRITE_TO_DB_RETRY_MAX_TIMES: u32 = 5;

embed_migrations!("../migrations");

#[derive(Clone)]
pub struct Store {
    pub connection_pool: ConnectionPool,
}

impl Store {
    pub fn new(db_url: impl AsRef<Path>) -> anyhow::Result<Self> {
        let db_url = db_url.as_ref().to_string_lossy().to_owned();
        let connection_manager = r2d2::ConnectionManager::<PgConnection>::new(db_url);

        info!("Connect to database");
        let connection_pool = r2d2::Builder::new().build(connection_manager)?;

        let store = Store { connection_pool };
        store.run_migrations()?;
        Ok(store)
    }

    fn run_migrations(&self) -> anyhow::Result<()> {
        info!("Run database migrations");
        let connection = self.connection_pool.get()?;
        embedded_migrations::run(&connection)?;
        Ok(())
    }

    fn write_items_to_db<S, F, G>(self, items: S, db_insert: F, on_err: G)
    where
        S: Stream + Send + 'static,
        S::Item: Send + Sync + Clone + 'static,
        F: Fn(Self, Vec<S::Item>) -> anyhow::Result<()> + Send + Sync + Copy + 'static,
        G: Fn((anyhow::Error, usize)) + Send + Sync + Copy + 'static,
    {
        tokio::spawn(async move {
            items
                .ready_chunks(WRITE_TO_DB_CHUNK_SIZE)
                .for_each(move |chunk: Vec<S::Item>| {
                    let mut consecutive_errors = 0;
                    let store = self.clone();

                    async move {
                        FutureRetry::new(
                            || async { db_insert(store.clone(), chunk.clone()) },
                            |e| retry_policy(e, &mut consecutive_errors),
                        )
                        .await
                    }
                    .map_err(on_err)
                    .map(|_| ())
                })
                .await;
        });
    }

    fn insert_pois<I>(&self, pois: Vec<types::ProofOfIndexing<I>>) -> anyhow::Result<()>
    where
        I: Indexer,
    {
        let pois = pois
            .into_iter()
            .map(|poi| models::ProofOfIndexing {
                timestamp: Utc::now().naive_utc(),
                indexer: poi.indexer.id().trim_start_matches("0x").into(),
                deployment: poi.deployment.deployment_id,
                block_number: poi.block.number as i64,
                block_hash: poi.block.hash.map(|hash| hash.into()),
                block_contents: poi.debug_data.block_contents,
                proof_of_indexing: poi.proof_of_indexing.into(),
                entity_updates: serde_json::to_value(poi.debug_data.entity_updates).unwrap(),
                entity_deletions: serde_json::to_value(poi.debug_data.entity_deletions).unwrap(),
            })
            .collect::<Vec<_>>();

        let number_of_pois = pois.len();

        let connection = self.connection_pool.get()?;
        diesel::insert_into(schema::proofs_of_indexing::table)
            .values(pois)
            .on_conflict_do_nothing()
            .execute(&connection)?;

        info!(%number_of_pois, "Wrote POIs to database");

        Ok(())
    }

    fn insert_reports<I>(&self, reports: Vec<types::POICrossCheckReport<I>>) -> anyhow::Result<()>
    where
        I: Indexer,
    {
        let reports = reports
            .into_iter()
            .map(|report| models::POICrossCheckReport {
                timestamp: Utc::now().naive_utc(),
                indexer1: report.poi1.indexer.id().trim_start_matches("0x").into(),
                indexer2: report.poi2.indexer.id().trim_start_matches("0x").into(),
                deployment: report.poi1.deployment.deployment_id,
                block_hash: report.poi1.block.hash.map(|hash| hash.to_string()),
                block_number: report.poi1.block.number as i64,
                proof_of_indexing1: report.poi1.proof_of_indexing.to_string(),
                proof_of_indexing2: report.poi2.proof_of_indexing.to_string(),
                diverging_block: report.diverging_block.map(From::from),
            })
            .collect::<Vec<_>>();

        let number_of_reports = reports.len();

        let connection = self.connection_pool.get()?;
        diesel::insert_into(schema::poi_cross_check_reports::table)
            .values(reports)
            .on_conflict_do_nothing()
            .execute(&connection)?;

        info!(%number_of_reports, "Wrote POI cross-check reports to database");

        Ok(())
    }

    /// Write any POIs that we receive to the database.
    pub fn write_stream_of_pois<S, I>(&self, proofs_of_indexing: S)
    where
        S: Stream<Item = types::ProofOfIndexing<I>> + Send + 'static,
        I: Indexer + Send + Sync + 'static,
    {
        self.clone().write_items_to_db(
            proofs_of_indexing,
            |store, chunk| {
                let number_of_pois = chunk.len();
                store.insert_pois(chunk)?;
                info!(%number_of_pois, "Wrote POIs to database");
                Ok(())
            },
            |(error, attempts)| {
                warn!(%error, %attempts, "Failed to write POI cross-check reports to database");
            },
        );
    }

    /// Write any POI cross-check reports that we receive to the database.
    pub fn write_stream_of_reports<S, I>(&self, reports: S)
    where
        S: Stream<Item = types::POICrossCheckReport<I>> + Send + 'static,
        I: Indexer + Send + Sync + 'static,
    {
        self.clone().write_items_to_db(
            reports,
            |store, reports| {
                let number_of_reports = reports.len();
                store.insert_reports(reports)?;
                info!(%number_of_reports, "Wrote POI cross-check reports to database");
                Ok(())
            },
            |(error, attempts)| {
                warn!(%error, %attempts, "Failed to write POI cross-check reports to database");
            },
        )
    }

    pub fn deployments(&self) -> anyhow::Result<Vec<String>> {
        use diesel::prelude::*;
        use schema::proofs_of_indexing::dsl::*;

        let connection = self.connection_pool.get()?;
        let pois = proofs_of_indexing
            .select(deployment)
            .distinct_on(deployment)
            .order_by(deployment.asc())
            .load(&connection)?;

        Ok(pois)
    }

    pub fn pois(
        &self,
        deployments: &[String],
        block_range: RangeInclusive<u64>,
        limit: usize,
    ) -> anyhow::Result<Vec<models::ProofOfIndexing>> {
        use diesel::prelude::*;
        use schema::proofs_of_indexing::dsl::*;

        let connection = self.connection_pool.get()?;
        Ok(proofs_of_indexing
            .order_by(block_number.desc())
            .order_by(timestamp.desc())
            .filter(deployment.eq_any(deployments))
            .filter(block_number.between(*block_range.start() as i64, *block_range.end() as i64))
            .limit(limit as i64)
            .load::<models::ProofOfIndexing>(&connection)?)
    }

    pub fn poi_reports(
        &self,
        indexer1: Option<String>,
        indexer2: Option<String>,
        limit: usize,
    ) -> anyhow::Result<Vec<models::POICrossCheckReport>> {
        use diesel::prelude::*;
        use schema::poi_cross_check_reports as r;

        let connection = self.connection_pool.get()?;

        let mut query = r::table
            .distinct_on((r::block_number, r::indexer1, r::indexer2, r::deployment))
            .into_boxed();

        if let Some(i) = indexer1 {
            query = query.filter(r::indexer1.eq(i));
        }
        if let Some(i) = indexer2 {
            query = query.filter(r::indexer2.eq(i));
        }

        Ok(query
            .order_by((
                r::block_number.desc(),
                r::deployment.asc(),
                r::indexer1.asc(),
                r::indexer2.asc(),
            ))
            .limit(limit as _)
            .load(&connection)?)
    }

    pub fn insert_cached_ethereum_calls(
        &self,
        calls: Vec<models::CachedEthereumCall>,
    ) -> anyhow::Result<()> {
        let number_of_calls = calls.len();
        let connection = self.connection_pool.get()?;
        diesel::insert_into(schema::cached_ethereum_calls::table)
            .values(calls)
            .on_conflict_do_nothing()
            .execute(&connection)?;

        info!(%number_of_calls, "Wrote cached Ethereum calls to database");
        Ok(())
    }

    pub fn cached_ethereum_calls(
        &self,
        indexer: &str,
        deployment: &str,
        block_number: u64,
        limit: Option<u32>,
    ) -> anyhow::Result<Vec<models::CachedEthereumCall>> {
        use diesel::prelude::*;
        use schema::cached_ethereum_calls as c;

        let query = c::table
            .filter(
                (c::indexer.eq(indexer))
                    .and(c::deployment.eq(deployment))
                    .and(c::block_number.eq(block_number as i64)),
            )
            .limit(limit.unwrap_or(u32::MAX) as _)
            .order_by(c::id_hash.asc());

        let connection = self.connection_pool.get()?;
        Ok(query.load(&connection)?)
    }
}

fn retry_policy<E>(e: E, num_consecutive_errors: &mut u32) -> RetryPolicy<E> {
    if *num_consecutive_errors >= WRITE_TO_DB_RETRY_MAX_TIMES {
        *num_consecutive_errors = 0;
        RetryPolicy::ForwardError(e)
    } else {
        *num_consecutive_errors += 1;
        RetryPolicy::WaitRetry(WRITE_TO_DB_RETRY_BACKOFF)
    }
}
