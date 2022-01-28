use async_graphql::{EmptyMutation, EmptySubscription, Schema};

use graph_ixi_common::api_schema::QueryRoot;

#[tokio::main]
async fn main() {
    let schema = Schema::build(QueryRoot, EmptyMutation, EmptySubscription).finish();

    // Print the schema in SDL format
    println!("{}", &schema.sdl());
}
