use std::convert::Infallible;
use std::future::Future;

use async_graphql::http::{playground_source, GraphQLPlaygroundConfig};
use async_graphql::Request;
use async_graphql_warp::{self, GraphQLResponse};
use clap::Parser;
use graphix_common::graphql_api::{self};
use graphix_common::store::Store;
use warp::http::{self, Method};
use warp::Filter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let cli_options = CliOptions::parse();
    let server_fut = create_server(cli_options);

    // Listen to requests forever.
    Ok(server_fut.await?.await)
}

async fn create_server(cli_options: CliOptions) -> anyhow::Result<impl Future<Output = ()>> {
    let store = Store::new(cli_options.database_url.as_str()).await?;

    // GET / -> 200 OK
    let health_check_route = warp::path::end().map(|| format!("Ready to roll!"));

    // GraphQL API
    let api_context = graphql_api::ApiSchemaContext { store };
    let api_schema = graphql_api::api_schema(api_context);
    let api = async_graphql_warp::graphql(api_schema).and_then(
        |(schema, request): (graphql_api::ApiSchema, Request)| async move {
            Ok::<_, Infallible>(GraphQLResponse::from(schema.execute(request).await))
        },
    );
    let cors = warp::cors()
        .allow_methods(&[Method::GET, Method::POST, Method::OPTIONS])
        .allow_header("content-type")
        .allow_any_origin();
    let graphql_route = warp::any()
        .and(warp::path("graphql").and(warp::path::end()))
        .and(api)
        .with(cors.clone());

    // GraphQL playground
    let graphql_playground = warp::get().map(|| {
        http::Response::builder()
            .header("content-type", "text/html")
            .body(playground_source(GraphQLPlaygroundConfig::new("/graphql")))
    });
    let graphql_playground_route = warp::get()
        .and(warp::path("graphql").and(warp::path::end()))
        .and(graphql_playground);

    let routes = warp::get()
        .and(health_check_route)
        .or(graphql_playground_route)
        .or(graphql_route);

    Ok(warp::serve(routes).run(([0, 0, 0, 0], cli_options.port)))
}

fn init_tracing() {
    tracing_subscriber::fmt::init();
}

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
