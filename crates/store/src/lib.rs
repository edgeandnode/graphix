//! Database access (read and write) abstractions for the Graphix backend.

mod diesel_queries;
use diesel_async::pooled_connection::deadpool::{Object, Pool};
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::scoped_futures::ScopedFutureExt;
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
#[cfg(tests)]
pub use diesel_queries;
use graphix_common_types::{
    BlockRangeInput, IndexerVersion, IndexersQuery, Network, SgDeploymentsQuery,
};
pub mod models;
mod schema;

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Error;
use diesel::prelude::*;
use diesel_async_migrations::{embed_migrations, EmbeddedMigrations};
use graphix_indexer_client::{Indexer, WritablePoi};
use tracing::info;

use self::models::QueriedSgDeployment;
use crate::models::{Indexer as IndexerModel, Poi};

// TODO
//#[cfg(test)]
//mod tests;

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

/// An abstraction over all database operations. It uses [`Arc`] internally, so
/// it's cheaply cloneable.
#[derive(Clone)]
pub struct Store {
    pool: Pool<AsyncPgConnection>,
}

impl Store {
    /// Connects to the database and runs all pending migrations.
    pub async fn new(db_url: &str) -> anyhow::Result<Self> {
        info!("Initializing database connection pool");
        let manager = AsyncDieselConnectionManager::new(db_url);
        let pool = Pool::builder(manager).build()?;
        let store = Self { pool };
        store.run_migrations().await?;
        Ok(store)
    }

    async fn run_migrations(&self) -> anyhow::Result<()> {
        let mut conn = self.pool.get().await?;

        // Get a lock for running migrations. Blocks until we get the lock.
        // We need this because different Graphix instances may attempt
        // to run migrations concurrently (that's a big no-no).
        diesel::sql_query("select pg_advisory_lock(1)")
            .execute(&mut conn)
            .await?;
        info!("Run database migrations");

        MIGRATIONS
            .run_pending_migrations(&mut conn)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        // Release the migration lock.
        diesel::sql_query("select pg_advisory_unlock(1)")
            .execute(&mut conn)
            .await?;
        Ok(())
    }

    async fn conn(&self) -> anyhow::Result<Object<AsyncPgConnection>> {
        Ok(self.pool.get().await?)
    }

    /// Returns subgraph deployments stored in the database that match the
    /// filtering criteria.
    pub async fn sg_deployments(
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

        Ok(query
            .load::<QueriedSgDeployment>(&mut self.conn().await?)
            .await?)
    }

    pub async fn create_sg_deployment(
        &self,
        network_name: &str,
        ipfs_cid: &str,
    ) -> anyhow::Result<()> {
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
            .execute(&mut self.conn().await?)
            .await?;

        Ok(())
    }

    pub async fn set_deployment_name(
        &self,
        sg_deployment_id: &str,
        name: &str,
    ) -> anyhow::Result<()> {
        use schema::{sg_deployments as sgd, sg_names};

        diesel::insert_into(sg_names::table)
            .values((
                sg_names::sg_deployment_id.eq(sgd::table
                    .select(sgd::id)
                    .filter(sgd::ipfs_cid.eq(sg_deployment_id))
                    .single_value()
                    .assume_not_null()),
                sg_names::name.eq(name),
            ))
            .on_conflict(sg_names::sg_deployment_id)
            .do_update()
            .set(sg_names::name.eq(name))
            .execute(&mut self.conn().await?)
            .await?;

        Ok(())
    }

    /// Fetches a Poi from the database.
    pub async fn poi(&self, poi: &str) -> anyhow::Result<Option<Poi>> {
        use schema::{blocks, indexers, pois, sg_deployments};

        let poi = hex::decode(poi)?;

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
            .filter(pois::poi.eq(poi));

        Ok(query.get_result(&mut self.conn().await?).await.optional()?)
    }

    /// Deletes the network with the given name from the database, together with
    /// **all** of its related data (indexers, deployments, etc.).
    pub async fn delete_network(&self, network_name: &str) -> anyhow::Result<()> {
        use schema::networks;

        diesel::delete(networks::table.filter(networks::name.eq(network_name)))
            .execute(&mut self.conn().await?)
            .await?;
        // The `ON DELETE CASCADE`s should take care of the rest of the cleanup.

        Ok(())
    }

    pub async fn networks(&self) -> anyhow::Result<Vec<Network>> {
        use schema::networks;

        let mut conn = self.conn().await?;
        let rows = networks::table
            .select((networks::name, networks::caip2))
            .load::<(String, Option<String>)>(&mut conn)
            .await?;

        let networks = rows
            .into_iter()
            .map(|(name, caip2)| Network { name, caip2 })
            .collect();

        Ok(networks)
    }

