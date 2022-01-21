use std::sync::Arc;

use async_graphql::{
    Context, EmptyMutation, EmptySubscription, InputObject, Object, Schema, SimpleObject,
};
use diesel::{
    r2d2::{ConnectionManager, Pool},
    ExpressionMethods, PgConnection, QueryDsl, RunQueryDsl,
};
use graph_ixi_common::db::{self, models};

pub struct QueryRoot;

#[derive(InputObject)]
struct ProofOfIndexingRequest {
    deployments: Vec<String>,
    block_range: Option<BlockRange>,
    limit: Option<u16>,
}

#[derive(InputObject)]
struct BlockRange {
    start: u64,
    end: u64,
}

#[derive(SimpleObject)]
struct PartialBlock {
    number: i64,
    hash: Option<String>,
}

#[derive(SimpleObject)]
struct ProofOfIndexing {
    timestamp: String,
    deployment: String,
    indexer: String,
    proof_of_indexing: String,
    block: PartialBlock,
}

impl From<models::ProofOfIndexing> for ProofOfIndexing {
    fn from(poi: models::ProofOfIndexing) -> Self {
        Self {
            timestamp: poi.timestamp.to_string(),
            deployment: poi.deployment,
            indexer: poi.indexer,
            block: PartialBlock {
                number: poi.block_number,
                hash: poi.block_hash,
            },
            proof_of_indexing: poi.proof_of_indexing,
        }
    }
}

#[Object]
impl QueryRoot {
    async fn pois(
        &self,
        ctx: &Context<'_>,
        request: ProofOfIndexingRequest,
    ) -> Result<Vec<ProofOfIndexing>, async_graphql::Error> {
        use db::schema::proofs_of_indexing::dsl::*;

        let api_ctx = ctx.data::<APISchemaContext>()?;
        let connection = api_ctx.db_connection_pool.get()?;

        let query = proofs_of_indexing
            .order_by(block_number.desc())
            .order_by(timestamp.desc())
            .filter(deployment.eq_any(&request.deployments))
            .filter(
                block_number.between(
                    request
                        .block_range
                        .as_ref()
                        .map_or(0, |range| range.start as i64),
                    request
                        .block_range
                        .map_or(i64::max_value(), |range| range.end as i64),
                ),
            )
            .limit(request.limit.unwrap_or(1000) as i64);

        Ok(query
            .load::<models::ProofOfIndexing>(&connection)?
            .into_iter()
            .map(ProofOfIndexing::from)
            .collect())
    }
}

pub type APISchema = Schema<QueryRoot, EmptyMutation, EmptySubscription>;

pub struct APISchemaContext {
    pub db_connection_pool: Arc<Pool<ConnectionManager<PgConnection>>>,
}

pub fn api_schema(ctx: APISchemaContext) -> APISchema {
    Schema::build(QueryRoot, EmptyMutation, EmptySubscription)
        .data(ctx)
        .finish()
}
