use diesel::{allow_tables_to_appear_in_same_query, joinable, table};

table! {
    block_cache_entries (id) {
        id -> Int8,
        indexer_id -> Int4,
        block_id -> Int8,
        block_data -> Jsonb,
        created_at -> Timestamp,
    }
}

table! {
    blocks (id) {
        id -> Int4,
        network_id -> Int4,
        number -> Int8,
        hash -> Bytea,
    }
}

table! {
    entity_changes_in_block (id) {
        id -> Int8,
        indexer_id -> Int4,
        block_id -> Int8,
        entity_change_data -> Jsonb,
        created_at -> Timestamp,
    }
}

table! {
    eth_call_cache_entries (id) {
        id -> Int8,
        indexer_id -> Int4,
        block_id -> Int8,
        eth_call_data -> Jsonb,
        eth_call_result -> Jsonb,
        created_at -> Timestamp,
    }
}

table! {
    indexers (id) {
        id -> Int4,
        name -> Nullable<Text>,
        address -> Nullable<Bytea>,
        created_at -> Timestamp,
    }
}

table! {
    live_pois (id) {
        id -> Int4,
        poi_id -> Int4,
    }
}

table! {
    networks (id) {
        id -> Int4,
        name -> Text,
        created_at -> Timestamp,
    }
}

table! {
    poi_divergence_bisect_reports (id) {
        id -> Int4,
        poi1_id -> Int4,
        poi2_id -> Int4,
        divergence_block_id -> Int4,
        created_at -> Timestamp,
    }
}

table! {
    pois (id) {
        id -> Int4,
        poi -> Bytea,
        sg_deployment_id -> Int4,
        indexer_id -> Int4,
        block_id -> Int4,
        created_at -> Timestamp,
    }
}

table! {
    sg_deployments (id) {
        id -> Int4,
        cid -> Text,
        network -> Int4,
        created_at -> Timestamp,
    }
}

table! {
    sg_names (id) {
        id -> Int4,
        sg_deployment_id -> Int4,
        name -> Text,
        created_at -> Timestamp,
    }
}

// joinable!(block_cache_entries -> blocks (block_id));
// joinable!(entity_changes_in_block -> blocks (block_id));
// joinable!(eth_call_cache_entries -> blocks (block_id));
joinable!(block_cache_entries -> indexers (indexer_id));
joinable!(blocks -> networks (network_id));
joinable!(entity_changes_in_block -> indexers (indexer_id));
joinable!(eth_call_cache_entries -> indexers (indexer_id));
joinable!(live_pois -> pois (poi_id));
joinable!(poi_divergence_bisect_reports -> blocks (divergence_block_id));
joinable!(pois -> blocks (block_id));
joinable!(pois -> indexers (indexer_id));
joinable!(pois -> sg_deployments (sg_deployment_id));
joinable!(sg_deployments -> networks (network));
joinable!(sg_names -> sg_deployments (sg_deployment_id));

allow_tables_to_appear_in_same_query!(
    block_cache_entries,
    blocks,
    entity_changes_in_block,
    eth_call_cache_entries,
    indexers,
    live_pois,
    networks,
    poi_divergence_bisect_reports,
    pois,
    sg_deployments,
    sg_names,
);
