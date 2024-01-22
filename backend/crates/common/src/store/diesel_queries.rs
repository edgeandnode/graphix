use std::borrow::Cow;
use std::collections::BTreeMap;

use chrono::Utc;
use diesel::prelude::*;
use diesel::sql_types;
use tracing::info;

use super::models::WritablePoi;
use super::PoiLiveness;
use crate::graphql_api::types::BlockRangeInput;
use crate::indexer::{Indexer, IndexerId};
use crate::store::models::{
    self, Indexer as IndexerModel, NewIndexer, NewLivePoi, NewPoi, NewSgDeployment, SgDeployment,
};
use crate::store::schema::{self, live_pois};
use crate::types::{BlockPointer, IndexerVersion};

pub(super) fn poi(conn: &mut PgConnection, poi: &str) -> anyhow::Result<Option<models::Poi>> {
    use schema::{blocks, indexers, pois, sg_deployments};

    let poi = hex::decode(poi)?;

    let query = pois::table
        .inner_join(sg_deployments::table)
        .inner_join(indexers::table)
        .inner_join(blocks::table)
        .select((
            pois::id,
            pois::poi,
            pois::created_at,
            sg_deployments::all_columns,
            indexers::all_columns,
            blocks::all_columns,
        ))
        .filter(pois::poi.eq(poi));

    Ok(query.get_result::<models::Poi>(conn).optional()?)
}

// This is a single SQL statement, a transaction is not necessary.
pub(super) fn pois(
    conn: &mut PgConnection,
    indexer_id: Option<&str>,
    sg_deployments: Option<&[String]>,
    block_range: Option<BlockRangeInput>,
    limit: Option<u16>,
    live_only: bool,
) -> anyhow::Result<Vec<models::Poi>> {
    #![allow(non_snake_case)]
    use schema::{blocks, indexers, pois, sg_deployments as sgd};

    let FALSE = diesel::dsl::sql::<sql_types::Bool>("false");
    let TRUE = diesel::dsl::sql::<sql_types::Bool>("true");

    let selection = (
        pois::id,
        pois::poi,
        pois::created_at,
        sgd::all_columns,
        indexers::all_columns,
        blocks::all_columns,
    );

    let blocks_filter = blocks::number.between(
        block_range
            .as_ref()
            .and_then(|b| b.start)
            .map_or(0, |start| start as i64),
        block_range
            .as_ref()
            .and_then(|b| b.end)
            .map_or(i64::max_value(), |end| end as i64),
    );

    let deployments_filter = match sg_deployments {
        Some(sg_deployments) => sgd::ipfs_cid.eq_any(sg_deployments).or(FALSE.clone()),
        None => sgd::ipfs_cid.eq_any([]).or(TRUE.clone()),
    };

    let indexer_filter = match indexer_id {
        Some(indexer_id) => indexers::name.eq(indexer_id).or(FALSE),
        None => indexers::name.eq("").or(TRUE),
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
            Ok(query.load::<models::Poi>(conn)?)
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
            Ok(query.load::<models::Poi>(conn)?)
        }
    }
}

pub fn write_indexers(
    conn: &mut PgConnection,
    indexers: &[impl AsRef<dyn Indexer>],
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
        .execute(conn)?;

    Ok(())
}

pub fn set_deployment_name(
    conn: &mut PgConnection,
    sg_deployment_id: &str,
    name: &str,
) -> anyhow::Result<()> {
    use schema::{sg_deployments as sgd, sg_names};

    diesel::insert_into(sg_names::table)
        .values((
            sg_names::sg_deployment_id.eq(sgd::table
                .select(sgd::id)
                .filter(sgd::ipfs_cid.eq(sg_deployment_id))
                .single_value()
                .assume_not_null()),
            sg_names::name.eq(name),
        ))
        .on_conflict(sg_names::sg_deployment_id)
        .do_update()
        .set(sg_names::name.eq(name))
        .execute(conn)?;

    Ok(())
}

pub fn delete_network(conn: &mut PgConnection, network_name: &str) -> anyhow::Result<()> {
    use schema::networks;

    diesel::delete(networks::table.filter(networks::name.eq(network_name))).execute(conn)?;
    // The `ON DELETE CASCADE`s should take care of the rest of the cleanup.

    Ok(())
}

