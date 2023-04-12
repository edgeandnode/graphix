use crate::{
    api_types::BlockRange,
    db::models::{IndexerRow, NewIndexer, NewPoI, NewSgDeployment, PoI, SgDeployment},
    indexer::Indexer,
    types,
};
use anyhow::Error;
use chrono::Utc;
use diesel::{
    r2d2::{self, ConnectionManager, Pool, PooledConnection},
    Connection, OptionalExtension, PgConnection,
};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use tracing::info;

pub mod models;
pub mod proofs_of_indexing;
mod schema;

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
                sg_deployments::all_columns,
                indexers::all_columns,
                blocks::all_columns,
            ))
            .order_by(blocks::number.desc())
            .order_by(schema::pois::created_at.desc())
            .filter(sg_deployments::cid.eq_any(sg_deployments))
            .filter(blocks::number.between(
                block_range.as_ref().map_or(0, |range| range.start as i64),
                block_range.map_or(i64::max_value(), |range| range.end as i64),
            ))
            .limit(limit.unwrap_or(1000) as i64);

        Ok(query
            .load::<models::PoI>(&mut self.conn()?)?
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

    pub fn write_pois(&self, pois: &[types::ProofOfIndexing<impl Indexer>]) -> anyhow::Result<()> {
        use schema::blocks;
        use schema::indexers;
        use schema::pois;
        use schema::sg_deployments;

        let len = pois.len();

        self.conn()?.transaction::<_, Error, _>(|conn| {
            for poi in pois {
                let sg_deployment_id = {
                    // First, attempt to find the existing sg_deployment by the deployment field
                    let existing_sg_deployment: Option<SgDeployment> = sg_deployments::table
                        .filter(sg_deployments::cid.eq(poi.deployment.as_str()))
                        .get_result(conn)
                        .optional()?;

                    if let Some(existing_sg_deployment) = existing_sg_deployment {
                        // If the sg_deployment exists, use its id
                        existing_sg_deployment.id
                    } else {
                        // If the sg_deployment doesn't exist, insert a new one and return its id
                        let new_sg_deployment = NewSgDeployment {
                            cid: poi.deployment.0.clone(),
                            created_at: Utc::now().naive_utc(),
                        };
                        diesel::insert_into(sg_deployments::table)
                            .values(&new_sg_deployment)
                            .returning(sg_deployments::id)
                            .get_result(conn)?
                    }
                };

                let indexer_id = {
                    // First, attempt to find the existing indexer by the address field
                    let existing_indexer: Option<IndexerRow> = indexers::table
                        .filter(indexers::name.eq(poi.indexer.id()))
                        .filter(indexers::address.eq(poi.indexer.address()))
                        .get_result(conn)
                        .optional()?;

                    if let Some(existing_indexer) = existing_indexer {
                        // If the indexer exists, use its id
                        existing_indexer.id
                    } else {
                        // If the indexer doesn't exist, insert a new one and return its id
                        let new_indexer = NewIndexer {
                            address: poi.indexer.address().map(ToOwned::to_owned),
                            created_at: Utc::now().naive_utc(),
                        };
                        diesel::insert_into(indexers::table)
                            .values(&new_indexer)
                            .returning(indexers::id)
                            .get_result(conn)?
                    }
                };

                let block_id = {
                    // First, attempt to find the existing block by hash
                    // TODO: also filter by network to be extra safe
                    let existing_block: Option<models::Block> = blocks::table
                        .filter(blocks::hash.eq(&poi.block.hash.unwrap().0.as_slice()))
                        .get_result(conn)
                        .optional()?;

                    if let Some(existing_block) = existing_block {
                        // If the block exists, use its id
                        existing_block.id
                    } else {
                        // If the block doesn't exist, insert a new one and return its id
                        let new_block = models::NewBlock {
                            number: poi.block.number as i64,
                            hash: poi.block.hash.unwrap().0.to_vec(),

                            // TODO: handle networks properly
                            network_id: 0,
                        };
                        diesel::insert_into(blocks::table)
                            .values(&new_block)
                            .returning(blocks::id)
                            .get_result(conn)?
                    }
                };

                let new_poi = NewPoI {
                    sg_deployment_id,
                    indexer_id,
                    block_id,
                    poi: poi.proof_of_indexing.0.to_vec(),
                    created_at: Utc::now().naive_utc(),
                };

                diesel::insert_into(pois::table)
                    .values(new_poi)
                    .on_conflict_do_nothing()
                    .execute(&mut self.conn()?)?;
            }
            Ok(())
        })?;
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
