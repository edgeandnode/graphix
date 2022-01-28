#!/bin/bash

set -e
set -x

SCRIPT_DIR=$(dirname $0)

cd "$SCRIPT_DIR/../api-server/"
cargo run --bin sync-schemas
