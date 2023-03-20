extern crate diesel;

#[macro_use]
extern crate diesel_migrations;

use std::{convert::Infallible, sync::Arc};

use async_graphql::{
    http::{playground_source, GraphQLPlaygroundConfig},
    Request,
};
use async_graphql_warp::{self, GraphQLResponse};
use diesel::{r2d2, PgConnection};
use tracing::*;
use tracing_subscriber::{self, layer::SubscriberExt as _, util::SubscriberInitExt as _};
use warp::{
    http::{self, Method},
    Filter,
};

use graph_ixi_common::{api_schema as schema, db::Store};

mod opt;

embed_migrations!("../migrations");

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
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

    let store = Store::new(options.database_url.as_str())?;

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

    // Run the API server
    warp::serve(routes)
        .run(([127, 0, 0, 1], options.port))
        .await;

    Ok(())
}
