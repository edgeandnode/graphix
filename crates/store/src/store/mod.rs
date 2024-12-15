mod diesel_queries;

use std::collections::HashMap;
use std::fmt::Debug;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{anyhow, Error};
use diesel::prelude::*;
use diesel_async::pooled_connection::deadpool::{Object, Pool};
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::scoped_futures::ScopedFutureExt;
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use diesel_async_migrations::{embed_migrations, EmbeddedMigrations};
use graphix_common_types::{inputs, ApiKeyPermissionLevel, IndexerAddress, IpfsCid, PoiBytes};
use graphix_indexer_client::{IndexerClient, IndexerId, WritablePoi};
use tracing::info;
use uuid::Uuid;

use crate::models::{
    ApiKey, ApiKeyDbRow, ApiKeyPublicMetadata, FailedQueryRow, Indexer as IndexerModel, IntId,
    NewIndexerNetworkSubgraphMetadata, NewNetwork, NewlyCreatedApiKey, Poi, SgDeployment,
};
use crate::{models, schema};

/// An abstraction over all database operations. It uses [`Arc`] internally, so
/// it's cheaply cloneable.
#[derive(Clone)]
pub struct Store {
    pool: Pool<AsyncPgConnection>,
}

impl Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // It might contain sensitive data, so don't print it.
        f.debug_struct("Store").finish()
    }
}

impl Store {
    const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

    /// Connects to the database and runs all pending migrations.
    pub async fn new(db_url: &str) -> anyhow::Result<Self> {
        info!("Initializing database connection pool");

        let manager = AsyncDieselConnectionManager::new(db_url);
        let pool = Pool::builder(manager).build()?;
        let store = Self { pool };

        store.run_migrations().await?;

        if store.api_keys().await?.is_empty() {
            info!("No API keys found in database, creating master API key");
            store.create_master_api_key().await?;
        }

        Ok(store)
    }

    async fn run_migrations(&self) -> anyhow::Result<()> {
        let mut conn = self.pool.get().await?;

        info!("Run database migrations");

        Self::MIGRATIONS
            .run_pending_migrations(&mut conn)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        Ok(())
    }

    pub async fn conn(&self) -> anyhow::Result<Object<AsyncPgConnection>> {
        Ok(self.pool.get().await?)
    }

