use std::convert::Infallible;
use std::future::Future;
use std::net::{Ipv4Addr, SocketAddrV4};

use async_graphql::http::{playground_source, GraphQLPlaygroundConfig};
use async_graphql::Request;
use async_graphql_warp::{self, GraphQLResponse};
use clap::Parser;
use graphix_common::graphql_api::{self};
use graphix_common::store::Store;
use warp::http::{self, Method};
use warp::Filter;

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

    #[clap(long, env = "GRAPHIX_ADMIN_BEARER_TOKEN")]
    pub admin_bearer_token: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let cli_options = CliOptions::parse();
    let server_fut = create_server(cli_options);

    // Listen to requests forever.
    Ok(server_fut.await?.await)
}

fn init_tracing() {
    tracing_subscriber::fmt::init();
}

async fn create_server(cli_options: CliOptions) -> anyhow::Result<impl Future<Output = ()>> {
    let store = Store::new(cli_options.database_url.as_str()).await?;

    // GET / -> 200 OK
    let health_check_route = warp::path::end().map(|| format!("Ready to roll!"));

    // GraphQL API
    let api_context = graphql_api::ApiSchemaContext {
        store: store.clone(),
    };
    let api_schema = graphql_api::api_schema(api_context);
    let api = async_graphql_warp::graphql(api_schema).and_then(
        |(schema, request): (graphql_api::ApiSchema, Request)| async move {
            Ok::<_, Infallible>(GraphQLResponse::from(schema.execute(request).await))
        },
    );
    let graphql_route = warp::any()
        .and(warp::path("graphql").and(warp::path::end()))
        .and(api)
        .with(cors_filter());

    // GraphQL playground
    let graphql_playground = warp::get().map(|| {
        http::Response::builder()
            .header("content-type", "text/html")
            .body(playground_source(GraphQLPlaygroundConfig::new("/graphql")))
    });
    let graphql_playground_route = warp::get()
        .and(warp::path("graphql").and(warp::path::end()))
        .and(graphql_playground);

    let admin_api_context = graphql_api::admin::ApiSchemaContext { store };
    let admin_api_schema = graphql_api::admin::api_schema(admin_api_context);
    let admin_api = async_graphql_warp::graphql(admin_api_schema).and_then(
        |(schema, request): (graphql_api::admin::ApiSchema, Request)| async move {
            Ok::<_, Infallible>(GraphQLResponse::from(schema.execute(request).await))
        },
    );
    let admin_graphql_route = warp::any()
        .and(warp::path("admin/graphql").and(warp::path::end()))
        .and(admin_api)
        .with(cors_filter());

    let routes = warp::get()
        .and(health_check_route)
        .or(graphql_playground_route)
        .or(graphql_route)
        .or(admin_graphql_route);

    let socket_addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, cli_options.port);
    Ok(warp::serve(routes).run(socket_addr))
}

fn cors_filter() -> warp::cors::Builder {
    warp::cors()
        .allow_methods(&[Method::GET, Method::POST, Method::OPTIONS])
        .allow_header("content-type")
        .allow_any_origin()
}
