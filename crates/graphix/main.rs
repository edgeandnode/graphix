use std::net::Ipv4Addr;

use async_graphql::http::GraphiQLSource;
use async_graphql_axum::GraphQL;
use axum::response::IntoResponse;
use axum::Router;
use clap::Parser;
use graphix_lib::graphql_api::{self};
use graphix_lib::store::Store;
use tokio::net::TcpListener;

#[derive(Parser, Debug)]
pub struct CliOptions {
    #[clap(long, default_value = "80", env = "GRAPHIX_PORT")]
    pub port: u16,

    #[clap(
        long,
        default_value = "postgresql://localhost:5432/graphix",
        env = "GRAPHIX_DATABASE_URL"
    )]
    pub database_url: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let cli_options = CliOptions::parse();

    // Listen to requests forever.
    axum::serve(
        TcpListener::bind((Ipv4Addr::UNSPECIFIED, cli_options.port)).await?,
        axum_server(cli_options).await?,
    )
    .await?;

    Ok(())
}

async fn axum_server(cli_options: CliOptions) -> anyhow::Result<Router<()>> {
    use axum::routing::get;

    let store = Store::new(cli_options.database_url.as_str()).await?;
    let api_schema = graphql_api::api_schema(graphql_api::ApiSchemaContext { store });

    Ok(axum::Router::new()
        .route("/", get(|| async { "Ready to roll!" }))
        .route(
            "/graphql",
            get(graphiql_route).post_service(GraphQL::new(api_schema)),
        ))
}

fn init_tracing() {
    tracing_subscriber::fmt::init();
}

async fn graphiql_route() -> impl IntoResponse {
    axum::response::Html(GraphiQLSource::build().endpoint("/graphql").finish())
}
