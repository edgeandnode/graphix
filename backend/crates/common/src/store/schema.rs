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
        uuid -> Text,
        report -> Jsonb,
        created_at -> Timestamp,
    }
}

diesel::table! {
    indexer_versions (id) {
        id -> Int4,
        indexer_id -> Int4,
        error -> Nullable<Text>,
        version_string -> Nullable<Text>,
        version_commit -> Nullable<Text>,
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
    pending_divergence_investigation_requests (uuid) {
        uuid -> Text,
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
diesel::joinable!(indexer_versions -> indexers (indexer_id));
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
    indexer_versions,
    indexers,
    live_pois,
    networks,
    pending_divergence_investigation_requests,
    pois,
    sg_deployment_api_versions,
    sg_deployments,
    sg_names,
);