// The caller must make sure that `conn` is within a transaction.
pub(super) fn write_pois(
    conn: &mut PgConnection,
    pois: &[impl WritablePoi],
    live: PoiLiveness,
) -> anyhow::Result<()> {
    use diesel::insert_into;
    use schema::pois;

    let len = pois.len();

    // Group PoIs by deployment
    let mut grouped_pois: BTreeMap<_, Vec<_>> = BTreeMap::new();
    for poi in pois {
        grouped_pois
            .entry(poi.deployment_cid())
            .or_insert_with(Vec::new)
            .push(poi);
    }

    for (deployment, poi_group) in grouped_pois {
        let sg_deployment_id = get_or_insert_deployment(conn, deployment)?;
        let block_ptr = poi_group[0].block();

        // Make sure all PoIs have the same block ptr
        if !poi_group.iter().all(|poi| poi.block() == block_ptr) {
            return Err(anyhow::anyhow!(
                "All PoIs for a given deployment must have the same block"
            ));
        }

        let block_id = get_or_insert_block(conn, block_ptr)?;

        let new_pois: Vec<_> = poi_group
            .iter()
            .map(|poi| {
                let indexer_id =
                    get_indexer_id(conn, poi.indexer_id().name(), poi.indexer_id().address())?;

                Ok(NewPoi {
                    sg_deployment_id,
                    indexer_id,
                    block_id,
                    poi: poi.proof_of_indexing().to_vec(),
                    created_at: Utc::now().naive_utc(),
                })
            })
            .collect::<anyhow::Result<_>>()?;

        // Insert all PoIs for this deployment
        let id_and_indexer: Vec<(i32, i32)> = insert_into(pois::table)
            .values(&new_pois)
            .returning((pois::id, pois::indexer_id))
            .get_results(conn)?;

        if live == PoiLiveness::Live {
            // Clear any live pois for this deployment
            diesel::delete(
                live_pois::table.filter(live_pois::sg_deployment_id.eq(sg_deployment_id)),
            )
            .execute(conn)?;

            for (poi_id, indexer_id) in id_and_indexer {
                let value = NewLivePoi {
                    poi_id,
                    sg_deployment_id,
                    indexer_id,
                };
                diesel::insert_into(live_pois::table)
                    .values(&value)
                    .execute(conn)?;
            }
        }
    }

    info!(%len, "Wrote POIs to database");
    Ok(())
}

fn get_or_insert_block(conn: &mut PgConnection, block: BlockPointer) -> anyhow::Result<i64> {
    use schema::blocks;

    // First, attempt to find the existing block by hash
    // TODO: also filter by network to be extra safe
    let existing_block: Option<models::Block> = blocks::table
        .filter(blocks::hash.eq(&block.hash.unwrap().0.as_slice()))
        .get_result(conn)
        .optional()?;

    if let Some(existing_block) = existing_block {
        // If the block exists, return its id
        Ok(existing_block.id)
    } else {
        // If the block doesn't exist, insert a new one and return its id
        let new_block = models::NewBlock {
            number: block.number as i64,
            hash: block.hash.unwrap().0.to_vec(),
            network_id: 1, // Network assumed to be mainnet, see also: hardcoded-mainnet
        };
        let block_id = diesel::insert_into(blocks::table)
            .values(&new_block)
            .returning(blocks::id)
            .get_result(conn)?;
        Ok(block_id)
    }
}

pub fn write_graph_node_version(
    conn: &mut PgConnection,
    indexer: &dyn Indexer,
    version: anyhow::Result<IndexerVersion>,
) -> anyhow::Result<()> {
    use schema::indexer_versions;

    let indexer_id = get_indexer_id(conn, indexer.name(), indexer.address())?;

    let new_version = match version {
        Ok(v) => models::NewIndexerVersion {
            indexer_id,
            error: None,
            version_string: Some(v.version),
            version_commit: Some(v.commit),
        },
        Err(err) => models::NewIndexerVersion {
            indexer_id,
            error: Some(err.to_string()),
            version_string: None,
            version_commit: None,
        },
    };

    diesel::insert_into(indexer_versions::table)
        .values(&new_version)
        .execute(conn)?;

    Ok(())
}

fn get_indexer_id(
    conn: &mut PgConnection,
    name: Option<Cow<String>>,
    address: &[u8],
) -> anyhow::Result<i32> {
    use schema::indexers;

    let existing_indexer: Option<IndexerModel> = indexers::table
        .filter(indexers::name.is_not_distinct_from(&name))
        .filter(indexers::address.is_not_distinct_from(address))
        .get_result(conn)
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

fn get_or_insert_deployment(
    conn: &mut PgConnection,
    deployment_cid: &str,
) -> Result<i32, anyhow::Error> {
    use schema::sg_deployments;

    let existing_sg_deployment: Option<SgDeployment> = sg_deployments::table
        .filter(sg_deployments::ipfs_cid.eq(&deployment_cid))
        .get_result(conn)
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
                .get_result(conn)?
        },
    )
}
