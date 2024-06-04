pub mod api_types;
mod mutation_root;
mod query_root;

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use async_graphql::dataloader::DataLoader;
use async_graphql::http::GraphiQLSource;
use async_graphql::{Context, EmptySubscription, Schema, SchemaBuilder};
use async_graphql_axum::GraphQL;
use axum::extract::State;
use axum::http::header::AUTHORIZATION;
use axum::http::StatusCode;
use axum::Json;
use graphix_common_types::ApiKeyPermissionLevel;
use graphix_store::models::ApiKey;
use graphix_store::{Store, StoreLoader};
use tower_service::Service;

use self::mutation_root::MutationRoot;
use self::query_root::QueryRoot;
use crate::config::Config;
use crate::GRAPHIX_VERSION;

pub type ApiSchema = Schema<QueryRoot, MutationRoot, EmptySubscription>;

#[derive(derive_more::Deref)]
pub struct RequestState {
    api_key: Option<ApiKey>,
    #[deref]
    data: Arc<ServerState>,
}

pub struct ServerState {
    pub store: Store,
    pub config: Config,
    pub loader_poi: DataLoader<StoreLoader<graphix_store::models::Poi>>,
    pub loader_network: DataLoader<StoreLoader<graphix_store::models::Network>>,
    pub loader_graph_node_collected_version:
        DataLoader<StoreLoader<graphix_store::models::GraphNodeCollectedVersion>>,
    pub loader_indexer_network_subgraph_metadata:
        DataLoader<StoreLoader<graphix_store::models::IndexerNetworkSubgraphMetadata>>,
    pub loader_block: DataLoader<StoreLoader<graphix_store::models::Block>>,
    pub loader_indexer: DataLoader<StoreLoader<graphix_store::models::Indexer>>,
    pub loader_subgraph_deployment: DataLoader<StoreLoader<graphix_store::models::SgDeployment>>,
}

impl ServerState {
    pub fn new(store: Store, config: Config) -> Self {
        // The default delay is 1ms, but we're happy to wait a bit longer to reduce load on the
        // database.
        let delay = Duration::from_millis(3);

        let loader_poi =
            DataLoader::new(StoreLoader::new(store.clone()), tokio::task::spawn).delay(delay);
        let loader_network =
            DataLoader::new(StoreLoader::new(store.clone()), tokio::task::spawn).delay(delay);
        let loader_graph_node_collected_version =
            DataLoader::new(StoreLoader::new(store.clone()), tokio::task::spawn).delay(delay);
        let loader_indexer_network_subgraph_metadata =
            DataLoader::new(StoreLoader::new(store.clone()), tokio::task::spawn).delay(delay);
        let loader_block =
            DataLoader::new(StoreLoader::new(store.clone()), tokio::task::spawn).delay(delay);
        let loader_indexer =
            DataLoader::new(StoreLoader::new(store.clone()), tokio::task::spawn).delay(delay);
        let loader_subgraph_deployment =
            DataLoader::new(StoreLoader::new(store.clone()), tokio::task::spawn).delay(delay);

        Self {
            store,
            config,
            loader_poi,
            loader_network,
            loader_graph_node_collected_version,
            loader_indexer_network_subgraph_metadata,
            loader_block,
            loader_indexer,
            loader_subgraph_deployment,
        }
    }
}

pub fn api_schema_builder() -> SchemaBuilder<QueryRoot, MutationRoot, EmptySubscription> {
    Schema::build(QueryRoot, MutationRoot, EmptySubscription).enable_federation()
}

pub fn ctx_data<'a>(ctx: &'a Context) -> &'a RequestState {
    ctx.data::<RequestState>()
        .expect("Failed to get API context")
}

pub async fn axum_router(database_url: &str, config: Config) -> anyhow::Result<axum::Router<()>> {
    use axum::routing::get;

    let store = Store::new(database_url).await?;
    let server_state = ServerState::new(store.clone(), config.clone());

    Ok(axum::Router::new()
        .route(
            "/",
            get(|| async {
                format!(
                    "Welcome to Graphix v{}. Go to `/graphql` to use the playground.",
                    GRAPHIX_VERSION
                )
            }),
        )
        .route("/graphql", get(graphiql_route).post(graphql_handler))
        .with_state(Arc::new(server_state)))
}

async fn graphql_handler(
    State(state): State<Arc<ServerState>>,
    request: axum::extract::Request,
) -> Result<axum::response::Response, (StatusCode, Json<serde_json::Value>)> {
    let api_key = match request.headers().get(AUTHORIZATION) {
        None => None,
        Some(value) => {
            let header_s = value.to_str().map_err(api_key_error)?;
            let api_key = ApiKey::from_str(header_s).map_err(api_key_error)?;

            Some(api_key)
        }
    };

    let api_schema = api_schema_builder()
        .data(RequestState {
            api_key,
            data: state.clone(),
        })
        .finish();

    let mut service = GraphQL::new(api_schema);
    Ok(service
        .call(request)
        .await
        .map_err(|_| api_key_error("Internal server error"))?)
}

fn api_key_error(err: impl ToString) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({
            "message": "Invalid API key",
            "error": err.to_string(),
        })),
    )
}

async fn graphiql_route() -> impl axum::response::IntoResponse {
    axum::response::Html(GraphiQLSource::build().endpoint("/graphql").finish())
}

async fn require_permission_level(
    ctx: &Context<'_>,
    required_permission_level: ApiKeyPermissionLevel,
) -> async_graphql::Result<()> {
    let ctx_data = ctx_data(ctx);
    let api_key = ctx_data
        .api_key
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No API key provided"))?;

    let Some(actual_permission_level) = ctx_data.store.permission_level(&api_key).await? else {
        return Err(anyhow::anyhow!("No permission level for API key").into());
    };

    if actual_permission_level < required_permission_level {
        return Err(anyhow::anyhow!(
            "Insufficient permission level for API key: expected {:?}, got {:?}",
            required_permission_level,
            actual_permission_level
        )
        .into());
    }

    Ok(())
}
