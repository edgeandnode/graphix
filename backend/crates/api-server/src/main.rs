use async_graphql::{
    http::{playground_source, GraphQLPlaygroundConfig},
    Request,
};
use async_graphql_warp::{self, GraphQLResponse};
use clap::Parser;
use graphix_common::{api_types as schema, db::Store};
use std::{convert::Infallible, net::Ipv4Addr};
use warp::{
    http::{self, Method},
    Filter,
};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    init_tracing();

    let cli_options = CliOptions::parse();
    let store = Store::new(cli_options.database_url.as_str())?;

    run_api_server(cli_options, store).await;

    Ok(())
}

async fn run_api_server(options: CliOptions, store: Store) {
    // GET / -> 200 OK
    let health_check_route = warp::path::end().map(|| format!("Ready to roll!"));

    // GraphQL API
    let api_context = schema::APISchemaContext { store };
    let api_schema = schema::api_schema(api_context);
    let api = async_graphql_warp::graphql(api_schema).and_then(
        |(schema, request): (schema::APISchema, Request)| async move {
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

    warp::serve(routes)
        .run((Ipv4Addr::LOCALHOST, options.port))
        .await;
}

fn init_tracing() {
    tracing_subscriber::fmt::init();
}

#[derive(Parser, Debug)]
pub struct CliOptions {
    #[clap(long, default_value = "80")]
    pub port: u16,

    #[clap(long, default_value = "postgresql://localhost:5432/graphix")]
    pub database_url: String,
}
