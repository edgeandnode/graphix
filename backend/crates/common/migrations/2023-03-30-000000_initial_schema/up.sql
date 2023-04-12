-- We'll have a routine that runs every few hours or so that deletes all blocks, PoIs,
-- PoI divergence reports, and indexer metadata older than one week.

CREATE TABLE indexers (
  id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
  name TEXT,
  address BYTEA UNIQUE,
  created_at TIMESTAMP NOT NULL DEFAULT NOW(),

  UNIQUE (name, address),
);

CREATE INDEX ON indexers (address);

-- Networks should be PostgreSQL namespaces ('schemas') but Diesel has poor
-- support for them.
CREATE TABLE networks (
  id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
  name TEXT NOT NULL UNIQUE,
  created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX ON networks (name);

CREATE TABLE sg_deployments (
  id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
  cid TEXT NOT NULL UNIQUE,
  network INTEGER NOT NULL REFERENCES networks(id) ON DELETE CASCADE,
  created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX ON sg_deployments (deployment);

CREATE TABLE sg_names (
  id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
  sg_deployment_id INTEGER NOT NULL REFERENCES sg_deployments(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX ON sg_names (sg_deployment_id);

CREATE TABLE blocks (
  id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
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
  poi BYTEA UNIQUE NOT NULL,
  sg_deployment_id INTEGER NOT NULL REFERENCES sg_deployments(id) ON DELETE CASCADE,
  indexer_id INTEGER NOT NULL REFERENCES indexers(id),
  block_id INTEGER NOT NULL REFERENCES blocks(id),
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
  poi_id INTEGER NOT NULL REFERENCES pois(id) ON DELETE CASCADE
);

CREATE TABLE poi_divergence_bisect_reports (
  id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
  poi1_id INTEGER NOT NULL REFERENCES pois(id) ON DELETE RESTRICT,
  poi2_id INTEGER NOT NULL REFERENCES pois(id) ON DELETE RESTRICT
  divergence_block_id INTEGER NOT NULL REFERENCES blocks(id),
  created_at TIMESTAMP NOT NULL DEFAULT NOW(),
  UNIQUE (poi1_id, poi2_id),
  -- We always "normalize" the ordering between PoI 1 & 2.
  CHECK (poi1_id < poi2_id)
);

CREATE INDEX ON poi_divergence_bisect_reports (divergence_block_id);

-- Indexer metadata.

CREATE TABLE block_cache_entries (
  id BIGINT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
  indexer_id INTEGER NOT NULL REFERENCES indexers(id),
  block_id BIGINT NOT NULL REFERENCES blocks(id),
  block_data JSONB NOT NULL,
  created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX ON block_cache_entries (indexer_id, block_id);

CREATE TABLE eth_call_cache_entries (
  id BIGINT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
  indexer_id INTEGER NOT NULL REFERENCES indexers(id),
  block_id BIGINT NOT NULL REFERENCES blocks(id),
  eth_call_data JSONB NOT NULL,
  eth_call_result JSONB NOT NULL,
  created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX ON eth_call_cache_entries (indexer_id, block_id);

CREATE TABLE entity_changes_in_block (
  id BIGINT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
  indexer_id INTEGER NOT NULL REFERENCES indexers(id),
  block_id BIGINT NOT NULL REFERENCES blocks(id),
  entity_change_data JSONB NOT NULL,
  created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX ON entity_changes_in_block (indexer_id, block_id);
