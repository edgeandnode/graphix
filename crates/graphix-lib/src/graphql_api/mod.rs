mod server;

use async_graphql::{EmptySubscription, Schema, SchemaBuilder};
use graphix_store::Store;

use self::server::{MutationRoot, QueryRoot};

pub type ApiSchema = Schema<QueryRoot, MutationRoot, EmptySubscription>;

pub struct ApiSchemaContext {
    pub store: Store,
}

pub fn api_schema_builder() -> SchemaBuilder<QueryRoot, MutationRoot, EmptySubscription> {
    Schema::build(QueryRoot, MutationRoot, EmptySubscription)
}

pub fn api_schema(ctx: ApiSchemaContext) -> ApiSchema {
    api_schema_builder().data(ctx).finish()
}
