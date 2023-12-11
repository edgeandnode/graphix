//! Database access (read and write) abstractions for all Graphix backend
//! services.

// Provides the diesel queries, callers should handle connection pooling and transactions.
mod diesel_queries;
#[cfg(tests)]
pub use diesel_queries;
pub mod models;
mod schema;

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Error;
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager, Pool, PooledConnection};
use diesel::{Connection, PgConnection};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use tracing::info;

use self::models::{QueriedSgDeployment, WritablePoi};
use crate::graphql_api::types::{BlockRangeInput, IndexersQuery, SgDeploymentsQuery};
use crate::indexer::Indexer;
use crate::prelude::IndexerVersion;
use crate::store::models::{IndexerRow, Poi};

#[cfg(test)]
mod tests;

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

/// An abstraction over all database operations. It uses [`Arc`] internally, so
/// it's cheaply cloneable.
#[derive(Clone)]
pub struct Store {
    pool: Pool<ConnectionManager<PgConnection>>,
}

impl Store {
    /// Connects to the database and runs migrations.
    pub async fn new(db_url: &str) -> anyhow::Result<Self> {
        info!("Initializing database connection pool");
        let manager = r2d2::ConnectionManager::<PgConnection>::new(db_url);
        let pool = r2d2::Builder::new().build(manager)?;
        let store = Self { pool };
        store.run_migrations()?;
        Ok(store)
    }

    fn run_migrations(&self) -> anyhow::Result<()> {
        let mut conn = self.pool.get()?;

        // Get a lock for running migrations. Blocks until we get the lock.
        diesel::sql_query("select pg_advisory_lock(1)").execute(&mut conn)?;
        info!("Run database migrations");
        conn.run_pending_migrations(MIGRATIONS)
            .map_err(|e| anyhow::anyhow!(e))?;

        // Release the migration lock.
        diesel::sql_query("select pg_advisory_unlock(1)").execute(&mut conn)?;
        Ok(())
    }

    fn conn(&self) -> anyhow::Result<PooledConnection<ConnectionManager<PgConnection>>> {
        Ok(self.pool.get()?)
    }

    /// Returns subgraph deployments stored in the database that match the
    /// filtering criteria.
    pub fn sg_deployments(
        &self,
        filter: SgDeploymentsQuery,
    ) -> anyhow::Result<Vec<QueriedSgDeployment>> {
        use schema::sg_deployments as sgd;

        let mut query = sgd::table
            .inner_join(schema::networks::table)
            .left_join(schema::sg_names::table)
            .select((
                sgd::ipfs_cid,
                schema::sg_names::name.nullable(),
                schema::networks::name,
            ))
            .order_by(sgd::ipfs_cid.asc())
            .into_boxed();

        if let Some(network) = filter.network {
            query = query.filter(schema::networks::name.eq(network));
        }
        if let Some(name) = filter.name {
            query = query.filter(schema::sg_names::name.eq(name));
        }
        if let Some(ipfs_cid) = filter.ipfs_cid {
            query = query.filter(sgd::ipfs_cid.eq(ipfs_cid));
        }
        if let Some(limit) = filter.limit {
            query = query.limit(limit.into());
        }

        Ok(query.load::<QueriedSgDeployment>(&mut self.conn()?)?)
    }

    pub fn create_sg_deployment(&self, network_name: &str, ipfs_cid: &str) -> anyhow::Result<()> {
        use schema::sg_deployments as sgd;

        diesel::insert_into(sgd::table)
            .values((
                sgd::ipfs_cid.eq(ipfs_cid),
                sgd::network.eq(schema::networks::table
                    .select(schema::networks::id)
                    .filter(schema::networks::name.eq(network_name))
                    .single_value()
                    .assume_not_null()),
            ))
            .execute(&mut self.conn()?)?;

        Ok(())
    }

    pub fn set_deployment_name(&self, sg_deployment_id: &str, name: &str) -> anyhow::Result<()> {
        let mut conn = self.conn()?;
        diesel_queries::set_deployment_name(&mut conn, sg_deployment_id, name)
    }

    /// Fetches a Poi from the database.
    pub fn poi(&self, poi: &str) -> anyhow::Result<Option<Poi>> {
        let mut conn = self.conn()?;
        diesel_queries::poi(&mut conn, poi)
    }

    /// Deletes the network with the given name from the database, together with
    /// **all** of its related data (indexers, deployments, etc.).
    pub fn delete_network(&self, network_name: &str) -> anyhow::Result<()> {
        let mut conn = self.conn()?;
        diesel_queries::delete_network(&mut conn, network_name)
    }

