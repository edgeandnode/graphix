use anyhow;
use async_graphql::{Context, EmptyMutation, EmptySubscription, Object, Schema};

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn value(&self, ctx: &Context<'_>) -> Result<i32, anyhow::Error> {
        Ok(1)
    }
}

pub type ApiSchema = Schema<QueryRoot, EmptyMutation, EmptySubscription>;

pub fn api_schema() -> ApiSchema {
    Schema::new(QueryRoot, EmptyMutation, EmptySubscription)
}
