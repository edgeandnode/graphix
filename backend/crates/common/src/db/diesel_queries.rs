use chrono::Utc;
use diesel::prelude::*;
use diesel::sql_types;
use tracing::info;

use super::models::WritablePoI;
use super::PoiLiveness;
use crate::api_types::BlockRangeInput;
use crate::db::models::{
    self, IndexerRow, NewIndexer, NewLivePoi, NewPoI, NewSgDeployment, SgDeployment,
};
use crate::db::schema::{self, live_pois};

pub(super) fn poi(conn: &mut PgConnection, poi: &str) -> anyhow::Result<Option<models::PoI>> {
    use schema::blocks;
    use schema::indexers;
    use schema::pois;
    use schema::sg_deployments;

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

    Ok(query.get_result::<models::PoI>(conn).optional()?)
}

pub(super) fn indexers(conn: &mut PgConnection) -> anyhow::Result<Vec<IndexerRow>> {
    use schema::indexers;

    Ok(indexers::table.load::<IndexerRow>(conn)?)
}

// This is a single SQL statement, a transaction is not necessary.
pub(super) fn pois(
    conn: &mut PgConnection,
    sg_deployments: Option<&[String]>,
    block_range: Option<BlockRangeInput>,
    limit: Option<u16>,
    live_only: bool,
) -> anyhow::Result<Vec<models::PoI>> {
    #![allow(non_snake_case)]
    use schema::blocks;
    use schema::indexers;
    use schema::pois;
    use schema::sg_deployments;

    let FALSE = diesel::dsl::sql::<sql_types::Bool>("false");
    let TRUE = diesel::dsl::sql::<sql_types::Bool>("true");

    let selection = (
        pois::id,
        pois::poi,
        pois::created_at,
        sg_deployments::all_columns,
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
        Some(sg_deployments) => sg_deployments::ipfs_cid.eq_any(sg_deployments).or(FALSE),
        None => sg_deployments::ipfs_cid.eq_any([]).or(TRUE),
    };

    let order_by = (blocks::number.desc(), schema::pois::created_at.desc());
    let limit = limit.unwrap_or(1000) as i64;

    match live_only {
        false => {
            let query = pois::table
                .inner_join(sg_deployments::table)
                .inner_join(indexers::table)
                .inner_join(blocks::table)
                .select(selection)
                .order_by(order_by)
                .filter(deployments_filter)
                .filter(blocks_filter)
                .limit(limit);
            return Ok(query.load::<models::PoI>(conn)?);
        }
        // This will additionally join with `live_pois` to filter out any PoIs that are not live.
        true => {
            let query = pois::table
                .inner_join(sg_deployments::table)
                .inner_join(indexers::table)
                .inner_join(blocks::table)
                .inner_join(live_pois::table)
                .select(selection)
                .order_by(order_by)
                .filter(deployments_filter)
                .filter(blocks_filter)
                .limit(limit);
            return Ok(query.load::<models::PoI>(conn)?);
        }
    }
}

// The caller must make sure that `conn` is within a transaction.
pub(super) fn write_pois(
    conn: &mut PgConnection,
    pois: &[impl WritablePoI],
    live: PoiLiveness,
) -> anyhow::Result<()> {
    use schema::blocks;
    use schema::pois;

    let len = pois.len();

    for poi in pois {
        let sg_deployment_id = get_or_insert_deployment(conn, poi.deployment_cid())?;

        let indexer_id = get_or_insert_indexer(conn, poi.indexer_id(), poi.indexer_address())?;

        let block_id = {
            // First, attempt to find the existing block by hash
            // TODO: also filter by network to be extra safe
            let existing_block: Option<models::Block> = blocks::table
                .filter(blocks::hash.eq(&poi.block().hash.unwrap().0.as_slice()))
                .get_result(conn)
                .optional()?;

            if let Some(existing_block) = existing_block {
                // If the block exists, use its id
                existing_block.id
            } else {
                // If the block doesn't exist, insert a new one and return its id
                let new_block = models::NewBlock {
                    number: poi.block().number as i64,
                    hash: poi.block().hash.unwrap().0.to_vec(),
                    network_id: 1, // Network assumed to be mainnet, see also: hardcoded-mainnet,
                };
                diesel::insert_into(blocks::table)
                    .values(&new_block)
                    .returning(blocks::id)
                    .get_result(conn)?
            }
        };

        let new_poi = NewPoI {
            sg_deployment_id,
            indexer_id,
            block_id,
            poi: poi.proof_of_indexing().to_vec(),
            created_at: Utc::now().naive_utc(),
        };

        let poi_id = diesel::insert_into(pois::table)
            .values(new_poi)
            .returning(pois::id)
            .on_conflict_do_nothing()
            .get_result::<i32>(conn)?;

        if live == PoiLiveness::Live {
            let value = NewLivePoi {
                poi_id,
                sg_deployment_id,
                indexer_id,
            };
            diesel::insert_into(live_pois::table)
                .values(&value)
                .on_conflict((live_pois::sg_deployment_id, live_pois::indexer_id))
                .do_update()
                .set(&value)
                .execute(conn)?;
        }
    }

    info!(%len, "Wrote POIs to database");
    Ok(())
}

fn get_or_insert_indexer(
    conn: &mut PgConnection,
    id: &str,
    address: Option<&[u8]>,
) -> Result<i32, anyhow::Error> {
    use schema::indexers;

    let existing_indexer: Option<IndexerRow> = indexers::table
        .filter(indexers::name.is_not_distinct_from(id))
        .filter(indexers::address.is_not_distinct_from(address))
        .get_result(conn)
        .optional()?;
    Ok(if let Some(existing_indexer) = existing_indexer {
        // If the indexer exists, use its id
        existing_indexer.id
    } else {
        // If the indexer doesn't exist, insert a new one and return its id
        let new_indexer = NewIndexer {
            address: address.map(ToOwned::to_owned),
            name: Some(id.to_string()),
        };
        diesel::insert_into(indexers::table)
            .values(&new_indexer)
            .returning(indexers::id)
            .get_result(conn)?
    })
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
