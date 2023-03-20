const INDEXER_SCHEMA_URL: &str = "https://raw.githubusercontent.com/graphprotocol/graph-node/master/server/index-node/src/schema.graphql";

fn main() {
    println!("cargo:rerun-if-changed=graphql/touch-this-file-to-refetch-schema");
    let indexer_schema = reqwest::blocking::get(INDEXER_SCHEMA_URL)
        .expect("Failed to fetch indexer schema")
        .text()
        .unwrap();

    std::fs::write("graphql/indexer/schema.gql", indexer_schema).unwrap();
}
