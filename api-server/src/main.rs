use std::convert::Infallible;

use async_graphql::{
    http::{playground_source, GraphQLPlaygroundConfig},
    Request,
};
use async_graphql_warp::{self, GraphQLResponse};
use tracing::*;
use tracing_subscriber::{self, layer::SubscriberExt as _, util::SubscriberInitExt as _};
use warp::{
    http::{self, Method},
    Filter,
};

mod opt;
mod schema;

#[tokio::main]
async fn main() {
    let filter_layer = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or(
        tracing_subscriber::EnvFilter::try_new(
            "info,graph_ixi_common=debug,graph_ixi_api_server=debug",
        )
        .unwrap(),
    );
    let defaults = tracing_subscriber::registry().with(filter_layer);
    let fmt_layer = tracing_subscriber::fmt::layer();
    defaults.with(fmt_layer).init();

    let options = opt::options_from_args();

    // GET / -> 200 OK
    let health_check_route = warp::path::end().map(|| format!("Ready to roll!"));

    // GraphQL API
    let api = async_graphql_warp::graphql(schema::api_schema()).and_then(
        |(schema, request): (schema::ApiSchema, Request)| async move {
            Ok::<_, Infallible>(GraphQLResponse::from(schema.execute(request).await))
        },
    );
    let cors = warp::cors().allow_methods(&[Method::GET, Method::POST]);
    let graphql_route = warp::path("graphql")
        .and(warp::path::end())
        .and(api)
        .with(cors);

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

    // Run the API server
    warp::serve(routes)
        .run(([127, 0, 0, 1], options.port))
        .await;
}
