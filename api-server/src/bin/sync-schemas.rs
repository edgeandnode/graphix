use std::env;
use std::fs::File;
use std::io::*;

use async_graphql::{EmptyMutation, EmptySubscription, Schema};

use graph_ixi_common::api_schema::QueryRoot;

fn main() -> std::io::Result<()> {
    let schema = Schema::build(QueryRoot, EmptyMutation, EmptySubscription).finish();
    let path = env::current_dir()
        .expect("Unable to identify working directory")
        .as_path()
        .join("../frontend/graphql/api_schema.graphql");

    let display_path = path
        .canonicalize()
        .expect("Failed to canonicalize frontend API schema path");

    match File::create(&path) {
        Ok(mut file) => {
            file.write_all(schema.sdl().as_bytes())?;
            println!("Updated: {}", display_path.display());
            Ok(())
        }
        Err(error) => {
            println!("Failed to open file: {}: {}", display_path.display(), error);
            Ok(())
        }
    }
}
