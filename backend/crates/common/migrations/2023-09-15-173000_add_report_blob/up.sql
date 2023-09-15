ALTER TABLE
  poi_divergence_bisect_reports
ADD
  COLUMN report_blob JSONB NOT NULL DEFAULT '{}';
