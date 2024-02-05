CREATE TABLE indexer_versions (
	id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
	indexer_id INTEGER NOT NULL REFERENCES indexers(id) ON DELETE CASCADE,
	error TEXT,
	version_string TEXT,
	version_commit TEXT,
	created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX indexer_versions_indexer_id_idx ON indexer_versions(indexer_id);

CREATE TABLE sg_deployment_api_versions (
	id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
	sg_deployment_id INTEGER NOT NULL REFERENCES sg_deployments(id) ON DELETE CASCADE,
	api_versions TEXT[] DEFAULT '{}',
	error TEXT,
	created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX sg_deployment_api_versions_sg_deployment_id_idx ON sg_deployment_api_versions(sg_deployment_id);
