//! Provides the diesel queries, callers should handle connection pooling and
//! transactions.

use std::borrow::Cow;
use std::collections::BTreeMap;

use chrono::Utc;
use diesel::prelude::*;
use diesel::sql_types;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use graphix_common_types::IpfsCid;
use graphix_common_types::{inputs, IndexerAddress};
use graphix_indexer_client::{BlockPointer, IndexerClient, IndexerId, WritablePoi};
use tracing::info;

use super::PoiLiveness;
use crate::models::{
    self, Indexer as IndexerModel, NewIndexer, NewLivePoi, NewPoi, NewSgDeployment, SgDeployment,
};
use crate::schema::{self, live_pois, sg_names};

// This is a single SQL statement, a transaction is not necessary.
pub(super) async fn pois(
    conn: &mut AsyncPgConnection,
    indexer_address: Option<&IndexerAddress>,
    sg_deployments: Option<&[IpfsCid]>,
    block_range: Option<inputs::BlockRange>,
    limit: Option<u16>,
    live_only: bool,
) -> anyhow::Result<Vec<models::Poi>> {
    #![allow(non_snake_case)]
    use schema::{blocks, indexers, pois, sg_deployments as sgd};

    let FALSE = diesel::dsl::sql::<sql_types::Bool>("false");
    let TRUE = diesel::dsl::sql::<sql_types::Bool>("true");

    let selection = pois::all_columns;

    // TODO: optimize this into a single comparison in the absence of lower or
    // upper bounds.
    let blocks_filter = blocks::number.between(
        block_range
            .as_ref()
            .and_then(|b| b.start)
            .map(|start| start.try_into())
            .transpose()?
            .unwrap_or(0),
        block_range
            .as_ref()
            .and_then(|b| b.start)
            .map(|start| start.try_into())
            .transpose()?
            .unwrap_or(i64::MAX),
    );

    let deployments_filter = match sg_deployments {
        Some(sg_deployments) => sgd::ipfs_cid.eq_any(sg_deployments).or(FALSE.clone()),
        None => sgd::ipfs_cid.eq_any([]).or(TRUE.clone()),
    };

    let default_indexer_address = IndexerAddress::default();
    let indexer_filter = match indexer_address {
        // Ugly hacks to have the match arms' types match.
        Some(addr) => indexers::address.eq(addr).or(FALSE),
        None => indexers::address.eq(&default_indexer_address).or(TRUE),
    };

    let order_by = (blocks::number.desc(), schema::pois::created_at.desc());
    let limit = limit.map(|l| l as i64).unwrap_or(i64::MAX);

    match live_only {
        false => {
            let query = pois::table
                .inner_join(sgd::table)
                .inner_join(indexers::table)
                .inner_join(blocks::table)
                .select(selection)
                .order_by(order_by)
                .filter(deployments_filter)
                .filter(blocks_filter)
                .filter(indexer_filter)
                .limit(limit);
            Ok(query.load::<models::Poi>(conn).await?)
        }
        // This will additionally join with `live_pois` to filter out any Pois that are not live.
        true => {
            let query = pois::table
                .inner_join(sgd::table)
                .inner_join(indexers::table)
                .inner_join(blocks::table)
                .inner_join(live_pois::table)
                .select(selection)
                .order_by(order_by)
                .filter(deployments_filter)
                .filter(blocks_filter)
                .filter(indexer_filter)
                .limit(limit);
            Ok(query.load::<models::Poi>(conn).await?)
        }
    }
}

pub async fn write_indexers(
    conn: &mut AsyncPgConnection,
    indexers: &[impl AsRef<dyn IndexerClient>],
) -> anyhow::Result<()> {
    use schema::indexers;

    let insertable_indexers = indexers
        .iter()
        .map(|indexer| {
            let indexer = indexer.as_ref();
            NewIndexer {
                address: indexer.address().to_owned(),
                name: indexer.name().map(|s| s.to_string()),
            }
        })
        .collect::<Vec<_>>();

    diesel::insert_into(indexers::table)
        .values(insertable_indexers)
        .on_conflict_do_nothing()
        .execute(conn)
        .await?;

    Ok(())
}

