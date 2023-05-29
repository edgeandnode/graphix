//! GraphQL API types.

use crate::db::{models, Store};
use async_graphql::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

type HexBytesWith0xPrefix = String;

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn deployments(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Vec<Deployment>, async_graphql::Error> {
        let api_ctx = ctx.data::<APISchemaContext>()?;
        let deployments = api_ctx.store.sg_deployments()?;

        Ok(deployments
            .into_iter()
            .map(|id| Deployment {
                id,
                pois_count: None,
            })
            .collect())
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

pub struct MutationRoot;

#[Object]
impl MutationRoot {
    async fn launch_cross_check_report(
        &self,
        ctx: &Context<'_>,
        req: DivergenceInvestigationRequest,
    ) -> Result<String> {
        let api_ctx = ctx.data::<APISchemaContext>()?;
        let store = &api_ctx.store;

        let id = store.queue_cross_check_report(req)?;

        Ok(id.to_string())
    }
}

#[derive(InputObject, Serialize, Deserialize, Debug, Clone)]
pub struct DivergenceInvestigationRequest {
    pub poi1: String,
    pub poi2: String,
    pub query_block_caches: bool,
    pub query_eth_call_caches: bool,
    pub query_entity_changes: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DivergenceInvestigationRequestWithUuid {
    pub uuid: Uuid,
    pub req: DivergenceInvestigationRequest,
}

#[derive(InputObject)]
struct ProofOfIndexingRequest {
    deployments: Vec<String>,
    block_range: Option<BlockRangeInput>,
    limit: Option<u16>,
}

#[derive(InputObject)]
pub struct BlockRangeInput {
    pub start: Option<u64>,
    pub end: Option<u64>,
}

#[derive(SimpleObject)]
pub struct Network {
    pub name: String,
    pub caip2: Option<String>,
}

#[derive(SimpleObject)]
pub struct Block {
    pub network: Network,
    pub number: u64,
    pub hash: HexBytesWith0xPrefix,
}

/// A block number that may or may not also have an associated hash.
#[derive(SimpleObject)]
struct PartialBlock {
    number: i64,
    hash: Option<String>,
}

#[derive(SimpleObject)]
struct Deployment {
    id: String,
    pois_count: Option<u64>,
}

#[derive(SimpleObject)]
struct ProofOfIndexing {
    block: Block,
    hash: String,
    deployment: Deployment,
    allocated_tokens: Option<u64>,
    indexers: Vec<Indexer>,
}

#[derive(SimpleObject)]
struct Indexer {
    id: HexBytesWith0xPrefix,
    allocated_tokens: Option<u64>,
}

impl From<models::PoI> for ProofOfIndexing {
    fn from(poi: models::PoI) -> Self {
        Self {
            allocated_tokens: None,
            deployment: Deployment {
                id: poi.sg_deployment.cid.clone(),
                pois_count: None,
            },
            hash: poi.poi_hex(),
            block: Block {
                network: Network {
                    name: "mainnet".to_string(),
                    caip2: None,
                },
                number: poi.block.number as u64,
                hash: hex::encode(poi.block.hash),
            },
            indexers: vec![],
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

pub type APISchema = Schema<QueryRoot, MutationRoot, EmptySubscription>;

pub struct APISchemaContext {
    pub store: Store,
}

pub fn api_schema(ctx: APISchemaContext) -> APISchema {
    Schema::build(QueryRoot, MutationRoot, EmptySubscription)
        .data(ctx)
        .finish()
}
