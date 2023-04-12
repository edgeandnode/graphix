use crate::{api_types::BlockRange, db::models::PoI};
use diesel::{
    r2d2::{self, ConnectionManager, Pool, PooledConnection},
    PgConnection,
};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use tracing::info;

pub mod models;
pub mod proofs_of_indexing;
mod schema;

embed_migrations!("migrations");

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
        let connection = self.pool.get()?;
        embedded_migrations::run(&connection)?;
        Ok(())
    }

    fn conn(&self) -> anyhow::Result<PooledConnection<ConnectionManager<PgConnection>>> {
        Ok(self.pool.get()?)
    }

    /// Returns all subgraph deployments that have ever analyzed.
    pub fn sg_deployments(&self) -> anyhow::Result<Vec<String>> {
        use schema::sg_deployments as sgd;

        let mut deployments: Vec<String> = sgd::table
            .select(sgd::deployment)
            .load::<Vec<u8>>(&self.conn()?)?
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
        use schema::blocks;
        use schema::indexers;
        use schema::pois;
        use schema::sg_deployments;

        let query = pois::table
            .inner_join(sg_deployments::table)
            .inner_join(indexers::table)
            .inner_join(blocks::table)
            .select((
                pois::id,
                pois::poi,
                pois::created_at,
                (
                    sg_deployments::id,
                    sg_deployments::deployment,
                    sg_deployments::created_at,
                ),
                (indexers::id, indexers::address, indexers::created_at),
                blocks::all_columns,
            ))
            .order_by(blocks::number.desc())
            .order_by(schema::pois::created_at.desc())
            .filter(pois::sg_deployment_id.eq(sg_deployments::id))
            .filter(blocks::number.between(
                block_range.as_ref().map_or(0, |range| range.start as i64),
                block_range.map_or(i64::max_value(), |range| range.end as i64),
            ))
            .limit(limit.unwrap_or(1000) as i64);

        Ok(query
            .load::<models::PoI>(&self.conn()?)?
            .into_iter()
            .map(PoI::from)
            .collect())
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

    pub fn write_pois(&self, pois: Vec<models::PoI>) -> anyhow::Result<()> {
        let len = pois.len();

        // TODO: Rewrite this
        // diesel::insert_into(schema::pois::table)
        //     .values(pois)
        //     .on_conflict_do_nothing()
        //     .execute(&self.conn()?)?;

        info!(%len, "Wrote POIs to database");
        Ok(())
    }

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
