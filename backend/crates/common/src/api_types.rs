//! GraphQL API types.

use std::collections::BTreeMap;

use crate::store::{models, Store};
use anyhow::Context as _;
use async_graphql::*;
use diesel::FromSqlRow;
use serde::{Deserialize, Serialize};
use tracing::debug;

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
            .map(|id| Deployment { id })
            .collect())
    }

    async fn indexers(&self, ctx: &Context<'_>) -> Result<Vec<Indexer>, async_graphql::Error> {
        let api_ctx = ctx.data::<APISchemaContext>()?;
        let indexers = api_ctx.store.indexers()?;

        Ok(indexers.into_iter().map(Indexer::from).collect())
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

    async fn live_proofs_of_indexing(
        &self,
        ctx: &Context<'_>,
        request: ProofOfIndexingRequest,
    ) -> Result<Vec<ProofOfIndexing>, async_graphql::Error> {
        let api_ctx = ctx.data::<APISchemaContext>()?;
        let pois = api_ctx.store.live_pois(
            None,
            Some(&request.deployments),
            request.block_range,
            request.limit,
        )?;

        Ok(pois.into_iter().map(ProofOfIndexing::from).collect())
    }

    async fn poi_agreement_ratios(
        &self,
        ctx: &Context<'_>,
        indexer_name: String,
    ) -> Result<Vec<POIAgreementRatio>, async_graphql::Error> {
        let api_ctx = ctx.data::<APISchemaContext>()?;

        // Query live POIs of a the requested indexer.
        let indexer_pois = api_ctx
            .store
            .live_pois(Some(&indexer_name), None, None, None)?;

        let deployment_cids: Vec<_> = indexer_pois
            .iter()
            .map(|poi| poi.sg_deployment.cid.clone())
            .collect();

        // Query all live POIs for the specific deployments.
        let all_deployment_pois =
            api_ctx
                .store
                .live_pois(None, Some(&deployment_cids), None, None)?;

        // Convert POIs to ProofOfIndexing and group by deployment
        let mut deployment_to_pois: BTreeMap<String, Vec<ProofOfIndexing>> = BTreeMap::new();
        for poi in all_deployment_pois {
            let proof_of_indexing: ProofOfIndexing = poi.into();
            deployment_to_pois
                .entry(proof_of_indexing.deployment.id.clone())
                .or_default()
                .push(proof_of_indexing);
        }

        let mut agreement_ratios: Vec<POIAgreementRatio> = Vec::new();

        for poi in indexer_pois {
            let poi: ProofOfIndexing = poi.into();

            let deployment = Deployment {
                id: poi.deployment.id.clone(),
            };

            let block = PartialBlock {
                number: poi.block.number as i64,
                hash: Some(poi.block.hash),
            };

            let deployment_pois = deployment_to_pois
                .get(&poi.deployment.id)
                .context("inconsistent pois table, no pois for deployment")?;

            let total_indexers = deployment_pois.len() as i32;

            // Calculate POI agreement by creating a map to count unique POIs and their occurrence.
            let mut poi_counts: BTreeMap<String, i32> = BTreeMap::new();
            for dp in deployment_pois {
                *poi_counts.entry(dp.hash.clone()).or_insert(0) += 1;
            }

            // Define consensus and agreement based on the map.
            let (max_poi, max_poi_count) = poi_counts
                .iter()
                .max_by_key(|(_, &v)| v)
                .context("inconsistent pois table, no pois")?;

            let has_consensus = *max_poi_count > total_indexers / 2;

            let n_agreeing_indexers = *poi_counts
                .get(&poi.hash)
                .context("inconsistent pois table, no matching poi")?;

            let n_disagreeing_indexers = total_indexers - n_agreeing_indexers;

            let in_consensus = has_consensus && max_poi == &poi.hash;

            let ratio = POIAgreementRatio {
                poi: poi.hash.clone(),
                deployment,
                block: block,
                total_indexers,
                n_agreeing_indexers,
                n_disagreeing_indexers,
                has_consensus,
                in_consensus,
            };

            agreement_ratios.push(ratio);
        }

        Ok(agreement_ratios)
    }

    async fn poi_cross_check_report(
        &self,
        ctx: &Context<'_>,
        request_id: String,
    ) -> Result<String, async_graphql::Error> {
        let api_ctx = ctx.data::<APISchemaContext>()?;
        let request = api_ctx.store.cross_check_report(&request_id)?;

        Ok(serde_json::to_string(&request).unwrap())
    }
}