    pub async fn conn_err_string(&self) -> Result<Object<AsyncPgConnection>, String> {
        Ok(self.pool.get().await.map_err(|e| e.to_string())?)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PoiLiveness {
    Live,
    NotLive,
}

/// Getters.
impl Store {
    pub async fn current_config(&self) -> anyhow::Result<Option<serde_json::Value>> {
        use schema::configs;

        Ok(configs::table
            .order_by(configs::id.desc())
            .select(configs::config)
            .first::<serde_json::Value>(&mut self.conn().await?)
            .await
            .optional()?)
    }

    /// Returns subgraph deployments stored in the database that match the
    /// filtering criteria.
    pub async fn sg_deployments(
        &self,
        filter: inputs::SgDeploymentsQuery,
    ) -> anyhow::Result<Vec<SgDeployment>> {
        use schema::sg_deployments as sgd;

        let mut query = sgd::table
            .inner_join(schema::networks::table)
            .left_join(schema::sg_names::table)
            .select((
                sgd::id,
                sgd::ipfs_cid,
                schema::sg_names::name.nullable(),
                sgd::network,
                sgd::created_at,
            ))
            .order_by(sgd::ipfs_cid.asc())
            .into_boxed();

        if let Some(network_name) = filter.network_name {
            query = query.filter(schema::networks::name.eq(network_name));
        }
        if let Some(name) = filter.name {
            query = query.filter(schema::sg_names::name.eq(name));
        }
        if let Some(ipfs_cid) = filter.ipfs_cid {
            query = query.filter(sgd::ipfs_cid.eq(ipfs_cid.to_string()));
        }
        if let Some(limit) = filter.limit {
            query = query.limit(limit.into());
        }

        Ok(query.load::<SgDeployment>(&mut self.conn().await?).await?)
    }

    /// Fetches a Poi from the database.
    pub async fn poi(&self, poi: &PoiBytes) -> anyhow::Result<Option<Poi>> {
        use schema::pois;

        let query = pois::table
            .select(pois::all_columns)
            .filter(pois::poi.eq(poi));

        Ok(query.get_result(&mut self.conn().await?).await.optional()?)
    }

    pub async fn failed_query(
        &self,
        indexer: &impl IndexerId,
        query_name: &str,
    ) -> anyhow::Result<Option<FailedQueryRow>> {
        use schema::failed_queries;

        let conn = &mut self.conn().await?;
        let indexer_id =
            diesel_queries::get_indexer_id(conn, indexer.name(), &indexer.address()).await?;

        let failed_query = failed_queries::table
            .filter(failed_queries::indexer_id.eq(indexer_id))
            .filter(failed_queries::query_name.eq(query_name))
            .select((
                failed_queries::indexer_id,
                failed_queries::query_name,
                failed_queries::raw_query,
                failed_queries::response,
                failed_queries::request_timestamp,
            ))
            .get_result::<FailedQueryRow>(conn)
            .await
            .optional()?;

        Ok(failed_query)
    }

    /// Returns all networks stored in the database. Filtering is not really
    /// necessary here because the number of networks is expected to be small,
    /// so filtering can be done client-side.
    pub async fn networks(&self) -> anyhow::Result<Vec<models::Network>> {
        use schema::networks;

        let mut conn = self.conn().await?;
        Ok(networks::table
            .select((networks::id, networks::name, networks::caip2))
            .load(&mut conn)
            .await?)
    }

    /// Returns all indexers stored in the database.
    pub async fn indexers(
        &self,
        filter: inputs::IndexersQuery,
    ) -> anyhow::Result<Vec<models::Indexer>> {
        use schema::indexers;

        let mut query = indexers::table.select(indexers::all_columns).into_boxed();

        if let Some(address) = filter.address {
            query = query.filter(indexers::address.eq(address));
        }
        if let Some(limit) = filter.limit {
            query = query.limit(limit.into());
        }

        Ok(query.load::<IndexerModel>(&mut self.conn().await?).await?)
    }

    /// Queries the database for proofs of indexing that refer to the specified
    /// subgraph deployments and in the given [`inputs::BlockRange`], if given.
    pub async fn pois(
        &self,
        sg_deployments: &[IpfsCid],
        block_range: Option<inputs::BlockRange>,
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
        indexer_address: Option<&IndexerAddress>,
        sg_deployments_cids: Option<&[IpfsCid]>,
        block_range: Option<inputs::BlockRange>,
        limit: Option<u16>,
    ) -> anyhow::Result<Vec<Poi>> {
        let mut conn = self.conn().await?;
        diesel_queries::pois(
            &mut conn,
            indexer_address,
            sg_deployments_cids,
            block_range,
            limit,
            true,
        )
        .await
    }

    pub async fn api_keys(&self) -> anyhow::Result<Vec<ApiKeyPublicMetadata>> {
        use schema::graphix_api_tokens;

        Ok(graphix_api_tokens::table
            .load::<ApiKeyDbRow>(&mut self.conn().await?)
            .await?
            .into_iter()
            .map(ApiKeyPublicMetadata::from)
            .collect())
    }

    pub async fn permission_level(
        &self,
        api_key: &ApiKey,
    ) -> anyhow::Result<Option<ApiKeyPermissionLevel>> {
        use schema::graphix_api_tokens;

        Ok(graphix_api_tokens::table
            .select(graphix_api_tokens::permission_level)
            .filter(graphix_api_tokens::sha256_api_key_hash.eq(api_key.hash()))
            .get_result(&mut self.conn().await?)
            .await
            .optional()?)
    }

    pub async fn get_first_pending_divergence_investigation_request(
        &self,
    ) -> anyhow::Result<Option<(Uuid, serde_json::Value)>> {
        use schema::pending_divergence_investigation_requests as requests;

        Ok(requests::table
            .select((requests::uuid, requests::request))
            .first::<(Uuid, serde_json::Value)>(&mut self.conn().await?)
            .await
            .optional()?)
    }

    /// Fetches the divergence investigation report with the given UUID, if it
    /// exists.
    pub async fn divergence_investigation_report(
        &self,
        uuid: &Uuid,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        use schema::divergence_investigation_reports as reports;

        Ok(reports::table
            .select(reports::report)
            .filter(reports::uuid.eq(uuid))
            .first(&mut self.conn().await?)
            .await
            .optional()?)
    }

    pub async fn divergence_investigation_request_exists(
        &self,
        uuid: &Uuid,
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
}

/// Setters and write operations.
impl Store {
    pub async fn overwrite_config(&self, config: serde_json::Value) -> anyhow::Result<()> {
        use schema::configs;

        diesel::update(configs::table)
            .set(configs::config.eq(config))
            .execute(&mut self.conn().await?)
            .await?;

        Ok(())
    }

    async fn create_master_api_key(&self) -> anyhow::Result<()> {
        let api_key = self
            .create_api_key(None, ApiKeyPermissionLevel::Admin)
            .await?;

        let description = format!("Master API key created during database initialization. Use it to create a new private API key and then delete it for security reasons. `{}`", api_key.api_key.to_string());
        self.modify_api_key(
            &api_key.api_key,
            Some(&description),
            ApiKeyPermissionLevel::Admin,
        )
        .await?;

        info!(api_key = ?api_key.api_key, "Created master API key");

        Ok(())
    }

    pub async fn create_networks_if_missing(&self, networks: &[NewNetwork]) -> anyhow::Result<()> {
        use schema::networks;

        let mut conn = self.conn().await?;

        // batch insert
        diesel::insert_into(networks::table)
            .values(networks)
            .on_conflict_do_nothing()
            .execute(&mut conn)
            .await?;

        Ok(())
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

    pub async fn create_network(&self, network: &NewNetwork) -> anyhow::Result<IntId> {
        use schema::networks;

        let id = diesel::insert_into(networks::table)
            .values(network)
            .returning(networks::id)
            .get_result(&mut self.conn().await?)
            .await?;

        Ok(id)
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

    pub async fn write_indexers(
        &self,
        indexers: &[impl AsRef<dyn IndexerClient>],
    ) -> anyhow::Result<()> {
        let mut conn = self.conn().await?;
        diesel_queries::write_indexers(&mut conn, indexers).await?;
        Ok(())
    }

    pub async fn delete_indexer_network_subgraph_metadata(
        &self,
        indexer_id: IntId,
    ) -> anyhow::Result<()> {
        use schema::indexers;

        diesel::update(indexers::table.filter(indexers::id.eq(indexer_id)))
            .set(indexers::network_subgraph_metadata.eq::<Option<IntId>>(None))
            .execute(&mut self.conn().await?)
            .await?;

        Ok(())
    }

    pub async fn create_or_update_indexer_network_subgraph_metadata(
        &self,
        indexer_id: IntId,
        metadata: NewIndexerNetworkSubgraphMetadata,
    ) -> anyhow::Result<IntId> {
        use schema::{indexer_network_subgraph_metadata, indexers};

        self.conn()
            .await?
            .transaction::<_, Error, _>(|conn| {
                // Fetch the metadata id from indexer_id, and update it if it exists
                // create a new one and set the foreign key to the indexer_id if it doesn't exist
                async move {
                    let metadata_id = indexers::table
                        .select(indexers::network_subgraph_metadata)
                        .filter(indexers::id.eq(indexer_id))
                        .get_result::<Option<IntId>>(conn)
                        .await?;

                    let metadata_id = match metadata_id {
                        Some(id) => {
                            diesel::update(
                                indexer_network_subgraph_metadata::table
                                    .filter(indexer_network_subgraph_metadata::id.eq(id)),
                            )
                            .set(metadata)
                            .execute(conn)
                            .await?;
                            id
                        }
                        None => {
                            let metadata_id =
                                diesel::insert_into(indexer_network_subgraph_metadata::table)
                                    .values(&metadata)
                                    .returning(indexer_network_subgraph_metadata::id)
                                    .get_result(conn)
                                    .await?;

                            diesel::update(indexers::table)
                                .filter(indexers::id.eq(indexer_id))
                                .set(indexers::network_subgraph_metadata.eq(metadata_id))
                                .execute(conn)
                                .await?;

                            metadata_id
                        }
                    };

                    Ok(metadata_id)
                }
                .scope_boxed()
            })
            .await?;

        Ok(indexer_id)
    }

    pub async fn create_api_key(
        &self,
        notes: Option<&str>,
        permission_level: ApiKeyPermissionLevel,
    ) -> anyhow::Result<NewlyCreatedApiKey> {
        use schema::graphix_api_tokens;

        let api_key = ApiKey::generate();
        let stored_api_key = ApiKeyDbRow {
            public_prefix: api_key.public_part_as_string(),
            sha256_api_key_hash: api_key.hash(),
            notes: notes.map(|s| s.to_string()),
            permission_level,
        };

        diesel::insert_into(graphix_api_tokens::table)
            .values(&[stored_api_key])
            .execute(&mut self.conn().await?)
            .await?;

        Ok(NewlyCreatedApiKey {
            api_key: api_key.to_string(),
            notes: notes.map(|s| s.to_string()),
            permission_level,
        })
    }

    pub async fn modify_api_key(
        &self,
        api_key_s: &str,
        notes: Option<&str>,
        permission_level: ApiKeyPermissionLevel,
    ) -> anyhow::Result<()> {
        use schema::graphix_api_tokens;

        let api_key = ApiKey::from_str(api_key_s).map_err(|e| anyhow!("invalid api key: {}", e))?;

        diesel::update(graphix_api_tokens::table)
            .filter(graphix_api_tokens::sha256_api_key_hash.eq(api_key.hash()))
            .set((
                graphix_api_tokens::notes.eq(notes),
                graphix_api_tokens::permission_level.eq(permission_level),
            ))
            .execute(&mut self.conn().await?)
            .await?;

        Ok(())
    }

    pub async fn delete_api_key(&self, api_key_s: &str) -> anyhow::Result<()> {
        use schema::graphix_api_tokens;

        let api_key = ApiKey::from_str(api_key_s).map_err(|e| anyhow!("invalid api key: {}", e))?;

        diesel::delete(graphix_api_tokens::table)
            .filter(graphix_api_tokens::sha256_api_key_hash.eq(api_key.hash()))
            .execute(&mut self.conn().await?)
            .await?;

        Ok(())
    }

    pub async fn write_graph_node_versions(
        &self,
        versions: HashMap<
            Arc<dyn IndexerClient>,
            anyhow::Result<graphix_common_types::GraphNodeCollectedVersion>,
        >,
    ) -> anyhow::Result<()> {
        use schema::graph_node_collected_versions;
        for version in versions.values() {
            let conn = &mut self.conn().await?;

            let new_version = match version {
                Ok(v) => models::NewGraphNodeCollectedVersion {
                    version_string: v.version.clone(),
                    version_commit: v.commit.clone(),
                    error_response: None,
                },
                Err(err) => models::NewGraphNodeCollectedVersion {
                    version_string: None,
                    version_commit: None,
                    error_response: Some(err.to_string()),
                },
            };

            diesel::insert_into(graph_node_collected_versions::table)
                .values(&new_version)
                .execute(conn)
                .await?;
        }

        Ok(())
    }

    pub async fn create_divergence_investigation_request(
        &self,
        request: serde_json::Value,
    ) -> anyhow::Result<Uuid> {
        use schema::pending_divergence_investigation_requests as requests;

        let uuid = uuid::Uuid::new_v4();
        diesel::insert_into(requests::table)
            .values((requests::uuid.eq(&uuid), requests::request.eq(&request)))
            .execute(&mut self.conn().await?)
            .await?;

        Ok(uuid)
    }

    pub async fn create_or_update_divergence_investigation_report(
        &self,
        uuid: &Uuid,
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

    pub async fn delete_divergence_investigation_request(&self, uuid: &Uuid) -> anyhow::Result<()> {
        use schema::pending_divergence_investigation_requests as requests;

        diesel::delete(requests::table.filter(requests::uuid.eq(uuid)))
            .execute(&mut self.conn().await?)
            .await?;

        Ok(())
    }
}
