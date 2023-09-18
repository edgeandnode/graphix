-- We'll have a routine that runs every few hours or so that deletes all blocks, PoIs,
-- PoI divergence reports, and indexer metadata older than one week.

CREATE TABLE indexers (
  id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
  name TEXT,
  address BYTEA UNIQUE,
  created_at TIMESTAMP NOT NULL DEFAULT NOW(),

  UNIQUE (name, address)
);

CREATE INDEX ON indexers (address);
CREATE INDEX ON indexers (name);

-- Networks should be PostgreSQL namespaces ('schemas') but Diesel has poor
-- support for them.
CREATE TABLE networks (
  id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
  name TEXT NOT NULL UNIQUE,
  created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX ON networks (name);

-- Insert the "mainnet" network with id == 1.
-- See also: hardcoded-mainnet
INSERT INTO networks (name) VALUES ('mainnet');

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
  -- We're wasting space and performance by using UUID as TEXT, but it's simpler
  -- and operations on this table won't be a bottleneck.
  uuid TEXT PRIMARY KEY,
  request JSONB NOT NULL,
  created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE TABLE divergence_investigation_reports (
  uuid TEXT PRIMARY KEY,
  report JSONB NOT NULL,
  created_at TIMESTAMP NOT NULL DEFAULT NOW()
);