pub struct MutationRoot;

impl MutationRoot {}

#[Object]
impl MutationRoot {
    async fn launch_cross_check_report(
        &self,
        ctx: &Context<'_>,
        req: DivergenceInvestigationRequest,
    ) -> Result<DivergenceInvestigationResponse> {
        debug!("launch_cross_check_report");

        let api_ctx = ctx.data::<APISchemaContext>()?;
        let store = &api_ctx.store;

        let id = store.queue_cross_check_report(req)?.to_string();

        Ok(DivergenceInvestigationResponse { id })
    }
}

#[derive(InputObject, Serialize, Deserialize, Debug, Clone, FromSqlRow)]
pub struct DivergenceInvestigationRequest {
    pub poi1: String,
    pub poi2: String,
    pub query_block_caches: bool,
    pub query_eth_call_caches: bool,
    pub query_entity_changes: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, FromSqlRow)]
pub struct DivergenceInvestigationRequestWithUuid {
    pub id: String,
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

#[derive(SimpleObject, Debug)]
pub struct Network {
    pub name: String,
    pub caip2: Option<String>,
}

#[derive(SimpleObject, Debug)]
pub struct Block {
    pub network: Network,
    pub number: u64,
    pub hash: HexBytesWith0xPrefix,
}

#[derive(SimpleObject)]
struct DivergenceInvestigationResponse {
    id: String,
}

/// A block number that may or may not also have an associated hash.
#[derive(SimpleObject)]
struct PartialBlock {
    number: i64,
    hash: Option<String>,
}

#[derive(SimpleObject, Debug)]
struct Deployment {
    id: String,
}

#[derive(SimpleObject, Debug)]
struct ProofOfIndexing {
    block: Block,
    hash: String,
    deployment: Deployment,
    allocated_tokens: Option<u64>,
    indexer: Indexer,
}

#[derive(SimpleObject, Debug)]
struct Indexer {
    id: HexBytesWith0xPrefix,
    allocated_tokens: Option<u64>,
}

impl From<models::Indexer> for Indexer {
    fn from(indexer: models::Indexer) -> Self {
        Self {
            id: indexer.name.unwrap_or_default(),
            allocated_tokens: None, // TODO: we don't store this in the db yet
        }
    }
}

impl From<models::PoI> for ProofOfIndexing {
    fn from(poi: models::PoI) -> Self {
        Self {
            allocated_tokens: None,
            deployment: Deployment {
                id: poi.sg_deployment.cid.clone(),
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
            indexer: Indexer::from(models::Indexer::from(poi.indexer)),
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

/// A specific indexer can use `POIAgreementRatio` to check in how much agreement it is with other
/// indexers, given its own poi for each deployment. A consensus currently means a majority of
/// indexers agreeing on a particular POI.
#[derive(SimpleObject)]
#[graphql(name = "POIAgreementRatio")]
struct POIAgreementRatio {
    poi: String,
    deployment: Deployment,
    block: PartialBlock,

    /// Total number of indexers that have live pois for the deployment.
    total_indexers: i32,

    /// Number of indexers that agree on the POI with the specified indexer,
    /// including the indexer itself.
    n_agreeing_indexers: i32,

    /// Number of indexers that disagree on the POI with the specified indexer.
    n_disagreeing_indexers: i32,

    /// Indicates if a consensus on the POI exists among indexers.
    has_consensus: bool,

    /// Indicates if the specified indexer's POI is part of the consensus.
    in_consensus: bool,
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
