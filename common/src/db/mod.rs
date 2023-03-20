use diesel::{
    r2d2::{self, ConnectionManager, Pool, PooledConnection},
    PgConnection,
};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use std::sync::Arc;
use tracing::info;

use crate::{api_schema::BlockRange, db::models::ProofOfIndexing};

pub mod models;
pub mod proofs_of_indexing;
pub mod schema;

embed_migrations!("../migrations");

/// An abstraction over all database operations. It uses [`Arc`] internally, so
/// it's cheaply cloneable.
#[derive(Clone)]
pub struct Store {
    pool: Arc<Pool<ConnectionManager<PgConnection>>>,
}

impl Store {
    pub fn new(db_url: &str) -> anyhow::Result<Self> {
        let manager = r2d2::ConnectionManager::<PgConnection>::new(db_url);
        let pool = Arc::new(r2d2::Builder::new().build(manager)?);
        let store = Self { pool };
        store.run_migrations()?;
        Ok(store)
    }

    fn run_migrations(&self) -> anyhow::Result<()> {
        info!("Run database migrations");
        let connection = self.pool.get()?;
        embedded_migrations::run(&connection)?;
        Ok(())
    }

    fn conn(&self) -> anyhow::Result<PooledConnection<ConnectionManager<PgConnection>>> {
        Ok(self.pool.get()?)
    }

    pub fn deployments(&self) -> anyhow::Result<Vec<String>> {
        use schema::proofs_of_indexing::dsl::*;

        let query = proofs_of_indexing.distinct_on(deployment);
        let pois = query
            .load::<models::ProofOfIndexing>(&self.conn()?)?
            .into_iter()
            .map(ProofOfIndexing::from);

        let mut deployments: Vec<String> = pois.map(|poi| poi.deployment).collect();
        deployments.sort();
        deployments.dedup();

        Ok(deployments)
    }

    pub fn pois(
        &self,
        deployments: &[String],
        block_range: Option<BlockRange>,
        limit: Option<u16>,
    ) -> anyhow::Result<Vec<ProofOfIndexing>> {
        use schema::proofs_of_indexing::dsl::*;

        let query = proofs_of_indexing
            .order_by(block_number.desc())
            .order_by(timestamp.desc())
            .filter(deployment.eq_any(deployments))
            .filter(block_number.between(
                block_range.as_ref().map_or(0, |range| range.start as i64),
                block_range.map_or(i64::max_value(), |range| range.end as i64),
            ))
            .limit(limit.unwrap_or(1000) as i64);

        Ok(query
            .load::<models::ProofOfIndexing>(&self.conn()?)?
            .into_iter()
            .map(ProofOfIndexing::from)
            .collect())
    }

    pub fn poi_cross_check_reports(
        &self,
        indexer1_s: Option<&str>,
        indexer2_s: Option<&str>,
    ) -> anyhow::Result<Vec<models::POICrossCheckReport>> {
        use schema::poi_cross_check_reports::dsl::*;

        let connection = self.conn()?;

        let mut query = poi_cross_check_reports
            .distinct_on((block_number, indexer1, indexer2, deployment))
            .into_boxed();

        if let Some(indexer) = indexer1_s {
            query = query.filter(indexer1.eq(indexer));
        }

        if let Some(indexer) = indexer2_s {
            query = query.filter(indexer2.eq(indexer));
        }

        query = query
            .order_by((
                block_number.desc(),
                deployment.asc(),
                indexer1.asc(),
                indexer2.asc(),
            ))
            .limit(5000);

        Ok(query.load::<models::POICrossCheckReport>(&connection)?)
    }

    pub fn write_pois(&self, pois: Vec<models::ProofOfIndexing>) -> anyhow::Result<()> {
        let len = pois.len();
        diesel::insert_into(schema::proofs_of_indexing::table)
            .values(pois)
            .on_conflict_do_nothing()
            .execute(&self.conn()?)?;

        info!(%len, "Wrote POIs to database");
        Ok(())
    }

    pub fn write_poi_cross_check_reports(
        &self,
        reports: Vec<models::POICrossCheckReport>,
    ) -> anyhow::Result<()> {
        let len = reports.len();
        diesel::insert_into(schema::poi_cross_check_reports::table)
            .values(reports)
            .on_conflict_do_nothing()
            .execute(&self.conn()?)?;

        info!(%len, "Wrote POI cross check reports to database");
        Ok(())
    }
}
