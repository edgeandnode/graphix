// @generated automatically by Diesel CLI.

diesel::table! {
    block_cache_entries (id) {
        id -> Int8,
        indexer_id -> Int4,
        block_id -> Int8,
        block_data -> Jsonb,
        created_at -> Timestamp,
    }
}

diesel::table! {
    blocks (id) {
        id -> Int8,
        network_id -> Int4,
        number -> Int8,
        hash -> Bytea,
    }
}

diesel::table! {
    divergence_investigation_requests (uuid) {
        uuid -> Text,
        request_contents -> Jsonb,
        created_at -> Timestamp,
    }
}

diesel::table! {
    entity_changes_in_block (id) {
        id -> Int8,
        indexer_id -> Int4,
        block_id -> Int8,
        entity_change_data -> Jsonb,
        created_at -> Timestamp,
    }
}

diesel::table! {
    eth_call_cache_entries (id) {
        id -> Int8,
        indexer_id -> Int4,
        block_id -> Int8,
        eth_call_data -> Jsonb,
        eth_call_result -> Jsonb,
        created_at -> Timestamp,
    }
}

diesel::table! {
    indexers (id) {
        id -> Int4,
        name -> Nullable<Text>,
        address -> Nullable<Bytea>,
        created_at -> Timestamp,
    }
}

diesel::table! {
    live_pois (id) {
        id -> Int4,
        sg_deployment_id -> Int4,
        indexer_id -> Int4,
        poi_id -> Int4,
    }
}

diesel::table! {
    networks (id) {
        id -> Int4,
        name -> Text,
        created_at -> Timestamp,
        caip2 -> Nullable<Text>,
    }
}

diesel::table! {
    poi_divergence_bisect_reports (id) {
        id -> Text,
        poi1_id -> Int4,
        poi2_id -> Int4,
        divergence_block_id -> Nullable<Int8>,
        created_at -> Timestamp,
    }
}

diesel::table! {
    pois (id) {
        id -> Int4,
        poi -> Bytea,
        sg_deployment_id -> Int4,
        indexer_id -> Int4,
        block_id -> Int8,
        created_at -> Timestamp,
    }
}

diesel::table! {
    sg_deployments (id) {
        id -> Int4,
        ipfs_cid -> Text,
        network -> Int4,
        created_at -> Timestamp,
    }
}

diesel::table! {
    sg_names (id) {
        id -> Int4,
        sg_deployment_id -> Int4,
        name -> Text,
        created_at -> Timestamp,
    }
}

diesel::joinable!(block_cache_entries -> blocks (block_id));
diesel::joinable!(block_cache_entries -> indexers (indexer_id));
diesel::joinable!(blocks -> networks (network_id));
diesel::joinable!(entity_changes_in_block -> blocks (block_id));
diesel::joinable!(entity_changes_in_block -> indexers (indexer_id));
diesel::joinable!(eth_call_cache_entries -> blocks (block_id));
diesel::joinable!(eth_call_cache_entries -> indexers (indexer_id));
diesel::joinable!(live_pois -> indexers (indexer_id));
diesel::joinable!(live_pois -> pois (poi_id));
diesel::joinable!(live_pois -> sg_deployments (sg_deployment_id));
diesel::joinable!(poi_divergence_bisect_reports -> blocks (divergence_block_id));
diesel::joinable!(pois -> blocks (block_id));
diesel::joinable!(pois -> indexers (indexer_id));
diesel::joinable!(pois -> sg_deployments (sg_deployment_id));
diesel::joinable!(sg_deployments -> networks (network));
diesel::joinable!(sg_names -> sg_deployments (sg_deployment_id));

diesel::allow_tables_to_appear_in_same_query!(
    block_cache_entries,
    blocks,
    divergence_investigation_requests,
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
