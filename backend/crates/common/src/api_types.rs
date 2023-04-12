//! GraphQL API types.

use crate::db::{models, Store};
use async_graphql::*;
//use async_graphql::{
//    Context, EmptyMutation, EmptySubscription, InputObject, Object, Schema, SimpleObject,
//};

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn deployments(&self, ctx: &Context<'_>) -> Result<Vec<String>, async_graphql::Error> {
        let api_ctx = ctx.data::<APISchemaContext>()?;
        let deployments = api_ctx.store.sg_deployments()?;

        Ok(deployments)
    }

    async fn proofs_of_indexing(
        &self,
        ctx: &Context<'_>,
        request: ProofOfIndexingRequest,
    ) -> Result<Vec<ProofOfIndexing>, async_graphql::Error> {
        let api_ctx = ctx.data::<APISchemaContext>()?;
        let pois = api_ctx
            .store
            .pois(&request.deployments, request.block_range, request.limit)?;

        Ok(pois.into_iter().map(ProofOfIndexing::from).collect())
    }

    // async fn poi_cross_check_reports(
    //     &self,
    //     ctx: &Context<'_>,
    //     request: POICrossCheckReportRequest,
    // ) -> Result<Vec<POICrossCheckReport>, async_graphql::Error> {
    //     let api_ctx = ctx.data::<APISchemaContext>()?;
    //     let reports = api_ctx
    //         .store
    //         .poi_cross_check_reports(request.indexer1.as_deref(), request.indexer2.as_deref())?;

    //     Ok(reports.into_iter().map(POICrossCheckReport::from).collect())
    // }
}

#[derive(InputObject)]
struct ProofOfIndexingRequest {
    deployments: Vec<String>,
    block_range: Option<BlockRange>,
    limit: Option<u16>,
}

#[derive(InputObject)]
pub struct BlockRange {
    pub start: u64,
    pub end: u64,
}

/// A block number that may or may not also have an associated hash.
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

impl From<models::PoI> for ProofOfIndexing {
    fn from(poi: models::PoI) -> Self {
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

// impl From<models::PoiCrossCheckReport> for POICrossCheckReport {
//     fn from(report: models::PoiCrossCheckReport) -> Self {
//         Self {
//             timestamp: report.timestamp.to_string(),
//             indexer1: report.indexer1,
//             indexer2: report.indexer2,
//             deployment: report.deployment,
//             block: PartialBlock {
//                 number: report.block_number,
//                 hash: report.block_hash,
//             },
//             proof_of_indexing1: report.proof_of_indexing1,
//             proof_of_indexing2: report.proof_of_indexing2,
//             diverging_block: report.diverging_block.map(|block| DivergingBlock {
//                 block: PartialBlock {
//                     number: block.block_number,
//                     hash: block.block_hash,
//                 },
//                 proof_of_indexing1: block.proof_of_indexing1,
//                 proof_of_indexing2: block.proof_of_indexing2,
//             }),
//         }
//     }
// }

pub type APISchema = Schema<QueryRoot, EmptyMutation, EmptySubscription>;

pub struct APISchemaContext {
    pub store: Store,
}

pub fn api_schema(ctx: APISchemaContext) -> APISchema {
    Schema::build(QueryRoot, EmptyMutation, EmptySubscription)
        .data(ctx)
        .finish()
}
