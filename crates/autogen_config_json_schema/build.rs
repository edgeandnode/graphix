use std::env;
use std::fs::File;
use std::io::*;

use graphix_lib::config::Config;
use schemars::schema_for;

fn main() -> std::io::Result<()> {
    // We're only interested in re-generating the API schema if build
    // dependencies change or the build script itself does.
    // See <https://doc.rust-lang.org/cargo/reference/build-scripts.html#change-detection>.
    println!("cargo:rerun-if-changed=build.rs");

    let out_path = env::current_dir()?.join("schema.json");
    let mut f = File::create(&out_path)?;

    let schema = schema_for!(Config);
    f.write_all(serde_json::to_string_pretty(&schema).unwrap().as_bytes())?;

    Ok(())
}
