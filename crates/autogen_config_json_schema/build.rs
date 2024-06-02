use std::env;
use std::fs::File;
use std::io::*;

use graphix_lib::config::Config;
use schemars::schema_for;

fn main() -> std::io::Result<()> {
    let out_path = env::current_dir()?.join("schema.json");
    let mut f = File::create(&out_path)?;

    let schema = schema_for!(Config);
    f.write_all(serde_json::to_string_pretty(&schema).unwrap().as_bytes())?;

    Ok(())
}
