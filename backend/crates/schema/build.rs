use std::env;
use std::fs::File;
use std::io::*;

use async_graphql::{EmptySubscription, Schema};
use graphix_common::graphql_api::{MutationRoot, QueryRoot};

fn main() -> std::io::Result<()> {
    // We're only interested in re-generating the API schema if build
    // dependencies change or the build script itself does.
    // See <https://doc.rust-lang.org/cargo/reference/build-scripts.html#change-detection>.
    println!("cargo:rerun-if-changed=build.rs");

    let api_schema = Schema::build(QueryRoot, MutationRoot, EmptySubscription).finish();
    let path = env::current_dir()?.join("graphql/api_schema.graphql");

    let mut f = File::create(&path)?;

    f.write_all(b"# AUTOGENERATED. DO NOT MODIFY. ALL CHANGES WILL BE LOST.\n\n")?;
    f.write_all(api_schema.sdl().as_bytes())?;

    println!("Updated: {}", path.display());
    Ok(())
}
