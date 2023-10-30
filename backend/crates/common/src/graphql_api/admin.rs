use async_graphql::{Context, EmptySubscription, Object, Result, Schema};

use super::types::Deployment;
use crate::prelude::Store;

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn _empty_method(&self, _ctx: &Context<'_>) -> Result<String> {
        Ok("".to_string())
    }
}

pub struct MutationRoot;

#[Object]
impl MutationRoot {
    async fn set_deployment_name(
        &self,
        ctx: &Context<'_>,
        deployment_ipfs_cid: String,
        name: String,
    ) -> Result<Deployment> {
        let api_ctx = ctx.data::<ApiSchemaContext>()?;
        let store = &api_ctx.store;

        store.set_deployment_name(&deployment_ipfs_cid, &name)?;

        Ok(Deployment {
            id: deployment_ipfs_cid,
        })
    }

    async fn delete_network(&self, ctx: &Context<'_>, network: String) -> Result<String> {
        let api_ctx = ctx.data::<ApiSchemaContext>()?;
        let store = &api_ctx.store;

        store.delete_network(&network)?;

        Ok(network)
    }
}

pub type ApiSchema = Schema<QueryRoot, MutationRoot, EmptySubscription>;

pub struct ApiSchemaContext {
    pub store: Store,
}

pub fn api_schema(ctx: ApiSchemaContext) -> ApiSchema {
    Schema::build(QueryRoot, MutationRoot, EmptySubscription)
        .data(ctx)
        .finish()
}