// The caller must make sure that `conn` is within a transaction.
pub(super) async fn write_pois<W>(
    conn: &mut AsyncPgConnection,
    pois: Vec<W>,
    live: PoiLiveness,
) -> anyhow::Result<()>
where
    W: WritablePoi + Send + Sync,
    W::IndexerId: Send + Sync,
{
    use diesel::insert_into;
    use schema::pois;

    let len = pois.len();

    // Group PoIs by deployment
    let mut grouped_pois: BTreeMap<_, Vec<_>> = BTreeMap::new();
    for poi in pois.iter() {
        grouped_pois
            .entry(poi.deployment_cid())
            .or_insert_with(Vec::new)
            .push(poi);
    }

    for (deployment, poi_group) in grouped_pois {
        let sg_deployment_id = get_or_insert_deployment(conn, deployment).await?;
        let block_ptr = poi_group[0].block();

        // Make sure all PoIs have the same block ptr
        if !poi_group.iter().all(|poi| poi.block() == block_ptr) {
            return Err(anyhow::anyhow!(
                "All PoIs for a given deployment must have the same block"
            ));
        }

        let block_id = get_or_insert_block(conn, block_ptr).await?;

        let mut new_pois = vec![];

        for poi in poi_group.iter() {
            let indexer_id =
                get_indexer_id(conn, poi.indexer_id().name(), &poi.indexer_id().address()).await?;

            new_pois.push(NewPoi {
                sg_deployment_id,
                indexer_id,
                block_id,
                poi: *poi.proof_of_indexing(),
                created_at: Utc::now().naive_utc(),
            });
        }

        // Insert all PoIs for this deployment
        let id_and_indexer: Vec<(i32, i32)> = insert_into(pois::table)
            .values(&new_pois)
            .returning((pois::id, pois::indexer_id))
            .get_results(conn)
            .await?;

        if live == PoiLiveness::Live {
            // Clear any live pois for this deployment
            diesel::delete(
                live_pois::table.filter(live_pois::sg_deployment_id.eq(sg_deployment_id)),
            )
            .execute(conn)
            .await?;

            for (poi_id, indexer_id) in id_and_indexer {
                let value = NewLivePoi {
                    poi_id,
                    sg_deployment_id,
                    indexer_id,
                };
                diesel::insert_into(live_pois::table)
                    .values(&value)
                    .execute(conn)
                    .await?;
            }
        }
    }

    info!(%len, "Wrote POIs to database");
    Ok(())
}

async fn get_or_insert_block(
    conn: &mut AsyncPgConnection,
    block: &BlockPointer,
) -> anyhow::Result<i64> {
    use schema::blocks;

    // First, attempt to find the existing block by hash
    // TODO: also filter by network to be extra safe
    let existing_block: Option<models::Block> = blocks::table
        .filter(blocks::hash.eq(&block.hash.as_ref().unwrap().0.as_slice()))
        .get_result(conn)
        .await
        .optional()?;

    if let Some(existing_block) = existing_block {
        // If the block exists, return its id
        Ok(existing_block.id)
    } else {
        // If the block doesn't exist, insert a new one and return its id
        let new_block = models::NewBlock {
            number: block.number as i64,
            hash: block.hash.clone().unwrap(),
            network_id: 1, // FIXME: network assumed to be mainnet, see also: hardcoded-mainnet
        };
        let block_id = diesel::insert_into(blocks::table)
            .values(&new_block)
            .returning(blocks::id)
            .get_result(conn)
            .await?;
        Ok(block_id)
    }
}

pub async fn get_indexer_id<'a>(
    conn: &mut AsyncPgConnection,
    name: Option<Cow<'a, str>>,
    address: &IndexerAddress,
) -> anyhow::Result<i32> {
    use schema::indexers;

    let existing_indexer: Option<IndexerModel> = indexers::table
        .filter(indexers::name.is_not_distinct_from(&name))
        .filter(indexers::address.is_not_distinct_from(address))
        .get_result(conn)
        .await
        .optional()?;

    if let Some(i) = existing_indexer {
        Ok(i.id)
    } else {
        Err(anyhow::anyhow!(
            "Indexer with name {:?} and/or address {:?} not found",
            &name,
            address
        ))
    }
}

async fn get_or_insert_deployment(
    conn: &mut AsyncPgConnection,
    deployment_cid: &str,
) -> Result<i32, anyhow::Error> {
    use schema::sg_deployments;

    let existing_sg_deployment: Option<SgDeployment> = sg_deployments::table
        .left_join(sg_names::table)
        .select((
            sg_deployments::id,
            sg_deployments::ipfs_cid,
            sg_names::name.nullable(),
            sg_deployments::network,
            sg_deployments::created_at,
        ))
        .filter(sg_deployments::ipfs_cid.eq(&deployment_cid))
        .get_result(conn)
        .await
        .optional()?;
    Ok(
        if let Some(existing_sg_deployment) = existing_sg_deployment {
            // If the sg_deployment exists, use its id
            existing_sg_deployment.id
        } else {
            // If the sg_deployment doesn't exist, insert a new one and return its id
            let new_sg_deployment = NewSgDeployment {
                ipfs_cid: deployment_cid.to_owned(),
                network: 1, // Network assumed to be mainnet, see also: hardcoded-mainnet
                created_at: Utc::now().naive_utc(),
            };
            diesel::insert_into(sg_deployments::table)
                .values(&new_sg_deployment)
                .returning(sg_deployments::id)
                .get_result(conn)
                .await?
        },
    )
}
