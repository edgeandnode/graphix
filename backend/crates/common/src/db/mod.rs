use crate::{api_types::BlockRange, db::models::PoI};
use anyhow::Error;
use diesel::prelude::*;
use diesel::{
    r2d2::{self, ConnectionManager, Pool, PooledConnection},
    Connection, PgConnection,
};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use tracing::info;

// Provides the diesel queries, callers should handle connection pooling and transactions.
mod diesel_queries;
#[cfg(tests)]
pub use diesel_queries;

use self::models::WritablePoI;

pub mod models;
pub mod proofs_of_indexing;
mod schema;

#[cfg(test)]
mod tests;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

/// An abstraction over all database operations. It uses [`Arc`](std::sync::Arc) internally, so
/// it's cheaply cloneable.
#[derive(Clone)]
pub struct Store {
    pool: Pool<ConnectionManager<PgConnection>>,
}

impl Store {
    pub fn new(db_url: &str) -> anyhow::Result<Self> {
        let manager = r2d2::ConnectionManager::<PgConnection>::new(db_url);
        let pool = r2d2::Builder::new().build(manager)?;
        let store = Self { pool };
        store.run_migrations()?;
        Ok(store)
    }

    fn run_migrations(&self) -> anyhow::Result<()> {
        info!("Run database migrations");
        let mut connection = self.pool.get()?;
        connection
            .run_pending_migrations(MIGRATIONS)
            .map_err(|e| anyhow::anyhow!(e))?;
        Ok(())
    }

    fn conn(&self) -> anyhow::Result<PooledConnection<ConnectionManager<PgConnection>>> {
        Ok(self.pool.get()?)
    }

    #[cfg(test)]
    pub fn test_conn(&self) -> PooledConnection<ConnectionManager<PgConnection>> {
        self.pool.get().unwrap()
    }

    /// Returns all subgraph deployments that have ever analyzed.
    pub fn sg_deployments(&self) -> anyhow::Result<Vec<String>> {
        use schema::sg_deployments as sgd;

        let mut deployments: Vec<String> = sgd::table
            .select(sgd::cid)
            .load::<String>(&mut self.conn()?)?
            .into_iter()
            .map(|x| hex::ToHex::encode_hex(&x))
            .collect();

        deployments.sort();
        Ok(deployments)
    }

    /// Queries the database for proofs of indexing that refer to the specified
    /// subgraph deployments and in the given [`BlockRange`], if given.
    pub fn pois(
        &self,
        sg_deployments: &[String],
        block_range: Option<BlockRange>,
        limit: Option<u16>,
    ) -> anyhow::Result<Vec<PoI>> {
        let mut conn = self.conn()?;
        diesel_queries::pois(&mut conn, sg_deployments, block_range, limit)
    }

    pub fn write_pois(&self, pois: &[impl WritablePoI], live: PoiLiveness) -> anyhow::Result<()> {
        self.conn()?
            .transaction::<_, Error, _>(|conn| diesel_queries::write_pois(conn, pois, live))
    }

    // pub fn poi_divergence_bisect_reports(
    //     &self,
    //     indexer1: Filter,
    //     indexer2: Filter,
    // ) -> anyhow::Result<Vec<models::PoiDivergenceBisectReport>> {
    //     use schema::poi_divergence_bisect_reports::dsl::*;

    //     let mut query = poi_divergence_bisect_reports
    //         .filter(sql)
    //         .filter(poi1_id.eq(foo).and(poi2_id.eq(bar)))
    //         .distinct_on((block_number, indexer1, indexer2, deployment))
    //         .into_boxed();

    //     if let Some(indexer) = indexer1_s {
    //         query = query.filter(indexer1.eq(indexer));
    //     }

    //     if let Some(indexer) = indexer2_s {
    //         query = query.filter(indexer2.eq(indexer));
    //     }

    //     query = query
    //         .order_by((
    //             block_number.desc(),
    //             deployment.asc(),
    //             indexer1.asc(),
    //             indexer2.asc(),
    //         ))
    //         .limit(5000);

    //     Ok(query.load::<models::PoiCrossCheckReport>(&self.conn()?)?)
    // }

    // pub fn write_poi_cross_check_reports(
    //     &self,
    //     reports: Vec<models::PoiCrossCheckReport>,
    // ) -> anyhow::Result<()> {
    //     let len = reports.len();
    //     diesel::insert_into(schema::poi_cross_check_reports::table)
    //         .values(reports)
    //         .on_conflict_do_nothing()
    //         .execute(&self.conn()?)?;

    //     info!(%len, "Wrote POI cross check reports to database");
    //     Ok(())
    // }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PoiLiveness {
    Live,
    NotLive,
}
