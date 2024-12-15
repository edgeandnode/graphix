use async_graphql::{Context, Object, Result};
use graphix_common_types::*;
use graphix_store::models::{DivergenceInvestigationRequest, NewlyCreatedApiKey};

use super::{ctx_data, require_permission_level};

pub struct MutationRoot;

#[Object]
impl MutationRoot {
    /// Launches a divergence investigation, which is a process of comparing
    /// two or more PoIs (up to four) and running a binary search to find the first
    /// diverging block.
    async fn launch_divergence_investigation(
        &self,
        ctx: &Context<'_>,
        #[graphql(
            validator(min_items = 2, max_items = 4),
            desc = "A list of PoI hashes that should be investigated for divergence. If this list contains more than two PoIs, a new bisection run will be performed for each unordered pair of PoIs."
        )]
        pois: Vec<PoiBytes>,
        #[graphql(
            default = true,
            desc = "Indicates whether to collect `graph-node`'s block cache contents during bisection runs to include in the report."
        )]
        query_block_caches: bool,
        #[graphql(
            default = true,
            desc = "Indicates whether to collect `graph-node`'s ETH call cache contents during bisection runs to include in the report."
        )]
        query_eth_call_caches: bool,
        #[graphql(
            default = true,
            desc = "Indicates whether to collect `graph-node`'s entity changes during bisection runs to include in the report."
        )]
        query_entity_changes: bool,
    ) -> Result<DivergenceInvestigationReport> {
        let ctx_data = ctx_data(ctx);
        let store = &ctx_data.store;

        let req = DivergenceInvestigationRequest {
            pois,
            query_block_caches,
            query_eth_call_caches,
            query_entity_changes,
        };
        let request_serialized = serde_json::to_value(req).unwrap();
        let uuid = store
            .create_divergence_investigation_request(request_serialized)
            .await?;

        let report = DivergenceInvestigationReport {
            uuid: uuid.clone(),
            status: DivergenceInvestigationStatus::Pending,
            bisection_runs: vec![],
            error: None,
        };

        Ok(report)
    }

    async fn set_configuration(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "The configuration file to use")] config: serde_json::Value,
    ) -> Result<bool> {
        require_permission_level(ctx, ApiKeyPermissionLevel::Admin).await?;

        let ctx_data = ctx_data(ctx);
        let store = &ctx_data.store;

        store.overwrite_config(config).await?;

        Ok(true)
    }

    /// Create a new API key with the given permission level. You'll need to
    /// authenticate with another API key with the `admin` permission level to
    /// do this.
    async fn create_api_key(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Permission level of the API key. Use `admin` for full access.")]
        permission_level: ApiKeyPermissionLevel,
        #[graphql(
            default,
            desc = "Not-encrypted notes to store in the database alongside the API key, to be used for debugging or identification purposes."
        )]
        notes: Option<String>,
    ) -> Result<NewlyCreatedApiKey> {
        // In order to create an API key with a certain permission level, you
        // need to have that permission level yourself.
        require_permission_level(ctx, permission_level).await?;

        let ctx_data = ctx_data(ctx);

        let api_key = ctx_data
            .store
            .create_api_key(notes.as_deref(), permission_level)
            .await?;

        Ok(api_key)
    }

    async fn delete_api_key(&self, ctx: &Context<'_>, api_key: String) -> Result<bool> {
        let ctx_data = ctx_data(ctx);

        ctx_data.store.delete_api_key(&api_key).await?;

        Ok(true)
    }

    async fn modify_api_key(
        &self,
        ctx: &Context<'_>,
        api_key: String,
        #[graphql(
            desc = "Not-encrypted notes to store in the database alongside the API key, to be used for debugging or identification purposes."
        )]
        notes: Option<String>,
        permission_level: ApiKeyPermissionLevel,
    ) -> Result<bool> {
        require_permission_level(ctx, permission_level).await?;

        let ctx_data = ctx_data(ctx);

        ctx_data
            .store
            .modify_api_key(&api_key, notes.as_deref(), permission_level)
            .await?;

        Ok(true)
    }

    async fn set_deployment_name(
        &self,
        ctx: &Context<'_>,
        deployment_ipfs_cid: String,
        name: String,
    ) -> Result<Deployment> {
        let ctx_data = ctx_data(ctx);
        let store = &ctx_data.store;

        store
            .set_deployment_name(&deployment_ipfs_cid, &name)
            .await?;

        Ok(Deployment {
            id: deployment_ipfs_cid,
        })
    }

    /// Completely deletes a network and all related data (PoIs, indexers, subgraphs, etc.).
    async fn delete_network(&self, ctx: &Context<'_>, network: String) -> Result<String> {
        let ctx_data = ctx_data(ctx);
        ctx_data.store.delete_network(&network).await?;

        Ok(network)
    }
}
