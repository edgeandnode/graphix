# Graphix

Graphix is a GraphQL API for monitoring and cross-checking PoIs on the The Graph network. It is designed to monitor, detect, and help debug inconsistencies in indexing results through comparison of Proofs of Indexing (PoIs).

All data collected by Graphix is accessible through a public GraphQL API (schema [here](crates/autogen_graphql_schema/api_schema.graphql)), but you can also use some of our pre-built Grafana dashboards as a visualization aid.

## Local development quickstart

```sh
# Spin up all dependencies in a separate terminal window.
# Grafana will be available at <http://localhost:3000>.
$ cd compose && docker compose -f dependencies.yml up

$ cargo build
$ ./target/debug/graphix --help

A GraphQL API for monitoring and cross-checking PoIs on the The Graph network.

Usage: graphix [OPTIONS] --database-url <DATABASE_URL>

Options:
      --database-url <DATABASE_URL>
          The URL of the PostgreSQL database to use. Can also be set via env. var.. [env: GRAPHIX_DB_URL=]
      --base-config <BASE_CONFIG>
          Path to the initial Graphix YAML configuration file
      --port <PORT>
          The port on which the GraphQL API server should listen [default: 8000]
      --prometheus-port <PROMETHEUS_PORT>
          The port on which the Prometheus exporter should listen [default: 9184]
  -h, --help
          Print help
  -V, --version
          Print version

$ export GRAPHIX_DB_URL=postgresql://postgres:foobar@localhost:5433/graphix
$ ./target/debug/graphix --base-config configs/readonly.graphix.yml
```

You can play around with some sample GraphQL queries using the [Bruno](https://www.usebruno.com/) open-source API client, you'll just need to open the Bruno collection located at [`./bruno/`](./bruno/).

## Grafana dashboards

Graphix comes with a set of pre-built Grafana dashboards. Copying these dashboads to your Grafana instance is a 2-step process:

1. Install and configure the [Infinity data source plugin](https://grafana.com/docs/plugins/yesoreyeram-infinity-datasource/latest/).
2. Import the dashboards from the [`./grafana/dashboards/`](./grafana/dashboards/) directory, one by one.

## Configuration

Graphix accepts a few CLI options as *server* configuration, as well as a YAML file for fine-grained Graphix-specific configuration. The format for the YAML configuration file is described [here](./crates/autogen_config_json_schema//schema.json) and you can find some examples in the [`./configs/`](./configs/) directory. You can also copy [`./.vscode/settings.default.json`](./.vscode/settings.default.json) to your VS Code settings file to get autocomplete for Graphix configuration files. Configuration parsing logic is implemented in [`./crates/graphix_lib/src/config.rs`](./crates/graphix_lib/src/config.rs).

### Configuration sources

Configuration sources are expressed as a list of objects, the kind of which is specified through `kind: <string>`. The following kinds are supported:

- `kind: 'indexer'`,
- `kind: 'indexerByAddress'`,
- `kind: 'interceptor'`,
- `kind: 'networkSubgraph'`.

Both `indexer` and `indexerByAddress` as configuration sources add a specific indexer to the indexer pool that Graphix uses to compare PoIs. If you run an indexer that you wish to monitor for PoI correctness, for example, any of these two configuration options will make sure that Graphix includes your indexer in its comparisons. As for the difference between the two, `indexer` specifies the indexer by its index node GraphQL URL, while `indexerByAddress` specifies the indexer by its address which is then queried from the network subgraph.

`interceptor` is only used for mocking and testing, and it shouldn't be used in production environments.

`networkSubgraph` is by far the most powerful configuration source. Instead of sourcing a single indexer like `indexer` and `indexerByAddress`, `networkSubgraph` will query the given network subgraph, and list all indexers found through that subgraph. This is the easiest way to aggregate data from a large subset of all active indexers on the network.

Each of these configuration sources has its own set of configuration values. For more information, you can take a look at these files in this repository:
- `ops/compose/graphix/network.yml`, which is the configuration file used by the local `docker-compose` setup.
- The configuration parsing code: `backend/crates/common/src/config.rs`.


# Copyright

Copyright (c) 2021-2024, Edge & Node Ventures, Inc.

Graphix is dual-licensed under the terms of the MIT license and the Apache License, Version 2.0.
