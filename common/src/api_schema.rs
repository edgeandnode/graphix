use async_graphql::{
    Context, EmptyMutation, EmptySubscription, InputObject, Object, Schema, SimpleObject,
};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};

use crate::db::{self, models, Store};

pub struct QueryRoot;

#[derive(InputObject)]
struct ProofOfIndexingRequest {
    deployments: Vec<String>,
    block_range: Option<BlockRange>,
    limit: Option<u16>,
}

#[derive(InputObject, Copy, Clone)]
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

#[derive(InputObject)]
#[graphql(input_name = "POICrossCheckReportRequest")]
struct POICrossCheckReportRequest {
    deployments: Vec<String>,
    indexer1: Option<String>,
    indexer2: Option<String>,
}

#[derive(SimpleObject)]
struct DivergingBlock {
    block: PartialBlock,
    proof_of_indexing1: String,
    proof_of_indexing2: String,
}

#[derive(SimpleObject)]
#[graphql(name = "POICrossCheckReport")]
struct POICrossCheckReport {
    timestamp: String,
    indexer1: String,
    indexer2: String,
    deployment: String,
    block: PartialBlock,
    proof_of_indexing1: String,
    proof_of_indexing2: String,
    diverging_block: Option<DivergingBlock>,
}

impl From<models::POICrossCheckReport> for POICrossCheckReport {
    fn from(report: models::POICrossCheckReport) -> Self {
        Self {
            timestamp: report.timestamp.to_string(),
            indexer1: report.indexer1,
            indexer2: report.indexer2,
            deployment: report.deployment,
            block: PartialBlock {
                number: report.block_number,
                hash: report.block_hash,
            },
            proof_of_indexing1: report.proof_of_indexing1,
            proof_of_indexing2: report.proof_of_indexing2,
            diverging_block: report.diverging_block.map(|block| DivergingBlock {
                block: PartialBlock {
                    number: block.block_number,
                    hash: block.block_hash,
                },
                proof_of_indexing1: block.proof_of_indexing1,
                proof_of_indexing2: block.proof_of_indexing2,
            }),
        }
    }
}

#[Object]
impl QueryRoot {
    async fn deployments(&self, ctx: &Context<'_>) -> Result<Vec<String>, async_graphql::Error> {
        let api_ctx = ctx.data::<APISchemaContext>()?;
        Ok(api_ctx.db.deployments()?)
    }

    async fn proofs_of_indexing(
        &self,
        ctx: &Context<'_>,
        request: ProofOfIndexingRequest,
    ) -> Result<Vec<ProofOfIndexing>, async_graphql::Error> {
        let api_ctx = ctx.data::<APISchemaContext>()?;

        let block_range = request
            .block_range
            .map(|BlockRange { start, end }| start..=end)
            .unwrap_or(0..=u64::MAX);

        let pois = api_ctx.db.pois(
            &request.deployments[..],
            block_range,
            request.limit.unwrap_or(1000) as _,
        )?;

        Ok(pois.into_iter().map(ProofOfIndexing::from).collect())
    }

    async fn poi_cross_check_reports(
        &self,
        ctx: &Context<'_>,
        request: POICrossCheckReportRequest,
    ) -> Result<Vec<POICrossCheckReport>, async_graphql::Error> {
        use db::schema::poi_cross_check_reports::dsl::*;

        let api_ctx = ctx.data::<APISchemaContext>()?;
        let connection = api_ctx.db.connection_pool.get()?;

        let mut query = poi_cross_check_reports
            .distinct_on((block_number, indexer1, indexer2, deployment))
            .into_boxed();

        if let Some(indexer) = request.indexer1 {
            query = query.filter(indexer1.eq(indexer));
        }

        if let Some(indexer) = request.indexer2 {
            query = query.filter(indexer2.eq(indexer));
        }

        query = query
            .order_by((
                block_number.desc(),
                deployment.asc(),
                indexer1.asc(),
                indexer2.asc(),
            ))
            .limit(5000);

        Ok(query
            .load::<models::POICrossCheckReport>(&connection)?
            .into_iter()
            .map(POICrossCheckReport::from)
            .collect())
    }
}

pub type APISchema = Schema<QueryRoot, EmptyMutation, EmptySubscription>;

pub struct APISchemaContext {
    pub db: Store,
}

pub fn api_schema(ctx: APISchemaContext) -> APISchema {
    Schema::build(QueryRoot, EmptyMutation, EmptySubscription)
        .data(ctx)
        .finish()
}