    /// Returns all indexers stored in the database.
    pub async fn indexers(&self, filter: IndexersQuery) -> anyhow::Result<Vec<models::Indexer>> {
        use schema::indexers;

        let mut query = indexers::table.into_boxed();

        // FIXME
        if let Some(address) = filter.address {
            query = query.filter(indexers::name.eq(address));
        }
        if let Some(limit) = filter.limit {
            query = query.limit(limit.into());
        }

        let rows = query.load::<IndexerModel>(&mut self.conn().await?).await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Queries the database for proofs of indexing that refer to the specified
    /// subgraph deployments and in the given [`BlockRange`], if given.
    pub async fn pois(
        &self,
        sg_deployments: &[String],
        block_range: Option<BlockRangeInput>,
        limit: Option<u16>,
    ) -> anyhow::Result<Vec<Poi>> {
        let mut conn = self.conn().await?;
        diesel_queries::pois(
            &mut conn,
            None,
            Some(sg_deployments),
            block_range,
            limit,
            false,
        )
        .await
    }

    /// Like `pois`, but only returns live pois.
    pub async fn live_pois(
        &self,
        indexer_name: Option<&str>,
        sg_deployments_cids: Option<&[String]>,
        block_range: Option<BlockRangeInput>,
        limit: Option<u16>,
    ) -> anyhow::Result<Vec<Poi>> {
        let mut conn = self.conn().await?;
        diesel_queries::pois(
            &mut conn,
            indexer_name,
            sg_deployments_cids,
            block_range,
            limit,
            true,
        )
        .await
    }

    pub async fn write_pois<W>(&self, pois: Vec<W>, live: PoiLiveness) -> anyhow::Result<()>
    where
        W: WritablePoi + Send + Sync,
        W::IndexerId: Send + Sync,
    {
        self.conn()
            .await?
            .transaction::<_, Error, _>(|conn| {
                async move {
                    diesel_queries::write_pois(conn, pois, live).await?;
                    Ok(())
                }
                .scope_boxed()
            })
            .await
    }

    pub async fn write_indexers(&self, indexers: &[impl AsRef<dyn Indexer>]) -> anyhow::Result<()> {
        let mut conn = self.conn().await?;
        diesel_queries::write_indexers(&mut conn, indexers).await?;
        Ok(())
    }

    pub async fn write_graph_node_versions(
        &self,
        versions: HashMap<Arc<dyn Indexer>, anyhow::Result<IndexerVersion>>,
    ) -> anyhow::Result<()> {
        use schema::indexer_versions;
        for (indexer, version) in versions {
            let conn = &mut self.conn().await?;

            let indexer_id =
                diesel_queries::get_indexer_id(conn, indexer.name(), indexer.address()).await?;

            let new_version = match version {
                Ok(v) => models::NewIndexerVersion {
                    indexer_id,
                    error: None,
                    version_string: Some(v.version),
                    version_commit: Some(v.commit),
                },
                Err(err) => models::NewIndexerVersion {
                    indexer_id,
                    error: Some(err.to_string()),
                    version_string: None,
                    version_commit: None,
                },
            };

            diesel::insert_into(indexer_versions::table)
                .values(&new_version)
                .execute(conn)
                .await?;
        }

        Ok(())
    }

    pub async fn get_first_pending_divergence_investigation_request(
        &self,
    ) -> anyhow::Result<Option<(String, serde_json::Value)>> {
        use schema::pending_divergence_investigation_requests as requests;

        Ok(requests::table
            .select((requests::uuid, requests::request))
            .first::<(String, serde_json::Value)>(&mut self.conn().await?)
            .await
            .optional()?)
    }

    pub async fn create_divergence_investigation_request(
        &self,
        request: serde_json::Value,
    ) -> anyhow::Result<String> {
        use schema::pending_divergence_investigation_requests as requests;

        let uuid = uuid::Uuid::new_v4().to_string();
        diesel::insert_into(requests::table)
            .values((requests::uuid.eq(&uuid), requests::request.eq(&request)))
            .execute(&mut self.conn().await?)
            .await?;

        Ok(uuid)
    }

    /// Fetches the divergence investigation report with the given UUID, if it
    /// exists.
    pub async fn divergence_investigation_report(
        &self,
        uuid: &str,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        use schema::divergence_investigation_reports as reports;

        Ok(reports::table
            .select(reports::report)
            .filter(reports::uuid.eq(uuid))
            .first(&mut self.conn().await?)
            .await
            .optional()?)
    }

    pub async fn create_or_update_divergence_investigation_report(
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
            .execute(&mut self.conn().await?)
            .await?;

        Ok(())
    }

    pub async fn divergence_investigation_request_exists(
        &self,
        uuid: &str,
    ) -> anyhow::Result<bool> {
        use schema::pending_divergence_investigation_requests as requests;

        let exists = requests::table
            .filter(requests::uuid.eq(uuid))
            .count()
            .get_result::<i64>(&mut self.conn().await?)
            .await?
            > 0;
        Ok(exists)
    }

    pub async fn delete_divergence_investigation_request(&self, uuid: &str) -> anyhow::Result<()> {
        use schema::pending_divergence_investigation_requests as requests;

        diesel::delete(requests::table.filter(requests::uuid.eq(uuid)))
            .execute(&mut self.conn().await?)
            .await?;

        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PoiLiveness {
    Live,
    NotLive,
}
