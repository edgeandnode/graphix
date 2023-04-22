use chrono::Utc;
use diesel::prelude::*;
use tracing::info;

use super::models::WritablePoI;
use super::PoiLiveness;
use crate::api_types::BlockRange;
use crate::db::models::{
    self, IndexerRow, NewIndexer, NewLivePoi, NewPoI, NewSgDeployment, SgDeployment,
};
use crate::db::schema;

// This is a single SQL statement, a transaction is not necessary.
pub(super) fn pois(
    conn: &mut PgConnection,
    sg_deployments: &[String],
    block_range: Option<BlockRange>,
    limit: Option<u16>,
) -> anyhow::Result<Vec<models::PoI>> {
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
        .order_by(blocks::number.desc())
        .order_by(schema::pois::created_at.desc())
        .filter(sg_deployments::cid.eq_any(sg_deployments))
        .filter(blocks::number.between(
            block_range.as_ref().map_or(0, |range| range.start as i64),
            block_range.map_or(i64::max_value(), |range| range.end as i64),
        ))
        .limit(limit.unwrap_or(1000) as i64);

    Ok(query.load::<models::PoI>(conn)?)
}

// The caller must make sure that `conn` is within a transaction.
pub(super) fn write_pois(
    conn: &mut PgConnection,
    pois: &[impl WritablePoI],
    live: PoiLiveness,
) -> anyhow::Result<()> {
    use schema::blocks;
    use schema::indexers;
    use schema::live_pois;
    use schema::pois;
    use schema::sg_deployments;

    let len = pois.len();

    for poi in pois {
        let sg_deployment_id = {
            // First, attempt to find the existing sg_deployment by the deployment field
            let existing_sg_deployment: Option<SgDeployment> = sg_deployments::table
                .filter(sg_deployments::cid.eq(poi.deployment_cid()))
                .get_result(conn)
                .optional()?;

            if let Some(existing_sg_deployment) = existing_sg_deployment {
                // If the sg_deployment exists, use its id
                existing_sg_deployment.id
            } else {
                // If the sg_deployment doesn't exist, insert a new one and return its id
                let new_sg_deployment = NewSgDeployment {
                    cid: poi.deployment_cid().to_string(),
                    network: 1, // Network assumed to be mainnet, see also: hardcoded-mainnet
                    created_at: Utc::now().naive_utc(),
                };
                diesel::insert_into(sg_deployments::table)
                    .values(&new_sg_deployment)
                    .returning(sg_deployments::id)
                    .get_result(conn)?
            }
        };

        let indexer_id = {
            // First, attempt to find the existing indexer by the address field
            let existing_indexer: Option<IndexerRow> = indexers::table
                .filter(indexers::name.eq(poi.indexer_id()))
                .filter(indexers::address.eq(poi.indexer_address()))
                .get_result(conn)
                .optional()?;

            if let Some(existing_indexer) = existing_indexer {
                // If the indexer exists, use its id
                existing_indexer.id
            } else {
                // If the indexer doesn't exist, insert a new one and return its id
                let new_indexer = NewIndexer {
                    address: poi.indexer_address().map(ToOwned::to_owned),
                    name: Some(poi.indexer_id().to_string()),
                };
                diesel::insert_into(indexers::table)
                    .values(&new_indexer)
                    .returning(indexers::id)
                    .get_result(conn)?
            }
        };

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
            diesel::insert_into(live_pois::table)
                .values(NewLivePoi { poi_id })
                .on_conflict_do_nothing()
                .execute(conn)?;
        }
    }

    info!(%len, "Wrote POIs to database");
    Ok(())
}
