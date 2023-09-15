use chrono::Utc;
use diesel::prelude::*;
use diesel::sql_types;
use tracing::info;

use super::models::WritablePoI;
use super::PoiLiveness;
use crate::api_types::BlockRangeInput;
use crate::api_types::DivergenceInvestigationRequest;
use crate::api_types::DivergenceInvestigationRequestWithUuid;
use crate::api_types::NewDivergenceInvestigationRequest;
use crate::store::models::{
    self, IndexerRow, NewIndexer, NewLivePoi, NewPoI, NewSgDeployment, SgDeployment,
};
use crate::store::schema::{self, live_pois};

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

fn poi_by_id(conn: &mut PgConnection, poi_id: i32) -> anyhow::Result<Option<models::PoI>> {
    use schema::blocks;
    use schema::indexers;
    use schema::pois;
    use schema::sg_deployments;

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
        .filter(pois::id.eq(&poi_id));

    Ok(query.get_result::<models::PoI>(conn).optional()?)
}

pub(super) fn indexers(conn: &mut PgConnection) -> anyhow::Result<Vec<IndexerRow>> {
    use schema::indexers;

    Ok(indexers::table.load::<IndexerRow>(conn)?)
}

// This is a single SQL statement, a transaction is not necessary.
pub(super) fn pois(
    conn: &mut PgConnection,
    indexer_id: Option<&str>,
    sg_deployments: Option<&[String]>,
    block_range: Option<BlockRangeInput>,
    limit: Option<u16>,
    live_only: bool,
) -> anyhow::Result<Vec<models::PoI>> {
    #![allow(non_snake_case)]
    use schema::blocks;
    use schema::indexers;
    use schema::pois;
    use schema::sg_deployments as sgd;

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
    let limit = limit.map(|l| l as i64).unwrap_or(i64::MAX) as i64;

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
            return Ok(query.load::<models::PoI>(conn)?);
        }
        // This will additionally join with `live_pois` to filter out any PoIs that are not live.
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
            return Ok(query.load::<models::PoI>(conn)?);
        }
    }
}

pub fn get_cross_check_report(
    conn: &mut PgConnection,
    req_id: &str,
) -> anyhow::Result<serde_json::Value> {
    use schema::poi_divergence_bisect_reports::dsl::*;

    let row = poi_divergence_bisect_reports
        .filter(id.eq(&req_id))
        .get_result::<models::PoiDivergenceBisectReport>(conn)?;

    let poi1 = poi_by_id(conn, row.poi1_id)?.unwrap();
    let poi2 = poi_by_id(conn, row.poi2_id)?.unwrap();

    Ok(serde_json::json! ({
        "poi1": poi1,
        "poi2": poi2,
        "report": row,
    }))
}

pub fn set_deployment_name(
    conn: &mut PgConnection,
    sg_deployment_id: &str,
    name: &str,
) -> anyhow::Result<()> {
    use schema::sg_deployments as sgd;
    use schema::sg_names;

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

pub fn get_first_divergence_investigation_request(
    conn: &mut PgConnection,
) -> anyhow::Result<(String, NewDivergenceInvestigationRequest)> {
    use schema::divergence_investigation_requests as requests;

    let (uuid_string, jsonb) = requests::table
        .select((requests::uuid, requests::request_contents))
        .first::<(String, serde_json::Value)>(conn)?;

    let request_contents = serde_json::from_value(jsonb)?;

    Ok((uuid_string, request_contents))
}

pub fn create_divergence_investigation_reqest(
    conn: &mut PgConnection,
    uuid_str: String,
    request_contents: serde_json::Value,
) -> anyhow::Result<()> {
    use schema::divergence_investigation_requests as requests;

    diesel::insert_into(requests::table)
        .values((
            requests::uuid.eq(uuid_str),
            requests::request_contents.eq(request_contents),
        ))
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
