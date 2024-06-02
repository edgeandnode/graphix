// @generated automatically by Diesel CLI.

diesel::table! {
    blocks (id) {
        id -> Int8,
        network_id -> Int4,
        number -> Int8,
        hash -> Bytea,
    }
}

diesel::table! {
    divergence_investigation_reports (uuid) {
        uuid -> Uuid,
        report -> Jsonb,
        created_at -> Timestamp,
    }
}

diesel::table! {
    failed_queries (id) {
        id -> Int4,
        indexer_id -> Int4,
        query_name -> Text,
        raw_query -> Text,
        response -> Text,
        request_timestamp -> Timestamp,
    }
}

diesel::table! {
    graph_node_collected_versions (id) {
        id -> Int4,
        version_string -> Nullable<Text>,
        version_commit -> Nullable<Text>,
        error_response -> Nullable<Text>,
        collected_at -> Timestamp,
    }
}

diesel::table! {
    graphix_api_tokens (public_prefix) {
        public_prefix -> Text,
        sha256_api_key_hash -> Bytea,
        notes -> Nullable<Text>,
        permission_level -> Text,
    }
}

diesel::table! {
    indexer_network_subgraph_metadata (id) {
        id -> Int4,
        geohash -> Nullable<Text>,
        indexer_url -> Nullable<Text>,
        staked_tokens -> Numeric,
        allocated_tokens -> Numeric,
        locked_tokens -> Numeric,
        query_fees_collected -> Numeric,
        query_fee_rebates -> Numeric,
        rewards_earned -> Numeric,
        indexer_indexing_rewards -> Numeric,
        delegator_indexing_rewards -> Numeric,
        last_updated_at -> Timestamp,
    }
}

diesel::table! {
    indexers (id) {
        id -> Int4,
        address -> Bytea,
        name -> Nullable<Text>,
        graph_node_version -> Nullable<Int4>,
        network_subgraph_metadata -> Nullable<Int4>,
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
        caip2 -> Nullable<Text>,
        created_at -> Timestamp,
    }
}

diesel::table! {
    pending_divergence_investigation_requests (uuid) {
        uuid -> Uuid,
        request -> Jsonb,
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
    sg_deployment_api_versions (id) {
        id -> Int4,
        sg_deployment_id -> Int4,
        api_versions -> Nullable<Array<Nullable<Text>>>,
        error -> Nullable<Text>,
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

diesel::joinable!(blocks -> networks (network_id));
diesel::joinable!(failed_queries -> indexers (indexer_id));
diesel::joinable!(indexers -> graph_node_collected_versions (graph_node_version));
diesel::joinable!(indexers -> indexer_network_subgraph_metadata (network_subgraph_metadata));
diesel::joinable!(live_pois -> indexers (indexer_id));
diesel::joinable!(live_pois -> pois (poi_id));
diesel::joinable!(live_pois -> sg_deployments (sg_deployment_id));
diesel::joinable!(pois -> blocks (block_id));
diesel::joinable!(pois -> indexers (indexer_id));
diesel::joinable!(pois -> sg_deployments (sg_deployment_id));
diesel::joinable!(sg_deployment_api_versions -> sg_deployments (sg_deployment_id));
diesel::joinable!(sg_deployments -> networks (network));
diesel::joinable!(sg_names -> sg_deployments (sg_deployment_id));

diesel::allow_tables_to_appear_in_same_query!(
    blocks,
    divergence_investigation_reports,
    failed_queries,
    graph_node_collected_versions,
    graphix_api_tokens,
    indexer_network_subgraph_metadata,
    indexers,
    live_pois,
    networks,
    pending_divergence_investigation_requests,
    pois,
    sg_deployment_api_versions,
    sg_deployments,
    sg_names,
);
