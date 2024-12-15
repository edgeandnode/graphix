CREATE TABLE graph_node_collected_versions (
	id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
	version_string TEXT,
	version_commit TEXT,
	error_response TEXT,
	collected_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE TABLE indexer_network_subgraph_metadata (
	id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
  geohash TEXT,
  indexer_url TEXT,
  staked_tokens DECIMAL NOT NULL,
  allocated_tokens DECIMAL NOT NULL,
  locked_tokens DECIMAL NOT NULL,
  query_fees_collected DECIMAL NOT NULL,
  query_fee_rebates DECIMAL NOT NULL,
  rewards_earned DECIMAL NOT NULL,
  indexer_indexing_rewards DECIMAL NOT NULL,
  delegator_indexing_rewards DECIMAL NOT NULL,
  last_updated_at TIMESTAMP NOT NULL
);

CREATE TABLE indexers (
  id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
  address BYTEA UNIQUE NOT NULL,
  name TEXT,
  graph_node_version INTEGER REFERENCES graph_node_collected_versions ON DELETE CASCADE,
  network_subgraph_metadata INTEGER REFERENCES indexer_network_subgraph_metadata ON DELETE CASCADE,
  created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX ON indexers (address);
CREATE INDEX ON indexers (name);

-- Networks should probably be PostgreSQL namespaces ('schemas') but Diesel has
-- poor support for them.
CREATE TABLE networks (
  id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
  name TEXT NOT NULL UNIQUE,
  caip2 TEXT UNIQUE,
  created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX ON networks (name);

CREATE TABLE sg_deployments (
  id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
  ipfs_cid TEXT NOT NULL UNIQUE,
  network INTEGER NOT NULL REFERENCES networks(id) ON DELETE CASCADE,
  created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX ON sg_deployments (ipfs_cid);

CREATE TABLE sg_names (
  id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
  sg_deployment_id INTEGER NOT NULL UNIQUE REFERENCES sg_deployments(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX ON sg_names (sg_deployment_id);

CREATE TABLE blocks (
  id BIGINT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
  network_id INTEGER NOT NULL REFERENCES networks(id) ON DELETE CASCADE,
  number BIGINT NOT NULL,
  hash BYTEA NOT NULL,
  UNIQUE (network_id, hash)
);

-- Not worth it to index by `network_id` first because we support few
-- networks.
CREATE INDEX ON blocks (number);
CREATE INDEX ON blocks (hash);

-- PoI stuff.

CREATE TABLE pois (
  id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
  poi BYTEA NOT NULL,
  sg_deployment_id INTEGER NOT NULL REFERENCES sg_deployments(id) ON DELETE CASCADE,
  indexer_id INTEGER NOT NULL REFERENCES indexers(id),
  block_id BIGINT NOT NULL REFERENCES blocks(id),
  created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

-- We'll only need this if we allow "search by PoI":
-- CREATE INDEX ON pois (poi);

-- We won't keep many PoIs around, so having many indexes is fine.
CREATE INDEX ON pois (sg_deployment_id, indexer_id);
CREATE INDEX ON pois (block_id);
CREATE INDEX ON pois (indexer_id);

CREATE TABLE live_pois (
  id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
  sg_deployment_id INTEGER NOT NULL REFERENCES sg_deployments(id) ON DELETE CASCADE,
  indexer_id INTEGER NOT NULL REFERENCES indexers(id),
  poi_id INTEGER NOT NULL REFERENCES pois(id) ON DELETE CASCADE,

  UNIQUE (sg_deployment_id, indexer_id)
);

-- Divergence investigations.

CREATE TABLE pending_divergence_investigation_requests (
  uuid UUID PRIMARY KEY,
  request JSONB NOT NULL,
  created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE TABLE divergence_investigation_reports (
  uuid UUID PRIMARY KEY,
  report JSONB NOT NULL,
  created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE TABLE sg_deployment_api_versions (
	id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
	sg_deployment_id INTEGER NOT NULL REFERENCES sg_deployments(id) ON DELETE CASCADE,
	api_versions TEXT[] DEFAULT '{}',
	error TEXT,
	created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX sg_deployment_api_versions_sg_deployment_id_idx ON sg_deployment_api_versions(sg_deployment_id);

CREATE TABLE failed_queries (
	id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
	indexer_id INTEGER NOT NULL REFERENCES indexers(id) ON DELETE CASCADE,
	query_name TEXT NOT NULL,
	raw_query TEXT NOT NULL,
	response TEXT NOT NULL,
	request_timestamp TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX ON failed_queries (indexer_id, query_name);

-- API key management.
CREATE TABLE graphix_api_tokens (
  public_prefix TEXT PRIMARY KEY,
  sha256_api_key_hash BYTEA NOT NULL UNIQUE,
  notes TEXT,
  permission_level INTEGER NOT NULL
);

CREATE TABLE configs (
  id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
  config JSONB NOT NULL,
  created_at TIMESTAMP NOT NULL DEFAULT NOW()
);
