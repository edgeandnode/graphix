use std::collections::BTreeMap;

use anyhow::Context as _;
use async_graphql::{Context, Object, Result};

use super::types::*;
use crate::store::models::QueriedSgDeployment;
use crate::store::Store;

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    /// Fetches all tracked subgraph deploymens in this Graphix instance and
    /// filters them according to some filtering rules.
    async fn deployments(
        &self,
        ctx: &Context<'_>,
        filter: SgDeploymentsQuery,
    ) -> Result<Vec<QueriedSgDeployment>> {
        let api_ctx = ctx.data::<ApiSchemaContext>()?;
        Ok(api_ctx.store.sg_deployments(filter)?)
    }

    /// Fetches all tracked indexers in this Graphix instance and filters them
    /// according to some filtering rules.
    async fn indexers(&self, ctx: &Context<'_>, filter: IndexersQuery) -> Result<Vec<Indexer>> {
        let api_ctx = ctx.data::<ApiSchemaContext>()?;
        let indexers = api_ctx.store.indexers(filter)?;

        Ok(indexers.into_iter().map(Indexer::from).collect())
    }

    /// Filters through all PoIs ever collected by this Graphix
    /// instance, according to some filtering rules specified in `filter`.
    async fn proofs_of_indexing(
        &self,
        ctx: &Context<'_>,
        filter: PoisQuery,
    ) -> Result<Vec<ProofOfIndexing>> {
        let api_ctx = ctx.data::<ApiSchemaContext>()?;
        let pois = api_ctx
            .store
            .pois(&filter.deployments, filter.block_range, filter.limit)?;

        Ok(pois.into_iter().map(ProofOfIndexing::from).collect())
    }

    /// Same as [`QueryRoot::proofs_of_indexing`], but only returns PoIs that
    /// are "live" i.e. they are the most recent PoI collected for their
    /// subgraph deployment.
    async fn live_proofs_of_indexing(
        &self,
        ctx: &Context<'_>,
        filter: PoisQuery,
    ) -> Result<Vec<ProofOfIndexing>> {
        let api_ctx = ctx.data::<ApiSchemaContext>()?;
        let pois = api_ctx.store.live_pois(
            None,
            Some(&filter.deployments),
            filter.block_range,
            filter.limit,
        )?;

        Ok(pois.into_iter().map(ProofOfIndexing::from).collect())
    }

    async fn poi_agreement_ratios(
        &self,
        ctx: &Context<'_>,
        indexer_name: String,
    ) -> Result<Vec<PoiAgreementRatio>> {
        let api_ctx = ctx.data::<ApiSchemaContext>()?;

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

        let mut agreement_ratios: Vec<PoiAgreementRatio> = Vec::new();

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

            let ratio = PoiAgreementRatio {
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

    async fn divergence_investigation_report(
        &self,
        ctx: &Context<'_>,
        uuid: String,
    ) -> Result<Option<DivergenceInvestigationReport>> {
        let api_ctx = ctx.data::<ApiSchemaContext>()?;

        if let Some(report_json) = api_ctx.store.divergence_investigation_report(&uuid)? {
            Ok(
                serde_json::from_value(report_json)
                    .expect("Can't deserialize report from database"),
            )
        } else if api_ctx
            .store
            .divergence_investigation_request_exists(&uuid)?
        {
            Ok(Some(DivergenceInvestigationReport {
                uuid,
                status: DivergenceInvestigationStatus::InProgress,
                bisection_runs: vec![],
                error: None,
            }))
        } else {
            Ok(None)
        }
    }

    async fn networks(&self, ctx: &Context<'_>) -> Result<Vec<Network>> {
        let api_ctx = ctx.data::<ApiSchemaContext>()?;

        let networks = api_ctx.store.networks()?;
        Ok(networks)
    }
}

pub struct MutationRoot;

#[Object]
impl MutationRoot {
    async fn launch_divergence_investigation(
        &self,
        ctx: &Context<'_>,
        req: DivergenceInvestigationRequest,
    ) -> Result<DivergenceInvestigationReport> {
        let api_ctx = ctx.data::<ApiSchemaContext>()?;
        let store = &api_ctx.store;

        let request_serialized = serde_json::to_value(req).unwrap();
        let uuid = store.create_divergence_investigation_request(request_serialized)?;

        let report = DivergenceInvestigationReport {
            uuid: uuid.clone(),
            status: DivergenceInvestigationStatus::Pending,
            bisection_runs: vec![],
            error: None,
        };

        Ok(report)
    }

    async fn set_deployment_name(
        &self,
        ctx: &Context<'_>,
        deployment_ipfs_cid: String,
        name: String,
    ) -> Result<Deployment> {
        let api_ctx = ctx.data::<ApiSchemaContext>()?;
        let store = &api_ctx.store;

        store.set_deployment_name(&deployment_ipfs_cid, &name)?;

        Ok(Deployment {
            id: deployment_ipfs_cid,
        })
    }

    async fn delete_network(&self, ctx: &Context<'_>, network: String) -> Result<String> {
        let api_ctx = ctx.data::<ApiSchemaContext>()?;
        let store = &api_ctx.store;

        store.delete_network(&network)?;

        Ok(network)
    }
}

pub struct ApiSchemaContext {
    pub store: Store,
}