    /// Returns all indexers stored in the database.
    pub fn indexers(&self, filter: IndexersQuery) -> anyhow::Result<Vec<models::Indexer>> {
        use schema::indexers;

        let mut query = indexers::table.into_boxed();

        if let Some(address) = filter.address {
            query = query.filter(indexers::name.eq(address));
        }
        if let Some(limit) = filter.limit {
            query = query.limit(limit.into());
        }

        let rows = query.load::<IndexerRow>(&mut self.conn()?)?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Queries the database for proofs of indexing that refer to the specified
    /// subgraph deployments and in the given [`BlockRange`], if given.
    pub fn pois(
        &self,
        sg_deployments: &[String],
        block_range: Option<BlockRangeInput>,
        limit: Option<u16>,
    ) -> anyhow::Result<Vec<Poi>> {
        let mut conn = self.conn()?;
        diesel_queries::pois(
            &mut conn,
            None,
            Some(sg_deployments),
            block_range,
            limit,
            false,
        )
    }

    /// Like `pois`, but only returns live pois.
    pub fn live_pois(
        &self,
        indexer_name: Option<&str>,
        sg_deployments_cids: Option<&[String]>,
        block_range: Option<BlockRangeInput>,
        limit: Option<u16>,
    ) -> anyhow::Result<Vec<Poi>> {
        let mut conn = self.conn()?;
        diesel_queries::pois(
            &mut conn,
            indexer_name,
            sg_deployments_cids,
            block_range,
            limit,
            true,
        )
    }

    pub fn write_pois(&self, pois: &[impl WritablePoi], live: PoiLiveness) -> anyhow::Result<()> {
        self.conn()?
            .transaction::<_, Error, _>(|conn| diesel_queries::write_pois(conn, pois, live))
    }

    pub fn write_graph_node_versions(
        &self,
        versions: HashMap<Arc<dyn Indexer>, anyhow::Result<IndexerVersion>>,
    ) -> anyhow::Result<()> {
        for (indexer, version) in versions {
            let mut conn = self.conn()?;
            diesel_queries::write_graph_node_version(&mut conn, &*indexer, version)?;
        }

        Ok(())
    }

    pub fn get_first_pending_divergence_investigation_request(
        &self,
    ) -> anyhow::Result<Option<(String, serde_json::Value)>> {
        use schema::pending_divergence_investigation_requests as requests;

        Ok(requests::table
            .select((requests::uuid, requests::request))
            .first::<(String, serde_json::Value)>(&mut self.conn()?)
            .optional()?)
    }

    pub fn create_divergence_investigation_request(
        &self,
        request: serde_json::Value,
    ) -> anyhow::Result<String> {
        use schema::pending_divergence_investigation_requests as requests;

        let uuid = uuid::Uuid::new_v4().to_string();
        diesel::insert_into(requests::table)
            .values((requests::uuid.eq(&uuid), requests::request.eq(&request)))
            .execute(&mut self.conn()?)?;

        Ok(uuid)
    }

    /// Fetches the divergence investigation report with the given UUID, if it
    /// exists.
    pub fn divergence_investigation_report(
        &self,
        uuid: &str,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        use schema::divergence_investigation_reports as reports;

        Ok(reports::table
            .select(reports::report)
            .filter(reports::uuid.eq(uuid))
            .first(&mut self.conn()?)
            .optional()?)
    }

    pub fn create_or_update_divergence_investigation_report(
        &self,
        uuid: &str,
        report: serde_json::Value,
    ) -> anyhow::Result<()> {
        use schema::divergence_investigation_reports as reports;

        diesel::insert_into(reports::table)
            .values((reports::uuid.eq(&uuid), reports::report.eq(&report)))
            .on_conflict(reports::uuid)
            .do_update()
            .set(reports::report.eq(&report))
            .execute(&mut self.conn()?)?;

        Ok(())
    }

    pub fn divergence_investigation_request_exists(&self, uuid: &str) -> anyhow::Result<bool> {
        use schema::pending_divergence_investigation_requests as requests;

        let exists = requests::table
            .filter(requests::uuid.eq(uuid))
            .count()
            .get_result::<i64>(&mut self.conn()?)?
            > 0;
        Ok(exists)
    }

    pub fn delete_divergence_investigation_request(&self, uuid: &str) -> anyhow::Result<()> {
        use schema::pending_divergence_investigation_requests as requests;

        diesel::delete(requests::table.filter(requests::uuid.eq(uuid)))
            .execute(&mut self.conn()?)?;

        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PoiLiveness {
    Live,
    NotLive,
}
